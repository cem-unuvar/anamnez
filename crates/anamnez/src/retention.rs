//! `anamnez retention sweep` — invoke `kvkk::retention::sweep` and print the
//! deletion counts. Write subcommand: refuses while `serve` is up.

use crate::cli::{RetentionCmd, RetentionSweepArgs};
use crate::dispatch_common::{load_config, open_db, refuse_while_serve_alive};
use anamnez_core::error::Result;
use anamnez_core::kvkk::retention;

pub fn run(cmd: RetentionCmd) -> Result<()> {
    match cmd {
        RetentionCmd::Sweep(args) => sweep_cmd(args),
    }
}

fn sweep_cmd(args: RetentionSweepArgs) -> Result<()> {
    refuse_while_serve_alive(args.pid_file.as_deref())?;
    let cfg = load_config(&args.config)?;
    let db = open_db(&cfg)?;
    let now = db.clock().now();
    let report = retention::sweep(&db, now)?;

    println!(
        "anamnez retention sweep: started {} → completed {}",
        report.started_at, report.completed_at
    );
    for (table, n) in &report.deleted_by_table {
        println!("  {table}: {n} rows");
    }
    if let Some(ts) = report.high_water_audit_occurred_at {
        println!("  audit high-water: {ts}");
    }
    Ok(())
}
