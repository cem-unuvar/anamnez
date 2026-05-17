//! SPEC §Deployment — `workstation` row CRUD + revocation deny-set load.
//!
//! The daemon loads `list_revoked()` into an in-memory `HashSet<WorkstationId>` at boot
//! and rejects mTLS handshakes whose client cert maps to a revoked device.
//!
//! Enrollment is two-phase: `mint_enrollment()` (called by `anamnez admin
//! enroll-workstation`) inserts a row into `workstation_enrollment` keyed by a
//! one-time token hash and prints an `anamnez://enroll?...` URI; the
//! workstation client posts the token to `/v1/enroll/exchange`, which calls
//! `exchange_enrollment()` here to mint a client cert and create the
//! workstation row.

use crate::audit::{self, Action, AppendInput};
use crate::db::Database;
use crate::error::{Error, Result};
use crate::ids::{AuthSessionId, UserId, WorkstationId};
use jiff::Timestamp;
use rcgen::{CertificateParams, DnType, KeyPair};
use rusqlite::{params, OptionalExtension};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Bound,
    Shared,
}

impl Mode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bound => "bound",
            Self::Shared => "shared",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "bound" => Ok(Self::Bound),
            "shared" => Ok(Self::Shared),
            _ => Err(Error::Invariant("unknown workstation mode")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workstation {
    pub id: WorkstationId,
    pub label: String,
    pub mode: Mode,
    pub bound_user_id: Option<UserId>,
    pub cert_serial: String,
    pub cert_fingerprint: String,
    pub enrolled_at: Timestamp,
    pub enrolled_by: UserId,
    pub last_seen_at: Option<Timestamp>,
    pub revoked_at: Option<Timestamp>,
    pub revoked_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewWorkstation {
    pub label: String,
    pub mode: Mode,
    pub bound_user_id: Option<UserId>,
    pub cert_serial: String,
    pub cert_fingerprint: String,
}

/// Enroll a new workstation. `mode = Bound` requires `bound_user_id`; `Shared` forbids it.
/// Audits `Action::WorkstationEnroll`.
pub fn enroll(db: &Database, admin: UserId, input: NewWorkstation) -> Result<Workstation> {
    if matches!(input.mode, Mode::Bound) && input.bound_user_id.is_none() {
        return Err(Error::Invariant(
            "workstation mode=bound requires bound_user_id",
        ));
    }
    if matches!(input.mode, Mode::Shared) && input.bound_user_id.is_some() {
        return Err(Error::Invariant(
            "workstation mode=shared forbids bound_user_id",
        ));
    }
    let id = WorkstationId::new();
    let now = db.clock().now();
    let ws = Workstation {
        id,
        label: input.label.clone(),
        mode: input.mode,
        bound_user_id: input.bound_user_id,
        cert_serial: input.cert_serial.clone(),
        cert_fingerprint: input.cert_fingerprint.clone(),
        enrolled_at: now,
        enrolled_by: admin,
        last_seen_at: None,
        revoked_at: None,
        revoked_reason: None,
    };

    db.with_writer(|conn| {
        conn.execute(
            "INSERT INTO workstation \
             (id, label, mode, bound_user_id, cert_serial, cert_fingerprint, enrolled_at, enrolled_by) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                ws.id.as_uuid().to_string(),
                ws.label,
                ws.mode.as_str(),
                ws.bound_user_id.map(|u| u.as_uuid().to_string()),
                ws.cert_serial,
                ws.cert_fingerprint,
                ws.enrolled_at.to_string(),
                ws.enrolled_by.as_uuid().to_string(),
            ],
        )?;
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(admin),
                auth_session_id: None,
                action: Action::WorkstationEnroll,
                target_type: "workstation".into(),
                target_id: ws.id.as_uuid().to_string(),
                patient_id: None,
                metadata: json!({"label": ws.label, "mode": ws.mode.as_str()}),
            },
        )?;
        Ok(())
    })?;
    Ok(ws)
}

