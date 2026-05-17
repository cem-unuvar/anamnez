//! `anamnez backup --to <path>` — atomic SQLCipher snapshot. Read subcommand
//! per SPEC §Workspace structure: runs concurrently with `serve`.

use crate::cli::BackupArgs;
use crate::dispatch_common::{data_dir, load_config, open_db};
use crate::passphrase;
use anamnez_core::backup as core_backup;
use anamnez_core::error::Result;

pub fn run(args: BackupArgs) -> Result<()> {
    let cfg = load_config(&args.config)?;
    let db = open_db(&cfg)?;
    let dd = data_dir(&cfg)?;
    let live_pass = passphrase::unwrap_for(&dd)?;
    let report = core_backup::take(&db, &args.to, &live_pass)?;
    println!(
        "anamnez backup: {} pages, {} bytes → {}",
        report.pages_copied,
        report.dst_bytes,
        args.to.display()
    );
    Ok(())
}
