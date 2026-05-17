//! Startup chain verification (README §Storage → Audit log integrity).
//!
//! Walks `audit_log` from the most recent `retention_sweep` row forward, recomputing
//! `row_hash` against the stored `prev_hash`. On first mismatch, returns
//! `Error::AuditTamper { row_id }`. The daemon panics on this error at startup.

use super::hash::{compute as compute_hash, GENESIS_PREV, HASH_LEN};
use super::{Action, AuditLogId};
use crate::db::Database;
use crate::error::{Error, Result};
use crate::ids::{AuthSessionId, PatientId, UserId};
use jiff::Timestamp;
use rusqlite::params;

#[derive(Debug, Clone)]
pub struct VerifyReport {
    pub rows_verified: u64,
    pub last_verified_id: i64,
}

/// Verify the chain from the most recent `retention_sweep` row to head.
pub fn verify_chain(db: &Database) -> Result<VerifyReport> {
    db.with_reader(|conn| {
        // Find the most recent retention_sweep row id; verification starts from there.
        let start_id: i64 = conn.query_row(
            "SELECT COALESCE((SELECT MAX(id) FROM audit_log WHERE action = 'retention_sweep'), 0)",
            params![],
            |r| r.get(0),
        )?;

        // Pull rows from start onward in chain order.
        let mut stmt = conn.prepare(
            "SELECT id, occurred_at, actor_user_id, auth_session_id, action, target_type, \
                    target_id, patient_id, metadata, prev_hash, row_hash \
             FROM audit_log WHERE id >= ?1 ORDER BY id ASC",
        )?;
        let mut rows = stmt.query(params![start_id])?;

        let mut expected_prev: [u8; HASH_LEN] = if start_id == 0 {
            GENESIS_PREV
        } else {
            // The retention_sweep row itself anchors the chain — its row_hash becomes
            // the expected prev for the row that follows. Read its row_hash directly.
            let h: Vec<u8> = conn.query_row(
                "SELECT row_hash FROM audit_log WHERE id = ?1",
                params![start_id],
                |r| r.get(0),
            )?;
            let mut arr = [0u8; HASH_LEN];
            if h.len() != HASH_LEN {
                return Err(Error::AuditTamper { row_id: start_id });
            }
            arr.copy_from_slice(&h);
            arr
        };

        let mut rows_verified: u64 = 0;
        let mut last_verified_id: i64 = start_id;

        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            // Skip the anchor row itself in the body of the verification loop — we
            // already trust its stored row_hash to seed `expected_prev` above.
            if id == start_id && start_id != 0 {
                continue;
            }

            let occurred_at_str: String = row.get(1)?;
            let actor_str: Option<String> = row.get(2)?;
            let session_str: Option<String> = row.get(3)?;
            let action_str: String = row.get(4)?;
            let target_type: String = row.get(5)?;
            let target_id: String = row.get(6)?;
            let patient_str: Option<String> = row.get(7)?;
            let metadata_str: String = row.get(8)?;
            let prev_hash: Vec<u8> = row.get(9)?;
            let stored_row_hash: Vec<u8> = row.get(10)?;

            // `prev_hash` must equal what the previous row left as `expected_prev`.
            if prev_hash.as_slice() != expected_prev.as_slice() {
                return Err(Error::AuditTamper { row_id: id });
            }

            let occurred_at: Timestamp = occurred_at_str
                .parse()
                .map_err(|_| Error::AuditTamper { row_id: id })?;
            let actor = parse_user_id(actor_str.as_deref(), id)?;
            let session = parse_session_id(session_str.as_deref(), id)?;
            let patient = parse_patient_id(patient_str.as_deref(), id)?;
            let action: Action = serde_json::from_str(&format!("\"{action_str}\""))
                .map_err(|_| Error::AuditTamper { row_id: id })?;
            let metadata: serde_json::Value = serde_json::from_str(&metadata_str)
                .map_err(|_| Error::AuditTamper { row_id: id })?;

            let recomputed = compute_hash(
                &prev_hash,
                AuditLogId(id),
                occurred_at,
                actor,
                session,
                action,
                &target_type,
                &target_id,
                patient,
                &metadata,
            );

            if stored_row_hash.as_slice() != recomputed.as_slice() {
                return Err(Error::AuditTamper { row_id: id });
            }

            expected_prev = recomputed;
            rows_verified += 1;
            last_verified_id = id;
        }

        Ok(VerifyReport {
            rows_verified,
            last_verified_id,
        })
    })
}

fn parse_user_id(s: Option<&str>, row_id: i64) -> Result<Option<UserId>> {
    match s {
        None => Ok(None),
        Some(s) => {
            let u = uuid::Uuid::parse_str(s).map_err(|_| Error::AuditTamper { row_id })?;
            Ok(Some(UserId(u)))
        }
    }
}

fn parse_session_id(s: Option<&str>, row_id: i64) -> Result<Option<AuthSessionId>> {
    match s {
        None => Ok(None),
        Some(s) => {
            let u = uuid::Uuid::parse_str(s).map_err(|_| Error::AuditTamper { row_id })?;
            Ok(Some(AuthSessionId(u)))
        }
    }
}

fn parse_patient_id(s: Option<&str>, row_id: i64) -> Result<Option<PatientId>> {
    match s {
        None => Ok(None),
        Some(s) => {
            let u = uuid::Uuid::parse_str(s).map_err(|_| Error::AuditTamper { row_id })?;
            Ok(Some(PatientId(u)))
        }
    }
}
