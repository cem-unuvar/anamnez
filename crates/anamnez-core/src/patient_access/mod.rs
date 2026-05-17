//! README §Tenancy — `patient_access` row grants a user a level on a specific patient.

pub mod caps;

use crate::audit::{self, Action, AppendInput};
use crate::db::Database;
use crate::error::{Error, Result};
use crate::ids::{PatientId, UserId};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessLevel {
    Owner,
    Collaborator,
    ReadOnly,
}

impl AccessLevel {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Collaborator => "collaborator",
            Self::ReadOnly => "read_only",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "owner" => Ok(Self::Owner),
            "collaborator" => Ok(Self::Collaborator),
            "read_only" => Ok(Self::ReadOnly),
            _ => Err(Error::Invariant("unknown access level")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientAccess {
    pub patient_id: PatientId,
    pub user_id: UserId,
    pub level: AccessLevel,
}

/// Grant collaborator / read_only access to a patient. `granter` must already be the owner.
/// To assign `Owner`, use [`transfer_ownership`] instead — owner is never granted via this path.
pub fn grant(
    db: &Database,
    granter: UserId,
    patient_id: PatientId,
    grantee: UserId,
    level: AccessLevel,
) -> Result<()> {
    if matches!(level, AccessLevel::Owner) {
        return Err(Error::Invariant(
            "use transfer_ownership to assign Owner — grant() only handles collaborator / read_only",
        ));
    }

    db.with_writer(|conn| {
        require_owner_in_conn(conn, granter, patient_id)?;

        // Upsert: existing access for (patient, user) gets overwritten with the new level.
        conn.execute(
            "INSERT INTO patient_access (patient_id, user_id, level) VALUES (?1, ?2, ?3) \
             ON CONFLICT(patient_id, user_id) DO UPDATE SET level = excluded.level",
            params![
                patient_id.as_uuid().to_string(),
                grantee.as_uuid().to_string(),
                level.as_str(),
            ],
        )?;

        audit::append_in_conn(
            conn,
            db.clock().now(),
            AppendInput {
                actor_user_id: Some(granter),
                auth_session_id: None,
                action: Action::PatientAccessGrant,
                target_type: "patient_access".into(),
                target_id: format!("{}:{}", patient_id.as_uuid(), grantee.as_uuid()),
                patient_id: Some(patient_id),
                metadata: json!({"grantee": grantee.as_uuid().to_string(), "level": level.as_str()}),
            },
        )?;
        Ok(())
    })
}

/// Revoke access for `grantee` on `patient_id`. Cannot revoke an owner via this path —
/// use [`transfer_ownership`] to relinquish ownership first.
pub fn revoke(
    db: &Database,
    revoker: UserId,
    patient_id: PatientId,
    grantee: UserId,
) -> Result<()> {
    db.with_writer(|conn| {
        require_owner_in_conn(conn, revoker, patient_id)?;
        let existing: Option<String> = conn
            .query_row(
                "SELECT level FROM patient_access WHERE patient_id = ?1 AND user_id = ?2",
                params![
                    patient_id.as_uuid().to_string(),
                    grantee.as_uuid().to_string()
                ],
                |r| r.get(0),
            )
            .optional()?;
        match existing {
            None => return Err(Error::NotFound),
            Some(s) if s == "owner" => {
                return Err(Error::Invariant(
                    "cannot revoke owner directly — transfer ownership first",
                ))
            }
            _ => {}
        }
        conn.execute(
            "DELETE FROM patient_access WHERE patient_id = ?1 AND user_id = ?2",
            params![
                patient_id.as_uuid().to_string(),
                grantee.as_uuid().to_string()
            ],
        )?;
        audit::append_in_conn(
            conn,
            db.clock().now(),
            AppendInput {
                actor_user_id: Some(revoker),
                auth_session_id: None,
                action: Action::PatientAccessRevoke,
                target_type: "patient_access".into(),
                target_id: format!("{}:{}", patient_id.as_uuid(), grantee.as_uuid()),
                patient_id: Some(patient_id),
                metadata: json!({"revoked": grantee.as_uuid().to_string()}),
            },
        )?;
        Ok(())
    })
}

/// Single-transaction ownership transfer: demote `from` to `collaborator`, promote `to` to `owner`.
/// `from` must be the current owner; `to` may have any prior access level (including none).
pub fn transfer_ownership(
    db: &Database,
    actor: UserId,
    patient_id: PatientId,
    from: UserId,
    to: UserId,
) -> Result<()> {
    if from == to {
        return Err(Error::Invariant("cannot transfer ownership to self"));
    }
    db.with_writer(|conn| {
        // `from` must currently be the sole owner.
        let from_level: Option<String> = conn
            .query_row(
                "SELECT level FROM patient_access WHERE patient_id = ?1 AND user_id = ?2",
                params![patient_id.as_uuid().to_string(), from.as_uuid().to_string()],
                |r| r.get(0),
            )
            .optional()?;
        match from_level.as_deref() {
            Some("owner") => {}
            Some(_) => return Err(Error::Forbidden),
            None => return Err(Error::NotFound),
        }

        // Step 1: downgrade the existing owner to collaborator so the partial unique
        // index permits the promotion below.
        conn.execute(
            "UPDATE patient_access SET level = 'collaborator' \
             WHERE patient_id = ?1 AND user_id = ?2",
            params![patient_id.as_uuid().to_string(), from.as_uuid().to_string()],
        )?;
        // Step 2: promote `to` to owner (upsert).
        conn.execute(
            "INSERT INTO patient_access (patient_id, user_id, level) VALUES (?1, ?2, 'owner') \
             ON CONFLICT(patient_id, user_id) DO UPDATE SET level = 'owner'",
            params![patient_id.as_uuid().to_string(), to.as_uuid().to_string()],
        )?;

        audit::append_in_conn(
            conn,
            db.clock().now(),
            AppendInput {
                actor_user_id: Some(actor),
                auth_session_id: None,
                action: Action::PatientOwnershipTransfer,
                target_type: "patient".into(),
                target_id: patient_id.as_uuid().to_string(),
                patient_id: Some(patient_id),
                metadata: json!({"from": from.as_uuid().to_string(), "to": to.as_uuid().to_string()}),
            },
        )?;
        Ok(())
    })
}

pub fn list(db: &Database, patient_id: PatientId) -> Result<Vec<PatientAccess>> {
    db.with_reader(|conn| {
        let mut stmt =
            conn.prepare("SELECT user_id, level FROM patient_access WHERE patient_id = ?1")?;
        let rows = stmt.query_map(params![patient_id.as_uuid().to_string()], |r| {
            let user_id: String = r.get(0)?;
            let level: String = r.get(1)?;
            Ok((user_id, level))
        })?;
        let mut out = Vec::new();
        for r in rows {
            let (uid, lvl) = r?;
            let uuid = uuid::Uuid::parse_str(&uid)
                .map_err(|_| Error::Invariant("patient_access.user_id not a UUID"))?;
            out.push(PatientAccess {
                patient_id,
                user_id: UserId(uuid),
                level: AccessLevel::parse(&lvl)?,
            });
        }
        Ok(out)
    })
}

pub fn level_for(
    db: &Database,
    user: UserId,
    patient_id: PatientId,
) -> Result<Option<AccessLevel>> {
    db.with_reader(|conn| level_for_in_conn(conn, user, patient_id))
}

/// Same as [`level_for`] but on a borrowed connection.
pub fn level_for_in_conn(
    conn: &Connection,
    user: UserId,
    patient_id: PatientId,
) -> Result<Option<AccessLevel>> {
    let lvl: Option<String> = conn
        .query_row(
            "SELECT level FROM patient_access WHERE patient_id = ?1 AND user_id = ?2",
            params![patient_id.as_uuid().to_string(), user.as_uuid().to_string()],
            |r| r.get(0),
        )
        .optional()?;
    match lvl {
        None => Ok(None),
        Some(s) => Ok(Some(AccessLevel::parse(&s)?)),
    }
}

/// Insert the creator-as-owner row. Called only from `patient::create` inside the
/// same transaction; never exposed publicly because the only legitimate way to
/// become an owner is to create the patient or be the successor in a transfer.
pub fn insert_creator_as_owner_in_conn(
    conn: &Connection,
    patient_id: PatientId,
    creator: UserId,
) -> Result<()> {
    conn.execute(
        "INSERT INTO patient_access (patient_id, user_id, level) VALUES (?1, ?2, 'owner')",
        params![
            patient_id.as_uuid().to_string(),
            creator.as_uuid().to_string()
        ],
    )?;
    Ok(())
}

fn require_owner_in_conn(conn: &Connection, user: UserId, patient_id: PatientId) -> Result<()> {
    let lvl = level_for_in_conn(conn, user, patient_id)?;
    if matches!(lvl, Some(AccessLevel::Owner)) {
        Ok(())
    } else {
        Err(Error::Forbidden)
    }
}
