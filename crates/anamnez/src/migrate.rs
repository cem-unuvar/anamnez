//! `anamnez migrate` — apply pending refinery migrations. Forward-only.

use crate::cli::MigrateArgs;
use crate::dispatch_common::{load_config, refuse_while_serve_alive};
use crate::passphrase;
use anamnez_core::config::Config;
use anamnez_core::db::pragmas;
use anamnez_core::db::{migrations, schema_version};
use anamnez_core::error::{Error, Result};
use rusqlite::{Connection, OpenFlags};
use secrecy::ExposeSecret;

pub fn run(args: MigrateArgs) -> Result<()> {
    refuse_while_serve_alive(args.pid_file.as_deref())?;
    let cfg = load_config(&args.config)?;
    let mut conn = open_writer(&cfg)?;

    let before = migrations::latest_applied(&mut conn)?.unwrap_or(0);
    migrations::apply(&mut conn)?;
    let after = migrations::latest_applied(&mut conn)?.unwrap_or(0);
    schema_version::assert_matches(&mut conn)?;

    if before == after {
        println!("anamnez migrate: no-op (schema version {after})");
    } else {
        println!("anamnez migrate: {before} → {after}");
    }
    Ok(())
}

/// Open a writer that runs migrations without going through `Database::open`'s
/// schema-version assertion (we're the one applying the migrations — assertion
/// would fail before we've run anything new).
fn open_writer(cfg: &Config) -> Result<Connection> {
    let dd = cfg
        .db_path
        .parent()
        .ok_or(Error::Invariant("db_path has no parent directory"))?;
    let pass = passphrase::unwrap_for(dd)?;
    let conn = Connection::open_with_flags(
        &cfg.db_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_URI,
    )?;
    pragmas::apply(&conn, pass.expose_secret())?;
    Ok(conn)
}
