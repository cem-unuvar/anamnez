//! Breach scope report (KVKK m. 12/5 + 2019/10 72-hour Kurul notification).

pub use crate::audit::{BreachReportRow, BreachScope};

use crate::audit;
use crate::db::Database;
use crate::error::Result;

/// Thin wrapper over `audit::breach_report` exposing it under the `kvkk` namespace.
pub fn run(db: &Database, scope: BreachScope) -> Result<Vec<BreachReportRow>> {
    audit::breach_report(db, scope)
}
