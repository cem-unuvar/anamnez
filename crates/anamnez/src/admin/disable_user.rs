//! `anamnez admin disable-user` — refuses unless every sole-owned patient has
//! a `--successor` mapping. Then transfers ownership and sets `disabled_at`.

use crate::admin::cli_actor;
use crate::cli::AdminDisableUserArgs;
use crate::dispatch_common::{load_config, open_db, refuse_while_serve_alive};
use anamnez_core::error::{Error, Result};
use anamnez_core::ids::{PatientId, UserId};
use anamnez_core::kvkk::ownership_transfer;

pub fn run(args: AdminDisableUserArgs) -> Result<()> {
    refuse_while_serve_alive(args.pid_file.as_deref())?;
    let cfg = load_config(&args.config)?;
    let db = open_db(&cfg)?;
    let admin = cli_actor(&db)?;
    let target = UserId(parse_uuid(&args.user)?);

    let mut successors: Vec<(PatientId, UserId)> = Vec::with_capacity(args.successors.len());
    for (p, u) in &args.successors {
        successors.push((PatientId(parse_uuid(p)?), UserId(parse_uuid(u)?)));
    }

    ownership_transfer::disable_user_with_successors(&db, admin, target, successors)?;
    println!("anamnez admin disable-user: ok ({})", args.user);
    Ok(())
}

fn parse_uuid(s: &str) -> Result<uuid::Uuid> {
    uuid::Uuid::parse_str(s).map_err(|_| Error::Invariant("argument is not a UUID"))
}
