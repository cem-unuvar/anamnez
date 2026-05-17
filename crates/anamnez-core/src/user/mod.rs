//! SPEC §Tenancy — admin-driven user CRUD. `login` lives in [`crate::auth`].

use crate::audit::{self, Action, AppendInput};
use crate::auth::{password, User, UserRole};
use crate::db::Database;
use crate::error::{Error, Result};
use crate::ids::UserId;
use jiff::Timestamp;
use rusqlite::{params, OptionalExtension};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct NewUser {
    pub email: String,
    pub display_name: String,
    pub role: UserRole,
    pub password: SecretString,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<UserRole>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub disabled_at: Option<Option<Timestamp>>,
}

/// Create a new user. Argon2id-hashes the password. Audits `Action::UserCreate`.
pub fn create(db: &Database, admin: UserId, input: NewUser) -> Result<User> {
    let id = UserId::new();
    let now = db.clock().now();
    let hash = password::hash(input.password)?;
    let user = User {
        id,
        email: input.email.clone(),
        display_name: input.display_name.clone(),
        role: input.role,
        created_at: now,
        disabled_at: None,
    };
    db.with_writer(|conn| {
        conn.execute(
            "INSERT INTO user (id, email, display_name, role, password_hash, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                user.id.as_uuid().to_string(),
                user.email,
                user.display_name,
                user.role.as_str(),
                hash,
                user.created_at.to_string(),
            ],
        )?;
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(admin),
                auth_session_id: None,
                action: Action::UserCreate,
                target_type: "user".into(),
                target_id: user.id.as_uuid().to_string(),
                patient_id: None,
                metadata: json!({"email": user.email, "role": user.role.as_str()}),
            },
        )?;
        Ok(())
    })?;
    Ok(user)
}

/// Update display_name / role / disabled_at on a user. Audits `Action::UserModify`
/// (or `Action::UserDisable` if the patch sets `disabled_at = Some(Some(_))`).
pub fn update(db: &Database, admin: UserId, id: UserId, patch: UserPatch) -> Result<User> {
    let now = db.clock().now();
    db.with_writer(|conn| {
        let mut current = load_in_conn(conn, id)?.ok_or(Error::NotFound)?;

        if let Some(v) = patch.display_name {
            current.display_name = v;
        }
        if let Some(v) = patch.role {
            current.role = v;
        }
        if let Some(v) = patch.disabled_at {
            current.disabled_at = v;
        }

        let action = if matches!(patch.disabled_at, Some(Some(_))) {
            Action::UserDisable
        } else {
            Action::UserModify
        };

        conn.execute(
            "UPDATE user SET display_name = ?2, role = ?3, disabled_at = ?4 WHERE id = ?1",
            params![
                current.id.as_uuid().to_string(),
                current.display_name,
                current.role.as_str(),
                current.disabled_at.map(|t| t.to_string()),
            ],
        )?;
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(admin),
                auth_session_id: None,
                action,
                target_type: "user".into(),
                target_id: current.id.as_uuid().to_string(),
                patient_id: None,
                metadata: json!({"role": current.role.as_str(), "disabled": current.disabled_at.is_some()}),
            },
        )?;
        Ok(current)
    })
}

pub fn get(db: &Database, id: UserId) -> Result<Option<User>> {
    db.with_reader(|conn| load_in_conn(conn, id))
}

/// Look up a user by email (case-sensitive — matches the unique index).
pub fn find_by_email(db: &Database, email: &str) -> Result<Option<User>> {
    db.with_reader(|conn| {
        let id_str: Option<String> = conn
            .query_row(
                "SELECT id FROM user WHERE email = ?1",
                params![email],
                |r| r.get(0),
            )
            .optional()?;
        let Some(s) = id_str else {
            return Ok(None);
        };
        let uuid = uuid::Uuid::parse_str(&s).map_err(|_| Error::Invariant("user.id not a UUID"))?;
        load_in_conn(conn, UserId(uuid))
    })
}

/// Reset a user's password. In one transaction:
/// 1. Argon2id-hash the new password.
/// 2. Update `user.password_hash`.
/// 3. Revoke every active `auth_session` for the target.
/// 4. Audit `Action::UserModify`.
///
/// SPEC §Tenancy: "password change sets revoked_at" — every request after this
/// returns rejected, regardless of access-token lifetime.
pub fn reset_password(
    db: &Database,
    admin: UserId,
    target: UserId,
    new_password: SecretString,
) -> Result<()> {
    let new_hash = password::hash(new_password)?;
    let now = db.clock().now();
    db.with_writer(|conn| {
        let affected = conn.execute(
            "UPDATE user SET password_hash = ?2 WHERE id = ?1",
            params![target.as_uuid().to_string(), new_hash],
        )?;
        if affected == 0 {
            return Err(Error::NotFound);
        }
        // Revoke every active session for the target.
        conn.execute(
            "UPDATE auth_session SET revoked_at = ?2 \
             WHERE user_id = ?1 AND revoked_at IS NULL",
            params![target.as_uuid().to_string(), now.to_string()],
        )?;
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(admin),
                auth_session_id: None,
                action: Action::UserModify,
                target_type: "user".into(),
                target_id: target.as_uuid().to_string(),
                patient_id: None,
                metadata: json!({"action": "reset_password", "sessions_revoked": true}),
            },
        )?;
        Ok(())
    })
}

fn load_in_conn(conn: &rusqlite::Connection, id: UserId) -> Result<Option<User>> {
    let row = conn
        .query_row(
            "SELECT id, email, display_name, role, created_at, disabled_at FROM user WHERE id = ?1",
            params![id.as_uuid().to_string()],
            |row| {
                let id_s: String = row.get(0)?;
                let email: String = row.get(1)?;
                let display_name: String = row.get(2)?;
                let role_s: String = row.get(3)?;
                let created_at_s: String = row.get(4)?;
                let disabled_at_s: Option<String> = row.get(5)?;
                Ok((
                    id_s,
                    email,
                    display_name,
                    role_s,
                    created_at_s,
                    disabled_at_s,
                ))
            },
        )
        .optional()?;
    let Some((id_s, email, display_name, role_s, created_at_s, disabled_at_s)) = row else {
        return Ok(None);
    };
    let uuid = uuid::Uuid::parse_str(&id_s).map_err(|_| Error::Invariant("user.id not a UUID"))?;
    Ok(Some(User {
        id: UserId(uuid),
        email,
        display_name,
        role: UserRole::parse(&role_s)?,
        created_at: created_at_s
            .parse()
            .map_err(|_| Error::Invariant("user.created_at parse"))?,
        disabled_at: match disabled_at_s {
            None => None,
            Some(s) => Some(
                s.parse()
                    .map_err(|_| Error::Invariant("user.disabled_at parse"))?,
            ),
        },
    }))
}
