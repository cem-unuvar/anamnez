//! `anamnez admin rotate-server-cert` — heavyweight rotation per SPEC
//! §Deployment. Refuses while `serve` is up (the daemon must be stopped so
//! the new TLS PEMs can be re-read on next start).

use crate::admin::cli_actor;
use crate::cli::AdminRotateServerCertArgs;
use crate::dispatch_common::{data_dir, load_config, open_db, refuse_while_serve_alive};
use anamnez_core::error::Result;
use anamnez_core::tls;

pub fn run(args: AdminRotateServerCertArgs) -> Result<()> {
    refuse_while_serve_alive(args.pid_file.as_deref())?;
    let cfg = load_config(&args.config)?;
    let db = open_db(&cfg)?;
    let admin = cli_actor(&db)?;
    let dd = data_dir(&cfg)?;

    let report = tls::rotate_server_cert(&db, &dd, admin, &args.bind_host)?;
    println!(
        "anamnez admin rotate-server-cert: {} workstation(s) revoked",
        report.workstations_revoked
    );
    println!(
        "  new ca fingerprint (sha256): {}",
        report.new_ca_fingerprint_sha256
    );
    println!("  workstations must re-enroll before they can reconnect");
    Ok(())
}
