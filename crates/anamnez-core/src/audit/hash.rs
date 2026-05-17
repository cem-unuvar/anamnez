//! README §Storage → Audit log integrity — row hash composition.
//!
//! `row_hash = SHA256(prev_hash || 0x1f || id_be || 0x1f || occurred_at || 0x1f ||
//!                    actor || 0x1f || session || 0x1f || action || 0x1f ||
//!                    target_type || 0x1f || target_id || 0x1f || patient_id || 0x1f ||
//!                    canonical(metadata))`
//!
//! UUIDs and absent IDs serialize as their `to_string()` / empty representations.

use crate::audit::canonical;
use crate::audit::{Action, AuditLogRow};
use crate::ids::{AuditLogId, AuthSessionId, PatientId, UserId};
use jiff::Timestamp;
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};

/// Length of the row hash in bytes.
pub const HASH_LEN: usize = 32;

/// The all-zero `prev_hash` written on the first audit row.
pub const GENESIS_PREV: [u8; HASH_LEN] = [0u8; HASH_LEN];

const SEP: u8 = 0x1f;

/// Compute the row hash for an audit row.
pub fn compute(
    prev_hash: &[u8],
    id: AuditLogId,
    occurred_at: Timestamp,
    actor: Option<UserId>,
    session: Option<AuthSessionId>,
    action: Action,
    target_type: &str,
    target_id: &str,
    patient_id: Option<PatientId>,
    metadata: &JsonValue,
) -> [u8; HASH_LEN] {
    let mut h = Sha256::new();
    h.update(prev_hash);
    h.update([SEP]);
    h.update(id.as_i64().to_be_bytes());
    h.update([SEP]);
    h.update(occurred_at.to_string().as_bytes());
    h.update([SEP]);
    h.update(
        actor
            .map(|u| u.as_uuid().to_string())
            .unwrap_or_default()
            .as_bytes(),
    );
    h.update([SEP]);
    h.update(
        session
            .map(|s| s.as_uuid().to_string())
            .unwrap_or_default()
            .as_bytes(),
    );
    h.update([SEP]);
    h.update(action.as_str().as_bytes());
    h.update([SEP]);
    h.update(target_type.as_bytes());
    h.update([SEP]);
    h.update(target_id.as_bytes());
    h.update([SEP]);
    h.update(
        patient_id
            .map(|p| p.as_uuid().to_string())
            .unwrap_or_default()
            .as_bytes(),
    );
    h.update([SEP]);
    h.update(canonical::to_canonical_bytes(metadata));
    let out = h.finalize();
    let mut arr = [0u8; HASH_LEN];
    arr.copy_from_slice(&out);
    arr
}

/// Re-derive the row hash from a row's stored fields. Used by chain verification.
#[must_use]
pub fn recompute(row: &AuditLogRow) -> [u8; HASH_LEN] {
    compute(
        &row.prev_hash,
        row.id,
        row.occurred_at,
        row.actor_user_id,
        row.auth_session_id,
        row.action,
        &row.target_type,
        &row.target_id,
        row.patient_id,
        &row.metadata,
    )
}
