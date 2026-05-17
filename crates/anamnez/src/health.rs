//! `anamnez health` — pretty-print `core::health::probe` as JSON. Read
//! subcommand; runs alongside `serve`. Non-zero exit on Degraded.

use crate::cli::HealthArgs;
use crate::dispatch_common::{load_config, open_db};
use anamnez_core::error::{Error, Result};
use anamnez_core::health::{self, HealthStatus};

pub fn run(args: HealthArgs) -> Result<()> {
    let cfg = load_config(&args.config)?;
    let db = open_db(&cfg)?;
    let report = health::probe(&db)?;
    let pretty = serde_json::to_string_pretty(&report)?;
    println!("{pretty}");
    if matches!(report.status, HealthStatus::Degraded) {
        return Err(Error::Invariant("health: degraded"));
    }
    Ok(())
}
