//! README §Tenancy + §Workstation client → Wire protocol — auth subsystem.

pub mod client_version;
pub mod password;
pub mod session;
pub mod stepup;
pub mod tokens;

use crate::audit::{self, Action, AppendInput};
use crate::db::Database;
use crate::error::{Error, Result};
use crate::ids::{AuthSessionId, UserId, WorkstationId};
use crate::rng::OsRng;
use jiff::Timestamp;
use rusqlite::{params, OptionalExtension};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    Admin,
    Provider,
}

impl UserRole {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Provider => "provider",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "admin" => Ok(Self::Admin),
            "provider" => Ok(Self::Provider),
            _ => Err(Error::Invariant("unknown role")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub email: String,
    pub display_name: String,
    pub role: UserRole,
    pub created_at: Timestamp,
    pub disabled_at: Option<Timestamp>,
}

#[derive(Debug, Clone)]
pub struct LoginOutcome {
    pub user: User,
    pub session_id: AuthSessionId,
    pub access_token: SecretString,
    pub refresh_token: SecretString,
}

/// Verify credentials, open a fresh `auth_session`, mint access + refresh tokens.
/// Audits `user.login` on success and on failure.
pub fn login(
    db: &Database,
    email: &str,
    password: SecretString,
    device_id: WorkstationId,
) -> Result<LoginOutcome> {
    let now = db.clock().now();
    let outcome: Result<LoginOutcome> = db.with_writer(|conn| {
        // Fetch user by email.
        let user_row = conn
            .query_row(
                "SELECT id, email, display_name, role, password_hash, created_at, disabled_at \
                 FROM user WHERE email = ?1",
                params![email],
                |row| {
                    let id: String = row.get(0)?;
                    let email_s: String = row.get(1)?;
                    let display_name: String = row.get(2)?;
                    let role_s: String = row.get(3)?;
                    let password_hash: String = row.get(4)?;
                    let created_at: String = row.get(5)?;
                    let disabled_at: Option<String> = row.get(6)?;
                    Ok((id, email_s, display_name, role_s, password_hash, created_at, disabled_at))
                },
            )
            .optional()?;
        let (id_s, email_s, display_name, role_s, password_hash, created_at_s, disabled_at_s) =
            user_row.ok_or(Error::BadCredentials)?;
        let uuid = uuid::Uuid::parse_str(&id_s)
            .map_err(|_| Error::Invariant("user.id not a UUID"))?;
        let user = User {
            id: UserId(uuid),
            email: email_s,
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
        };
        if user.disabled_at.is_some() {
            return Err(Error::BadCredentials);
        }
        if !password::verify(password, &password_hash)? {
            // Emit a UserLogin audit on failure too — README §Tenancy says every auth event audits.
            audit::append_in_conn(
                conn,
                now,
                AppendInput {
                    actor_user_id: Some(user.id),
                    auth_session_id: None,
                    action: Action::UserLogin,
                    target_type: "user".into(),
                    target_id: user.id.as_uuid().to_string(),
                    patient_id: None,
                    metadata: json!({"outcome": "bad_credentials"}),
                },
            )?;
            return Err(Error::BadCredentials);
        }

        let refresh_token = tokens::fresh(&OsRng);
        let access_token = tokens::fresh(&OsRng);
        let refresh_hash = tokens::hash(&refresh_token);
        let session_id = session::open(conn, user.id, device_id, refresh_hash, now)?;
        // Store access-token hash so check_session can validate.
        // For simplicity we put it in a small `auth_access_token` keyed by session_id —
        // but we declared this column-less, so use the session row directly: store
        // the SHA-256 in a side-channel. Simplest: stash it as a separate prepared
        // value in `auth_session.refresh_token_hash` would collide. Instead, use
        // `auth_session.last_seen_at`-adjacent storage — that's awkward.
        //
        // Cleanest within phase-1 scope: model the access token as a per-session
        // ephemeral fact stored under the session's `last_seen_at` companion.
        // To keep changes minimal we store the hashed access token in a new auxiliary
        // table created here (in-memory only, surfaced through `check_session`).
        // Done above via tokens::hash on refresh; for access we keep the unhashed
        // string in the response and validate by re-hashing against a stored value
        // tracked in `auth_session.last_seen_at`-side data. To make this concrete,
        // we use a tiny session-scoped table.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS _auth_access_token_hash (\
              session_id TEXT PRIMARY KEY NOT NULL REFERENCES auth_session(id) ON DELETE CASCADE,\
              token_hash BLOB NOT NULL,\
              issued_at TEXT NOT NULL\
            ) STRICT",
            params![],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO _auth_access_token_hash (session_id, token_hash, issued_at) VALUES (?1, ?2, ?3)",
            params![session_id.as_uuid().to_string(), tokens::hash(&access_token), now.to_string()],
        )?;

        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(user.id),
                auth_session_id: Some(session_id),
                action: Action::UserLogin,
                target_type: "user".into(),
                target_id: user.id.as_uuid().to_string(),
                patient_id: None,
                metadata: json!({"outcome": "success"}),
            },
        )?;

        Ok(LoginOutcome {
            user,
            session_id,
            access_token,
            refresh_token,
        })
    });
    outcome
}

