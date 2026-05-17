//! `anamnez admin add-user` — create a `provider` or `admin`-role user.

use crate::admin::cli_actor;
use crate::cli::AdminAddUserArgs;
use crate::dispatch_common::{load_config, open_db, refuse_while_serve_alive};
use anamnez_core::auth::UserRole;
use anamnez_core::error::{Error, Result};
use anamnez_core::user;
use secrecy::SecretString;
use std::io::{BufRead, Write};

pub fn run(args: AdminAddUserArgs) -> Result<()> {
    refuse_while_serve_alive(args.pid_file.as_deref())?;
    let cfg = load_config(&args.config)?;
    let db = open_db(&cfg)?;
    let role = UserRole::parse(&args.role)?;
    let password = read_password(args.password_stdin)?;
    let admin = cli_actor(&db)?;
    let u = user::create(
        &db,
        admin,
        user::NewUser {
            email: args.email.clone(),
            display_name: args.display_name.clone(),
            role,
            password,
        },
    )?;
    println!(
        "anamnez admin add-user: id={} email={} role={}",
        u.id.as_uuid(),
        u.email,
        u.role.as_str()
    );
    Ok(())
}

fn read_password(stdin_mode: bool) -> Result<SecretString> {
    if stdin_mode {
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line)?;
        let t = line.trim_end_matches(['\n', '\r']).to_owned();
        if t.is_empty() {
            return Err(Error::Invariant("add-user: empty password on stdin"));
        }
        return Ok(SecretString::from(t));
    }
    print!("New user password: ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    let t = line.trim_end_matches(['\n', '\r']).to_owned();
    if t.is_empty() {
        return Err(Error::Invariant("add-user: empty password"));
    }
    Ok(SecretString::from(t))
}