/// Revoke a workstation. Sets `revoked_at`; the daemon adds the device_id to its
/// in-memory deny set so subsequent mTLS handshakes fail. Audits `Action::WorkstationRevoke`.
pub fn revoke(db: &Database, admin: UserId, id: WorkstationId, reason: String) -> Result<()> {
    let now = db.clock().now();
    db.with_writer(|conn| {
        let affected = conn.execute(
            "UPDATE workstation SET revoked_at = ?2, revoked_reason = ?3 \
             WHERE id = ?1 AND revoked_at IS NULL",
            params![id.as_uuid().to_string(), now.to_string(), reason],
        )?;
        if affected == 0 {
            return Err(Error::NotFound);
        }
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(admin),
                auth_session_id: None,
                action: Action::WorkstationRevoke,
                target_type: "workstation".into(),
                target_id: id.as_uuid().to_string(),
                patient_id: None,
                metadata: json!({"reason": reason}),
            },
        )?;
        Ok(())
    })
}

/// Snapshot of revoked device_ids — loaded into the daemon's in-memory deny set at boot.
pub fn list_revoked(db: &Database) -> Result<Vec<WorkstationId>> {
    db.with_reader(|conn| {
        let mut stmt = conn.prepare("SELECT id FROM workstation WHERE revoked_at IS NOT NULL")?;
        let rows: Vec<String> = stmt
            .query_map(params![], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut out = Vec::with_capacity(rows.len());
        for s in rows {
            let uuid = uuid::Uuid::parse_str(&s)
                .map_err(|_| Error::Invariant("workstation.id not a UUID"))?;
            out.push(WorkstationId(uuid));
        }
        Ok(out)
    })
}

pub fn get(db: &Database, id: WorkstationId) -> Result<Option<Workstation>> {
    db.with_reader(|conn| {
        conn.query_row(
            "SELECT id, label, mode, bound_user_id, cert_serial, cert_fingerprint, \
                    enrolled_at, enrolled_by, last_seen_at, revoked_at, revoked_reason \
             FROM workstation WHERE id = ?1",
            params![id.as_uuid().to_string()],
            row_to_workstation,
        )
        .optional()
        .map_err(Error::from)
    })
}

/// Outcome of `mint_enrollment` — the on-disk token printed in the enrollment
/// URI plus the host/fingerprint that lets the client locate and pin the server.
#[derive(Debug, Clone)]
pub struct MintedEnrollment {
    pub enrollment_id: uuid::Uuid,
    pub token: SecretString,
    pub uri: String,
}

#[derive(Debug, Clone)]
pub struct NewEnrollment {
    pub label: String,
    pub mode: Mode,
    pub bound_user_id: Option<UserId>,
    /// LAN host the workstation client connects to. Embedded in the URI.
    pub host: String,
    /// Hex-encoded SHA-256 fingerprint of the server's TLS cert. Embedded in
    /// the URI so the client can pin without going through an OS trust store.
    pub server_fingerprint_sha256: String,
}

const ENROLLMENT_TTL_HOURS: i64 = 24;

/// Issue a workstation enrollment. Inserts a row into `workstation_enrollment`
/// keyed by the SHA-256 of a freshly-minted 32-byte token, returns the URI the
/// admin hands to the workstation user. Audits `Action::WorkstationEnroll` with
/// `target_type = "workstation_enrollment"`.
pub fn mint_enrollment(
    db: &Database,
    admin: UserId,
    rng: &dyn crate::rng::Rng,
    input: NewEnrollment,
) -> Result<MintedEnrollment> {
    if matches!(input.mode, Mode::Bound) && input.bound_user_id.is_none() {
        return Err(Error::Invariant(
            "enrollment mode=bound requires bound_user_id",
        ));
    }
    if matches!(input.mode, Mode::Shared) && input.bound_user_id.is_some() {
        return Err(Error::Invariant(
            "enrollment mode=shared forbids bound_user_id",
        ));
    }

    let mut token_bytes = [0u8; 32];
    rng.fill_bytes(&mut token_bytes);
    let token_hex = hex_lower(&token_bytes);
    let token_hash = sha256(token_hex.as_bytes());

    let id = uuid::Uuid::new_v4();
    let now = db.clock().now();
    let expires = now
        .checked_add(std::time::Duration::from_secs(
            60 * 60 * ENROLLMENT_TTL_HOURS as u64,
        ))
        .map_err(|_| Error::Invariant("enrollment expires_at overflow"))?;

    db.with_writer(|conn| {
        conn.execute(
            "INSERT INTO workstation_enrollment \
             (id, label, mode, bound_user_id, token_hash, created_by, created_at, expires_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id.to_string(),
                input.label,
                input.mode.as_str(),
                input.bound_user_id.map(|u| u.as_uuid().to_string()),
                token_hash,
                admin.as_uuid().to_string(),
                now.to_string(),
                expires.to_string(),
            ],
        )?;
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(admin),
                auth_session_id: None,
                action: Action::WorkstationEnroll,
                target_type: "workstation_enrollment".into(),
                target_id: id.to_string(),
                patient_id: None,
                metadata: json!({"phase": "issued", "label": input.label, "mode": input.mode.as_str()}),
            },
        )?;
        Ok(())
    })?;

    let uri = format!(
        "anamnez://enroll?host={}&fingerprint={}&token={}",
        url_escape(&input.host),
        url_escape(&input.server_fingerprint_sha256),
        token_hex,
    );

    Ok(MintedEnrollment {
        enrollment_id: id,
        token: SecretString::from(token_hex),
        uri,
    })
}

