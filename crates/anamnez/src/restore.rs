//! `anamnez restore --from <path>` — overwrite the live DB with a snapshot.
//! Write subcommand: refuses while `serve` holds the PID file.

use crate::cli::RestoreArgs;
use crate::dispatch_common::{data_dir, load_config, refuse_while_serve_alive};
use crate::passphrase;
use anamnez_core::backup as core_backup;
use anamnez_core::error::Result;

pub fn run(args: RestoreArgs) -> Result<()> {
    refuse_while_serve_alive(args.pid_file.as_deref())?;
    let cfg = load_config(&args.config)?;
    let dd = data_dir(&cfg)?;
    let live_pass = passphrase::unwrap_for(&dd)?;
    let report = core_backup::restore(&args.from, &cfg.db_path, &live_pass)?;
    println!(
        "anamnez restore: {} bytes restored → {}",
        report.bytes_copied,
        cfg.db_path.display()
    );
    Ok(())
}
