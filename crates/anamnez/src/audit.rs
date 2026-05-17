//! `anamnez audit verify` — walk the chain from the latest retention_sweep
//! anchor to head and panic-shaped-exit if any row hash mismatches.

use crate::cli::{AuditCmd, AuditVerifyArgs};
use crate::dispatch_common::{load_config, open_db};
use anamnez_core::audit::verify;
use anamnez_core::error::{Error, Result};

pub fn run(cmd: AuditCmd) -> Result<()> {
    match cmd {
        AuditCmd::Verify(args) => verify_cmd(args),
    }
}

fn verify_cmd(args: AuditVerifyArgs) -> Result<()> {
    let cfg = load_config(&args.config)?;
    let db = open_db(&cfg)?;
    match verify::verify_chain(&db) {
        Ok(report) => {
            println!(
                "anamnez audit verify: OK — {} rows verified up to id {}",
                report.rows_verified, report.last_verified_id
            );
            Ok(())
        }
        Err(Error::AuditTamper { row_id }) => {
            eprintln!("anamnez audit verify: FAIL — tampered at row {row_id}");
            Err(Error::AuditTamper { row_id })
        }
        Err(e) => Err(e),
    }
}
