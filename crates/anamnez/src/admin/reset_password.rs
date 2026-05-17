//! `anamnez admin reset-password` — set a user's password and revoke their
//! active sessions. Selectable by `--user <uuid>` or `--email <addr>`.

use crate::admin::cli_actor;
use crate::cli::AdminResetPasswordArgs;
use crate::dispatch_common::{load_config, open_db, refuse_while_serve_alive};
use anamnez_core::error::{Error, Result};
use anamnez_core::ids::UserId;
use anamnez_core::user;
use secrecy::SecretString;
use std::io::{BufRead, Write};

pub fn run(args: AdminResetPasswordArgs) -> Result<()> {
    refuse_while_serve_alive(args.pid_file.as_deref())?;
    let cfg = load_config(&args.config)?;
    let db = open_db(&cfg)?;
    let admin = cli_actor(&db)?;

    let target: UserId = match (args.user.as_deref(), args.email.as_deref()) {
        (Some(s), _) => {
            UserId(uuid::Uuid::parse_str(s).map_err(|_| Error::Invariant("--user is not a UUID"))?)
        }
        (None, Some(email)) => user::find_by_email(&db, email)?.ok_or(Error::NotFound)?.id,
        _ => return Err(Error::Invariant("specify --user or --email")),
    };

    let new_password = read_password(args.password_stdin)?;
    user::reset_password(&db, admin, target, new_password)?;
    println!(
        "anamnez admin reset-password: ok ({}) — sessions revoked",
        target.as_uuid()
    );
    Ok(())
}

fn read_password(stdin_mode: bool) -> Result<SecretString> {
    if stdin_mode {
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line)?;
        let t = line.trim_end_matches(['\n', '\r']).to_owned();
        if t.is_empty() {
            return Err(Error::Invariant("reset-password: empty password on stdin"));
        }
        return Ok(SecretString::from(t));
    }
    print!("New password: ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    let t = line.trim_end_matches(['\n', '\r']).to_owned();
    if t.is_empty() {
        return Err(Error::Invariant("reset-password: empty password"));
    }
    Ok(SecretString::from(t))
}
