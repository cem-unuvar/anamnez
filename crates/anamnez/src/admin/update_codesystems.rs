//! `anamnez admin update-codesystems --from <path>` — verify + apply a signed
//! bundle. Online-pull path is deferred (distribution-host URL is not yet
//! defined in SPEC).

use crate::cli::AdminUpdateCodesystemsArgs;
use crate::dispatch_common::{load_config, refuse_while_serve_alive};
use crate::passphrase;
use anamnez_core::code_systems::bundle;
use anamnez_core::db::pragmas;
use anamnez_core::error::{Error, Result};
use rusqlite::{Connection, OpenFlags};
use secrecy::ExposeSecret;

pub fn run(args: AdminUpdateCodesystemsArgs) -> Result<()> {
    refuse_while_serve_alive(args.pid_file.as_deref())?;
    let cfg = load_config(&args.config)?;
    let dd = cfg
        .db_path
        .parent()
        .ok_or(Error::Invariant("db_path has no parent"))?;
    let pass = passphrase::unwrap_for(dd)?;

    let mut conn = Connection::open_with_flags(
        &cfg.db_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_URI,
    )?;
    pragmas::apply(&conn, pass.expose_secret())?;

    let report = bundle::apply(&mut conn, &args.from)?;
    println!(
        "anamnez admin update-codesystems: inserted={} updated={} retired={}",
        report.inserted, report.updated, report.retired
    );
    if let Some(m) = report.manifest {
        println!("  bundle version: {}", m.version);
        println!("  bundle built_at: {}", m.built_at);
    }
    Ok(())
}