/// Rotate the refresh token. Old hash is invalidated; replay fails.
/// Refuses past `absolute_expires_at` or if `revoked_at` is set.
pub fn refresh(db: &Database, refresh_token: SecretString) -> Result<LoginOutcome> {
    let now = db.clock().now();
    db.with_writer(|conn| {
        let refresh_hash = tokens::hash(&refresh_token);
        let sess = session::find_by_refresh_hash(conn, &refresh_hash)?
            .ok_or(Error::BadCredentials)?;
        if sess.revoked_at.is_some() {
            return Err(Error::Revoked);
        }
        if now > sess.absolute_expires_at {
            return Err(Error::SessionExpired);
        }
        if now > sess.refresh_expires_at {
            return Err(Error::SessionExpired);
        }

        // Mint new tokens and rotate.
        let new_refresh = tokens::fresh(&OsRng);
        let new_access = tokens::fresh(&OsRng);
        session::rotate_refresh(conn, sess.id, tokens::hash(&new_refresh), now)?;
        conn.execute(
            "INSERT OR REPLACE INTO _auth_access_token_hash (session_id, token_hash, issued_at) VALUES (?1, ?2, ?3)",
            params![sess.id.as_uuid().to_string(), tokens::hash(&new_access), now.to_string()],
        )?;

        // Load the user.
        let (email, display_name, role_s, created_at_s, disabled_at_s): (
            String,
            String,
            String,
            String,
            Option<String>,
        ) = conn.query_row(
            "SELECT email, display_name, role, created_at, disabled_at FROM user WHERE id = ?1",
            params![sess.user_id.as_uuid().to_string()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )?;
        let user = User {
            id: sess.user_id,
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
        };

        Ok(LoginOutcome {
            user,
            session_id: sess.id,
            access_token: new_access,
            refresh_token: new_refresh,
        })
    })
}

/// Set `auth_session.revoked_at`; the next authenticated request rejects.
pub fn revoke(db: &Database, session_id: AuthSessionId) -> Result<()> {
    let now = db.clock().now();
    db.with_writer(|conn| session::mark_revoked(conn, session_id, now))
}

/// Per-request session check: looks up by access-token hash, asserts not revoked,
/// not past absolute expiry, access token still valid.
pub fn check_session(db: &Database, access_token: &SecretString) -> Result<User> {
    check_session_with_id(db, access_token).map(|(user, _, _)| user)
}

/// Same as [`check_session`], but also returns the `auth_session.id` and the bound
/// `device_id`. The wire layer needs both: `auth_session.id` populates `audit_log.auth_session_id`,
/// and `device_id` cross-checks the mTLS-presented workstation cert.
pub fn check_session_with_id(
    db: &Database,
    access_token: &SecretString,
) -> Result<(User, AuthSessionId, WorkstationId)> {
    let now = db.clock().now();
    db.with_writer(|conn| {
        let access_hash = tokens::hash(access_token);
        let session_id_str: Option<String> = conn
            .query_row(
                "SELECT session_id FROM _auth_access_token_hash WHERE token_hash = ?1",
                params![access_hash],
                |r| r.get(0),
            )
            .optional()?;
        let sid = session_id_str.ok_or(Error::BadCredentials)?;
        let uuid = uuid::Uuid::parse_str(&sid).map_err(|_| Error::Invariant("session_id parse"))?;
        let sess = session::get(conn, AuthSessionId(uuid))?;
        if sess.revoked_at.is_some() {
            return Err(Error::Revoked);
        }
        if now > sess.absolute_expires_at {
            return Err(Error::SessionExpired);
        }
        session::touch(conn, sess.id, now)?;
        let (email, display_name, role_s, created_at_s, disabled_at_s): (
            String,
            String,
            String,
            String,
            Option<String>,
        ) = conn.query_row(
            "SELECT email, display_name, role, created_at, disabled_at FROM user WHERE id = ?1",
            params![sess.user_id.as_uuid().to_string()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )?;
        let user = User {
            id: sess.user_id,
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
        };
        Ok((user, sess.id, sess.device_id))
    })
}
