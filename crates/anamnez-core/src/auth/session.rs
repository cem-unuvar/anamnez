//! `auth_session` row CRUD + horizon enforcement.

use crate::error::{Error, Result};
use crate::ids::{AuthSessionId, UserId, WorkstationId};
use jiff::Timestamp;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

pub const REFRESH_WINDOW_HOURS: i64 = 12;
pub const ABSOLUTE_HORIZON_DAYS: i64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub id: AuthSessionId,
    pub user_id: UserId,
    pub device_id: WorkstationId,
    pub refresh_token_hash: Vec<u8>,
    pub refresh_expires_at: Timestamp,
    pub absolute_expires_at: Timestamp,
    pub created_at: Timestamp,
    pub last_seen_at: Timestamp,
    pub revoked_at: Option<Timestamp>,
}

pub fn open(
    conn: &rusqlite::Connection,
    user_id: UserId,
    device_id: WorkstationId,
    refresh_token_hash: Vec<u8>,
    now: Timestamp,
) -> Result<AuthSessionId> {
    let id = AuthSessionId::new();
    let refresh_expires_at = now
        .checked_add(std::time::Duration::from_secs(
            60 * 60 * REFRESH_WINDOW_HOURS as u64,
        ))
        .map_err(|_| Error::Invariant("refresh_expires_at overflow"))?;
    let absolute_expires_at = now
        .checked_add(std::time::Duration::from_secs(
            60 * 60 * 24 * ABSOLUTE_HORIZON_DAYS as u64,
        ))
        .map_err(|_| Error::Invariant("absolute_expires_at overflow"))?;
    conn.execute(
        "INSERT INTO auth_session \
         (id, user_id, device_id, refresh_token_hash, refresh_expires_at, absolute_expires_at, created_at, last_seen_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![
            id.as_uuid().to_string(),
            user_id.as_uuid().to_string(),
            device_id.as_uuid().to_string(),
            refresh_token_hash,
            refresh_expires_at.to_string(),
            absolute_expires_at.to_string(),
            now.to_string(),
        ],
    )?;
    Ok(id)
}

pub fn get(conn: &rusqlite::Connection, id: AuthSessionId) -> Result<AuthSession> {
    let row = conn
        .query_row(
            "SELECT id, user_id, device_id, refresh_token_hash, refresh_expires_at, \
                    absolute_expires_at, created_at, last_seen_at, revoked_at \
             FROM auth_session WHERE id = ?1",
            params![id.as_uuid().to_string()],
            row_to_session,
        )
        .optional()?;
    row.ok_or(Error::NotFound)
}

pub fn touch(conn: &rusqlite::Connection, id: AuthSessionId, now: Timestamp) -> Result<()> {
    conn.execute(
        "UPDATE auth_session SET last_seen_at = ?2 WHERE id = ?1",
        params![id.as_uuid().to_string(), now.to_string()],
    )?;
    Ok(())
}

pub fn rotate_refresh(
    conn: &rusqlite::Connection,
    id: AuthSessionId,
    new_hash: Vec<u8>,
    now: Timestamp,
) -> Result<()> {
    let refresh_expires_at = now
        .checked_add(std::time::Duration::from_secs(
            60 * 60 * REFRESH_WINDOW_HOURS as u64,
        ))
        .map_err(|_| Error::Invariant("refresh_expires_at overflow"))?;
    conn.execute(
        "UPDATE auth_session SET refresh_token_hash = ?2, refresh_expires_at = ?3, last_seen_at = ?4 \
         WHERE id = ?1",
        params![
            id.as_uuid().to_string(),
            new_hash,
            refresh_expires_at.to_string(),
            now.to_string(),
        ],
    )?;
    Ok(())
}

pub fn mark_revoked(conn: &rusqlite::Connection, id: AuthSessionId, now: Timestamp) -> Result<()> {
    conn.execute(
        "UPDATE auth_session SET revoked_at = ?2 WHERE id = ?1",
        params![id.as_uuid().to_string(), now.to_string()],
    )?;
    Ok(())
}

fn row_to_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<AuthSession> {
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
    let user_id: String = row.get(1)?;
    let device_id: String = row.get(2)?;
    let refresh_token_hash: Vec<u8> = row.get(3)?;
    let refresh_expires_at: String = row.get(4)?;
    let absolute_expires_at: String = row.get(5)?;
    let created_at: String = row.get(6)?;
    let last_seen_at: String = row.get(7)?;
    let revoked_at_str: Option<String> = row.get(8)?;
    Ok(AuthSession {
        id: AuthSessionId(parse_uuid(&id)?),
        user_id: UserId(parse_uuid(&user_id)?),
        device_id: WorkstationId(parse_uuid(&device_id)?),
        refresh_token_hash,
        refresh_expires_at: parse_ts(&refresh_expires_at)?,
        absolute_expires_at: parse_ts(&absolute_expires_at)?,
        created_at: parse_ts(&created_at)?,
        last_seen_at: parse_ts(&last_seen_at)?,
        revoked_at: match revoked_at_str {
            None => None,
            Some(s) => Some(parse_ts(&s)?),
        },
    })
}

/// Look up `auth_session` by refresh-token-hash. Used by `auth::refresh`.
pub fn find_by_refresh_hash(
    conn: &rusqlite::Connection,
    refresh_hash: &[u8],
) -> Result<Option<AuthSession>> {
    let row = conn
        .query_row(
            "SELECT id, user_id, device_id, refresh_token_hash, refresh_expires_at, \
                    absolute_expires_at, created_at, last_seen_at, revoked_at \
             FROM auth_session WHERE refresh_token_hash = ?1",
            params![refresh_hash],
            row_to_session,
        )
        .optional()?;
    Ok(row)
}
