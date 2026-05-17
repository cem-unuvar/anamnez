//! Subsystem A — Storage substrate. README §Storage → Engine; §Privacy.

#![allow(clippy::wildcard_imports)]

use anamnez_core::env::Environment;
use anamnez_core::test_support::prelude::*;

#[test]
fn refinery_applies_to_empty_db() {
    let _db = TempDb::new().expect("TempDb opens cleanly on fresh tempdir");
}

#[test]
fn pragmas_assert_wal_foreign_keys_on() {
    let temp = TempDb::new().expect("TempDb opens");
    temp.db
        .with_reader(|conn| {
            let mode: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
            assert!(
                mode.eq_ignore_ascii_case("wal"),
                "journal_mode = {mode}, want WAL"
            );
            let fk: i64 = conn.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
            assert_eq!(fk, 1, "foreign_keys must be ON");
            Ok(())
        })
        .expect("pragmas assert");
}

#[test]
fn every_user_table_is_strict() {
    let temp = TempDb::new().expect("TempDb opens");
    temp.db
        .with_reader(|conn| {
            let mut stmt = conn.prepare(
                "SELECT name, sql FROM sqlite_schema \
                 WHERE type = 'table' AND name NOT LIKE 'sqlite_%' \
                   AND name NOT LIKE 'refinery_%' \
                   AND name NOT LIKE 'fts_%' \
                   AND sql IS NOT NULL",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    let name: String = row.get(0)?;
                    let sql: String = row.get(1)?;
                    Ok((name, sql))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            assert!(!rows.is_empty(), "expected user tables present");
            for (name, sql) in rows {
                assert!(
                    sql.to_ascii_uppercase().contains("STRICT"),
                    "table {name} is not STRICT"
                );
            }
            Ok(())
        })
        .expect("strict table check");
}

#[test]
fn test_daemon_writes_test_marker_on_first_boot() {
    let temp = TempDb::new_with(
        Environment::Test,
        std::sync::Arc::new(anamnez_core::time::SystemClock),
    )
    .expect("TempDb opens in test mode");
    temp.db
        .with_reader(|conn| {
            let marker: String = conn.query_row(
                "SELECT environment FROM environment_marker WHERE singleton = 1",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(marker, "test");
            Ok(())
        })
        .expect("env marker read");
}

#[test]
fn production_daemon_refuses_test_marked_db() {
    use anamnez_core::Error;

    let temp = TempDb::new().expect("first open in Test mode");
    let path = temp.path().to_owned();

    // Close the test-mode DB by dropping it but keep the file behind.
    let root = temp.root;
    drop(temp.db);

    // Try to reopen as Production — must refuse.
    let pass =
        secrecy::SecretString::from("anamnez-test-passphrase-deterministic-7f9e1b3a".to_owned());
    let err = anamnez_core::db::Database::open(&path, pass, Environment::Production)
        .err()
        .expect("production daemon must refuse test-marked DB");
    matches!(err, Error::EnvironmentMarkerMismatch { .. })
        .then_some(())
        .expect("expected EnvironmentMarkerMismatch");

    // Keep `root` alive to ensure the file isn't dropped before the assertion.
    drop(root);
}
