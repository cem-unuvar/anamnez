//! Process-level health probe — shared between the CLI `health` subcommand
//! and the daemon's `/v1/health` route.
//!
//! Checks (in order):
//! 1. DB writer connection responds to `SELECT 1`.
//! 2. Schema version matches `BINARY_SCHEMA_VERSION` (already asserted at open
//!    time; we re-check here as a safety net).
//! 3. At least one code-system row exists (CSV bootstrap or bundle apply has
//!    populated the lookup tables).
//! 4. Audit chain head exists and is parseable (the chain has been seeded —
//!    every bootstrap appends `user.create` so this is true after `init`).

use crate::db::{schema_version, Database};
use crate::error::{Error, Result};
use jiff::Timestamp;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub status: HealthStatus,
    pub db_open: bool,
    pub schema_version: u32,
    pub code_systems_loaded: bool,
    pub audit_chain_head_id: i64,
    pub audit_chain_head_at: Option<Timestamp>,
    pub generated_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Ok,
    Degraded,
}

/// Run the probe. Returns `Ok(HealthReport { status: Degraded, .. })` on any
/// soft failure (code systems absent, etc.); returns `Err` only when the DB
/// handle itself can't be queried.
pub fn probe(db: &Database) -> Result<HealthReport> {
    let now = db.clock().now();
    let mut status = HealthStatus::Ok;

    let db_open = db
        .with_reader(|conn| {
            conn.query_row("SELECT 1", params![], |r| r.get::<_, i64>(0))
                .map(|_| true)
                .map_err(Error::from)
        })
        .unwrap_or(false);
    if !db_open {
        status = HealthStatus::Degraded;
    }

    let schema_v = db
        .with_writer(crate::db::migrations::latest_applied)?
        .unwrap_or(0);
    if schema_v != schema_version::BINARY_SCHEMA_VERSION {
        status = HealthStatus::Degraded;
    }

    let code_systems_loaded = db
        .with_reader(|conn| {
            // SUT and SKRS-VP are the smallest hand-curated lists; either being
            // empty plus everything else empty is the bootstrap gate. We check
            // ICD-10-TM as the canonical "did we load anything" indicator.
            let n: i64 =
                conn.query_row("SELECT COUNT(*) FROM icd10_tm", params![], |r| r.get(0))?;
            Ok(n > 0)
        })
        .unwrap_or(false);
    if !code_systems_loaded {
        status = HealthStatus::Degraded;
    }

    let (audit_chain_head_id, audit_chain_head_at) = db
        .with_reader(|conn| {
            let head: Option<(i64, String)> = conn
                .query_row(
                    "SELECT id, occurred_at FROM audit_log ORDER BY id DESC LIMIT 1",
                    params![],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .ok();
            let ts = head.as_ref().and_then(|(_, s)| s.parse::<Timestamp>().ok());
            Ok((head.map_or(0, |(id, _)| id), ts))
        })
        .unwrap_or((0, None));
    if audit_chain_head_id == 0 {
        status = HealthStatus::Degraded;
    }

    Ok(HealthReport {
        status,
        db_open,
        schema_version: schema_v,
        code_systems_loaded,
        audit_chain_head_id,
        audit_chain_head_at,
        generated_at: now,
    })
}
