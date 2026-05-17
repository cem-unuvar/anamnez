//! `anamnez admin <subcommand>` — operator surface for user/workstation/breach
//! management plus heavyweight maintenance (cert rotation, bundle apply).

pub mod add_user;
pub mod breach_report;
pub mod disable_user;
pub mod enroll_workstation;
pub mod reset_password;
pub mod rotate_server_cert;
pub mod update_codesystems;

use crate::cli::AdminCmd;
use anamnez_core::error::Result;

pub fn run(cmd: AdminCmd) -> Result<()> {
    match cmd {
        AdminCmd::AddUser(args) => add_user::run(args),
        AdminCmd::DisableUser(args) => disable_user::run(args),
        AdminCmd::ResetPassword(args) => reset_password::run(args),
        AdminCmd::EnrollWorkstation(args) => enroll_workstation::run(args),
        AdminCmd::BreachReport(args) => breach_report::run(args),
        AdminCmd::RotateServerCert(args) => rotate_server_cert::run(args),
        AdminCmd::UpdateCodesystems(args) => update_codesystems::run(args),
    }
}

/// Resolve the singleton admin user the CLI acts as.
///
/// Operator subcommands like `add-user`, `enroll-workstation`, etc. need an
/// `actor` UUID to attribute the audit row to. The CLI is invoked on the Mac
/// Studio by the local operator and bypasses HTTP auth; there is no
/// per-invocation user identity. We attribute every CLI write to the first
/// admin user — the one minted by `init` — and audit accordingly.
pub fn cli_actor(db: &anamnez_core::db::Database) -> Result<anamnez_core::ids::UserId> {
    db.with_reader(|conn| {
        let id_s: String = conn.query_row(
            "SELECT id FROM user WHERE role = 'admin' AND disabled_at IS NULL \
             ORDER BY created_at ASC LIMIT 1",
            rusqlite::params![],
            |r| r.get(0),
        )?;
        let uuid = uuid::Uuid::parse_str(&id_s)
            .map_err(|_| anamnez_core::error::Error::Invariant("user.id not a UUID"))?;
        Ok(anamnez_core::ids::UserId(uuid))
    })
}