/// What the daemon returns to the workstation client after a successful
/// enrollment exchange. The client persists `client_cert_pem` + `client_key_pem`
/// in the OS secret store and uses them as the mTLS identity from then on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangedEnrollment {
    pub workstation_id: WorkstationId,
    pub client_cert_pem: String,
    pub client_key_pem: String,
    pub ca_cert_pem: String,
}

/// Consume an enrollment token. Validates the token hash against a non-claimed,
/// non-expired row, mints a workstation client cert signed by the CA at
/// `<data_dir>/tls/ca_{cert,key}.pem`, creates a matching `workstation` row, and
/// marks the pending enrollment as claimed. Audits a second
/// `Action::WorkstationEnroll` for the now-real workstation.
pub fn exchange_enrollment(
    db: &Database,
    data_dir: &Path,
    token: &SecretString,
) -> Result<ExchangedEnrollment> {
    let token_hash = sha256(token.expose_secret().as_bytes());

    let ca_cert_pem = std::fs::read_to_string(data_dir.join("tls").join("ca_cert.pem"))?;
    let ca_key_pem = std::fs::read_to_string(data_dir.join("tls").join("ca_key.pem"))?;

    let now = db.clock().now();
    db.with_writer(|conn| {
        let row: Option<(String, String, String, Option<String>, String, String)> = conn
            .query_row(
                "SELECT id, label, mode, bound_user_id, created_by, expires_at \
                 FROM workstation_enrollment \
                 WHERE token_hash = ?1 AND claimed_at IS NULL",
                params![token_hash],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                },
            )
            .optional()?;
        let (enrollment_id_s, label, mode_s, bound_user_id_s, admin_s, expires_s) =
            row.ok_or(Error::NotFound)?;

        let expires: Timestamp = expires_s
            .parse()
            .map_err(|_| Error::Invariant("enrollment.expires_at parse"))?;
        if now > expires {
            return Err(Error::Revoked);
        }

        let mode = Mode::parse(&mode_s)?;
        let bound_user_id = match bound_user_id_s {
            None => None,
            Some(s) => {
                let u = uuid::Uuid::parse_str(&s)
                    .map_err(|_| Error::Invariant("bound_user_id not a UUID"))?;
                Some(UserId(u))
            }
        };
        let admin_uuid = uuid::Uuid::parse_str(&admin_s)
            .map_err(|_| Error::Invariant("enrollment.created_by not a UUID"))?;
        let admin = UserId(admin_uuid);

        let workstation_id = WorkstationId::new();
        let device_cn = workstation_id.as_uuid().to_string();

        let mut ws_params = CertificateParams::new(Vec::<String>::new())
            .map_err(|e| invariant(&format!("workstation params: {e}")))?;
        ws_params.distinguished_name.push(DnType::CommonName, &device_cn);
        let ws_key = KeyPair::generate().map_err(|e| invariant(&format!("ws key: {e}")))?;
        let ca_signer_kp =
            KeyPair::from_pem(&ca_key_pem).map_err(|e| invariant(&format!("clone ca key: {e}")))?;
        let mut ca_params = CertificateParams::new(Vec::<String>::new())
            .map_err(|e| invariant(&format!("ca params: {e}")))?;
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        ca_params
            .distinguished_name
            .push(DnType::CommonName, "anamnez-local-ca");
        let signer_ca_cert = ca_params
            .self_signed(&ca_signer_kp)
            .map_err(|e| invariant(&format!("ca self-sign for signing: {e}")))?;
        let ws_cert = ws_params
            .signed_by(&ws_key, &signer_ca_cert, &ca_signer_kp)
            .map_err(|e| invariant(&format!("ws signed: {e}")))?;
        let client_cert_pem = ws_cert.pem();
        let client_key_pem = ws_key.serialize_pem();

        let cert_serial = uuid::Uuid::new_v4().to_string();
        let cert_fp = sha256_hex(client_cert_pem.as_bytes());

        conn.execute(
            "INSERT INTO workstation \
             (id, label, mode, bound_user_id, cert_serial, cert_fingerprint, enrolled_at, enrolled_by) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                workstation_id.as_uuid().to_string(),
                label,
                mode.as_str(),
                bound_user_id.map(|u| u.as_uuid().to_string()),
                cert_serial,
                cert_fp,
                now.to_string(),
                admin.as_uuid().to_string(),
            ],
        )?;
        conn.execute(
            "UPDATE workstation_enrollment SET claimed_at = ?2, claimed_workstation_id = ?3 \
             WHERE id = ?1",
            params![
                enrollment_id_s,
                now.to_string(),
                workstation_id.as_uuid().to_string(),
            ],
        )?;
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(admin),
                auth_session_id: None,
                action: Action::WorkstationEnroll,
                target_type: "workstation".into(),
                target_id: workstation_id.as_uuid().to_string(),
                patient_id: None,
                metadata: json!({"phase": "claimed", "enrollment_id": enrollment_id_s, "label": label, "mode": mode.as_str()}),
            },
        )?;

        Ok(ExchangedEnrollment {
            workstation_id,
            client_cert_pem,
            client_key_pem,
            ca_cert_pem: ca_cert_pem.clone(),
        })
    })
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn sha256(bytes: &[u8]) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().to_vec()
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex_lower(&sha256(bytes))
}

