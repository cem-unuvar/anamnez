//! `anamnez admin breach-report` — print the breach-scope report as CSV to
//! stdout. Read subcommand: runs concurrently with `serve`.

use crate::cli::AdminBreachReportArgs;
use crate::dispatch_common::{load_config, open_db};
use anamnez_core::audit::BreachScope;
use anamnez_core::error::{Error, Result};
use anamnez_core::ids::{AuthSessionId, UserId};
use anamnez_core::kvkk::breach_report;
use jiff::Timestamp;

pub fn run(args: AdminBreachReportArgs) -> Result<()> {
    let cfg = load_config(&args.config)?;
    let db = open_db(&cfg)?;
    let scope = build_scope(&args)?;
    let rows = breach_report::run(&db, scope)?;

    // CSV header.
    println!("occurred_at,action,patient_id,target_type,target_id");
    for r in &rows {
        let pid = r
            .patient_id
            .map(|p| p.as_uuid().to_string())
            .unwrap_or_default();
        println!(
            "{},{},{},{},{}",
            r.occurred_at,
            r.action.as_str(),
            pid,
            r.target_type,
            r.target_id
        );
    }
    eprintln!("anamnez admin breach-report: {} rows", rows.len());
    Ok(())
}

fn build_scope(args: &AdminBreachReportArgs) -> Result<BreachScope> {
    match (&args.session, &args.user, &args.since, &args.until) {
        (Some(s), None, _, _) => Ok(BreachScope::BySession(AuthSessionId(parse_uuid(s)?))),
        (None, Some(u), Some(since), Some(until)) => {
            let since_ts: Timestamp = since
                .parse()
                .map_err(|_| Error::Invariant("--since not parseable as RFC 3339"))?;
            let until_ts: Timestamp = until
                .parse()
                .map_err(|_| Error::Invariant("--until not parseable as RFC 3339"))?;
            Ok(BreachScope::ByUser {
                user_id: UserId(parse_uuid(u)?),
                from: since_ts,
                until: until_ts,
            })
        }
        _ => Err(Error::Invariant(
            "breach-report: specify --session OR (--user --since --until)",
        )),
    }
}

fn parse_uuid(s: &str) -> Result<uuid::Uuid> {
    uuid::Uuid::parse_str(s).map_err(|_| Error::Invariant("argument is not a UUID"))
}