fn url_escape(s: &str) -> String {
    // Minimal escaping for what we put in URI query values — hex hashes,
    // hostnames, ASCII labels. Replace unsafe chars; full RFC 3986 escaping is
    // not necessary because we know the input alphabet.
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~' | ':') {
                c.to_string()
            } else {
                format!("%{:02X}", c as u32)
            }
        })
        .collect()
}

fn invariant(s: &str) -> Error {
    Error::Invariant(Box::leak(s.to_owned().into_boxed_str()))
}

/// Look up `auth_session.id`s for live sessions bound to a workstation — used to
/// fan out `ForcedLogout` SSE events after a revocation.
pub fn list_sessions_on(db: &Database, id: WorkstationId) -> Result<Vec<AuthSessionId>> {
    db.with_reader(|conn| {
        let mut stmt = conn
            .prepare("SELECT id FROM auth_session WHERE device_id = ?1 AND revoked_at IS NULL")?;
        let rows: Vec<String> = stmt
            .query_map(params![id.as_uuid().to_string()], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut out = Vec::with_capacity(rows.len());
        for s in rows {
            let uuid = uuid::Uuid::parse_str(&s)
                .map_err(|_| Error::Invariant("auth_session.id not a UUID"))?;
            out.push(AuthSessionId(uuid));
        }
        Ok(out)
    })
}

fn row_to_workstation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Workstation> {
    let parse_uuid = |s: &str| {
        uuid::Uuid::parse_str(s).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })
    };
    let parse_ts = |s: &str| -> rusqlite::Result<Timestamp> {
        s.parse().map_err(|e: jiff::Error| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })
    };

    let id: String = row.get(0)?;
    let label: String = row.get(1)?;
    let mode: String = row.get(2)?;
    let bound_user_id: Option<String> = row.get(3)?;
    let cert_serial: String = row.get(4)?;
    let cert_fingerprint: String = row.get(5)?;
    let enrolled_at: String = row.get(6)?;
    let enrolled_by: String = row.get(7)?;
    let last_seen_at: Option<String> = row.get(8)?;
    let revoked_at: Option<String> = row.get(9)?;
    let revoked_reason: Option<String> = row.get(10)?;

    Ok(Workstation {
        id: WorkstationId(parse_uuid(&id)?),
        label,
        mode: Mode::parse(&mode).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        bound_user_id: match bound_user_id {
            None => None,
            Some(s) => Some(UserId(parse_uuid(&s)?)),
        },
        cert_serial,
        cert_fingerprint,
        enrolled_at: parse_ts(&enrolled_at)?,
        enrolled_by: UserId(parse_uuid(&enrolled_by)?),
        last_seen_at: match last_seen_at {
            None => None,
            Some(s) => Some(parse_ts(&s)?),
        },
        revoked_at: match revoked_at {
            None => None,
            Some(s) => Some(parse_ts(&s)?),
        },
        revoked_reason,
    })
}
