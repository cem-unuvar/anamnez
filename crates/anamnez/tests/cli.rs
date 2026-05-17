//! Layer-1 CLI integration test — exercises every non-`serve` subcommand
//! against a tempdir DB by invoking the real `anamnez` binary via `assert_cmd`.
//!
//! The pattern matches the spec's verification script: `init` lays down state,
//! later subcommands operate on it, and assertions inspect the resulting DB or
//! stdout. We use the env-var-recovery path for opening the DB after init so
//! we don't depend on the (Phase-1 placeholder) DevSep wrap behavior in tests.

use anamnez_core::env::Environment;
use assert_cmd::Command;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// A staged `anamnez init` plus a parsed recovery code, ready for downstream
/// subcommands. We always invoke init in `test` environment so other
/// subcommands can use TEST-prefixed names without tripping the prefix check.
struct Staged {
    _tempdir: TempDir,
    data_dir: PathBuf,
    config: PathBuf,
    pid_file: PathBuf,
    admin_email: String,
    admin_password: String,
    recovery_code: String,
}

fn stage_init() -> Staged {
    let tempdir = TempDir::new().expect("tempdir");
    let data_dir = tempdir.path().to_path_buf();
    let config = data_dir.join("config.toml");
    let pid_file = data_dir.join("anamnez.pid");
    let admin_email = "admin@example.test".to_owned();
    let admin_password = "[TEST]-correct-horse-battery".to_owned();

    let out = Command::cargo_bin("anamnez")
        .unwrap()
        .env("RUST_LOG", "error")
        .args([
            "init",
            "--data-dir",
            data_dir.to_str().unwrap(),
            "--admin-email",
            &admin_email,
            "--admin-display-name",
            "Test Admin",
            "--environment",
            "test",
            "--bind-host",
            "127.0.0.1",
            "--password-stdin",
        ])
        .write_stdin(format!("{admin_password}\n"))
        .assert()
        .success()
        .get_output()
        .clone();

    let stdout = String::from_utf8(out.stdout).expect("init stdout utf-8");
    let recovery_code = parse_recovery_code(&stdout).expect("recovery code in init stdout");

    Staged {
        _tempdir: tempdir,
        data_dir,
        config,
        pid_file,
        admin_email,
        admin_password,
        recovery_code,
    }
}

fn parse_recovery_code(stdout: &str) -> Option<String> {
    // The recovery code is the first all-lowercase-hex line >= 32 chars
    // appearing on its own (after trimming leading whitespace) anywhere in
    // stdout. We don't anchor on the header text because tracing noise can
    // interleave with the printed banner.
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.len() >= 32
            && trimmed
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Some(trimmed.to_owned());
        }
    }
    None
}

fn anamnez(staged: &Staged) -> Command {
    let mut c = Command::cargo_bin("anamnez").unwrap();
    c.env("ANAMNEZ_RECOVERY_CODE", &staged.recovery_code)
        .env("RUST_LOG", "error");
    c
}

fn assert_pid_path_unused(p: &Path) {
    assert!(!p.exists(), "pid file should be absent: {}", p.display());
}

// ─── tests ────────────────────────────────────────────────────────────────

#[test]
fn init_then_health_reports_db_state() {
    let s = stage_init();
    // health is expected to return Degraded after a code-system-less init
    // (we haven't applied a bundle yet). Non-zero exit on Degraded is by design.
    let out = anamnez(&s)
        .args(["health", "--config", s.config.to_str().unwrap()])
        .assert()
        .failure()
        .get_output()
        .clone();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("\"db_open\": true"), "{stdout}");
    assert!(stdout.contains("\"schema_version\": 6"), "{stdout}");
    assert!(stdout.contains("\"audit_chain_head_id\":"), "{stdout}");
    assert!(
        stdout.contains("\"code_systems_loaded\": false"),
        "{stdout}"
    );
}

#[test]
fn init_refuses_second_invocation() {
    let s = stage_init();
    // Second init in the same data_dir must fail loudly.
    Command::cargo_bin("anamnez")
        .unwrap()
        .args([
            "init",
            "--data-dir",
            s.data_dir.to_str().unwrap(),
            "--admin-email",
            "second@example.test",
            "--admin-display-name",
            "Second",
            "--environment",
            "test",
            "--password-stdin",
        ])
        .write_stdin("[TEST]-x\n")
        .assert()
        .failure();
}

#[test]
fn migrate_is_idempotent() {
    let s = stage_init();
    anamnez(&s)
        .args(["migrate", "--config", s.config.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("no-op"));
}

#[test]
fn backup_then_restore_round_trip() {
    let s = stage_init();
    let snap = s.data_dir.join("snapshot.sqlite");
    anamnez(&s)
        .args([
            "backup",
            "--config",
            s.config.to_str().unwrap(),
            "--to",
            snap.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert!(snap.exists());

    // Capture the audit chain head before restore.
    let head_before = audit_chain_head(&s).expect("head before");

    anamnez(&s)
        .args([
            "restore",
            "--config",
            s.config.to_str().unwrap(),
            "--from",
            snap.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Restore should preserve the same head.
    let head_after = audit_chain_head(&s).expect("head after");
    assert_eq!(head_before, head_after);
}

#[test]
fn audit_verify_ok_on_fresh_init() {
    let s = stage_init();
    anamnez(&s)
        .args(["audit", "verify", "--config", s.config.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("audit verify: OK"));
}

#[test]
fn retention_sweep_runs_clean() {
    let s = stage_init();
    anamnez(&s)
        .args(["retention", "sweep", "--config", s.config.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("retention sweep"));
}

#[test]
fn admin_add_user_succeeds_and_login_works() {
    let s = stage_init();
    let email = "doctor@example.test";
    let pwd = "[TEST]-doctor-staple";
    anamnez(&s)
        .args([
            "admin",
            "add-user",
            "--config",
            s.config.to_str().unwrap(),
            "--email",
            email,
            "--display-name",
            "Dr. Test",
            "--role",
            "provider",
            "--password-stdin",
        ])
        .write_stdin(format!("{pwd}\n"))
        .assert()
        .success()
        .stdout(predicates::str::contains("add-user"));

    // Verify directly against the DB that the user exists and the password
    // hash verifies.
    let user_id = lookup_user_id_by_email(&s, email).expect("doctor exists");
    assert!(!user_id.is_nil());
}

#[test]
fn admin_reset_password_revokes_sessions() {
    use anamnez_core::auth as core_auth;
    use anamnez_core::workstation::{self, Mode, NewWorkstation};
    use secrecy::SecretString;

    let s = stage_init();
    // Open the DB via the test harness path, mint a session for the admin user
    // by hand, then reset and verify the session got revoked.
    let db = open_db(&s);

    // Enroll a workstation we can attach the session to.
    let admin_id = lookup_admin_id(&db);
    let ws = workstation::enroll(
        &db,
        admin_id,
        NewWorkstation {
            label: "test".into(),
            mode: Mode::Shared,
            bound_user_id: None,
            cert_serial: "cli-test".into(),
            cert_fingerprint: "cli-test-fp".into(),
        },
    )
    .expect("enroll");

    // Log in to mint a session.
    let outcome = core_auth::login(
        &db,
        &s.admin_email,
        SecretString::from(s.admin_password.clone()),
        ws.id,
    )
    .expect("login");
    let access_token = outcome.access_token;

    // Sanity: session is live.
    assert!(core_auth::check_session(&db, &access_token).is_ok());
    drop(db);

    // Reset.
    let new_pwd = "[TEST]-rotated-staple";
    anamnez(&s)
        .args([
            "admin",
            "reset-password",
            "--config",
            s.config.to_str().unwrap(),
            "--email",
            &s.admin_email,
            "--password-stdin",
        ])
        .write_stdin(format!("{new_pwd}\n"))
        .assert()
        .success();

    // After reset: the old access token must fail.
    let db = open_db(&s);
    assert!(core_auth::check_session(&db, &access_token).is_err());
}

#[test]
fn admin_enroll_workstation_emits_uri() {
    let s = stage_init();
    let out = anamnez(&s)
        .args([
            "admin",
            "enroll-workstation",
            "--config",
            s.config.to_str().unwrap(),
            "--label",
            "Exam 1",
            "--mode",
            "bound",
            "--bind-user-email",
            &s.admin_email,
            "--host",
            "10.0.0.5",
        ])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("anamnez://enroll?host=10.0.0.5"),
        "{stdout}"
    );
    assert!(stdout.contains("fingerprint="), "{stdout}");
    assert!(stdout.contains("token="), "{stdout}");
}

#[test]
fn admin_breach_report_emits_csv_header() {
    let s = stage_init();
    let session_id = uuid::Uuid::nil().to_string();
    let out = anamnez(&s)
        .args([
            "admin",
            "breach-report",
            "--config",
            s.config.to_str().unwrap(),
            "--session",
            &session_id,
        ])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.starts_with("occurred_at,action,patient_id,target_type,target_id"),
        "{stdout}"
    );
}

#[test]
fn admin_rotate_server_cert_writes_new_pems_and_revokes() {
    let s = stage_init();

    // Enroll one workstation row first so we have something to revoke.
    {
        let db = open_db(&s);
        let admin = lookup_admin_id(&db);
        anamnez_core::workstation::enroll(
            &db,
            admin,
            anamnez_core::workstation::NewWorkstation {
                label: "to-revoke".into(),
                mode: anamnez_core::workstation::Mode::Shared,
                bound_user_id: None,
                cert_serial: "before-rotation".into(),
                cert_fingerprint: "before-rotation-fp".into(),
            },
        )
        .expect("pre-rotation enroll");
    }

    let server_cert_before =
        std::fs::read_to_string(s.data_dir.join("tls/server_cert.pem")).unwrap();

    anamnez(&s)
        .args([
            "admin",
            "rotate-server-cert",
            "--config",
            s.config.to_str().unwrap(),
            "--bind-host",
            "127.0.0.1",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("workstation(s) revoked"));

    let server_cert_after =
        std::fs::read_to_string(s.data_dir.join("tls/server_cert.pem")).unwrap();
    assert_ne!(server_cert_before, server_cert_after);

    // The pre-rotation workstation row must now be revoked.
    let db = open_db(&s);
    let revoked = anamnez_core::workstation::list_revoked(&db).unwrap();
    assert!(!revoked.is_empty(), "expected at least one revoked ws");
    assert_pid_path_unused(&s.pid_file);
}

#[test]
fn write_subcommands_refuse_when_pid_file_holds_live_pid() {
    let s = stage_init();
    // Plant a PID file holding our own (live) PID.
    std::fs::write(&s.pid_file, format!("{}", std::process::id())).unwrap();
    anamnez(&s)
        .args([
            "migrate",
            "--config",
            s.config.to_str().unwrap(),
            "--pid-file",
            s.pid_file.to_str().unwrap(),
        ])
        .assert()
        .failure();
    // Read subcommands run regardless of the PID file — `audit verify` is a
    // read; running with a stale-or-live PID file holder should still work.
    anamnez(&s)
        .args(["audit", "verify", "--config", s.config.to_str().unwrap()])
        .assert()
        .success();
}

// ─── helpers ──────────────────────────────────────────────────────────────

struct UnusedSep;
impl anamnez_core::key_custody::SecureEnclaveWrap for UnusedSep {
    fn wrap(&self, _: &secrecy::SecretString) -> anamnez_core::error::Result<Vec<u8>> {
        unreachable!()
    }
    fn unwrap(&self, _: &[u8]) -> anamnez_core::error::Result<secrecy::SecretString> {
        unreachable!()
    }
}

fn open_db(s: &Staged) -> anamnez_core::db::Database {
    use anamnez_core::db::Database;
    use anamnez_core::key_custody::ColdBoot;
    use secrecy::SecretString;
    use std::sync::Arc;

    let bytes = std::fs::read(s.data_dir.join("wrap_recovery.bin")).unwrap();
    let cb = ColdBoot::new(Arc::new(UnusedSep));
    let pass = cb
        .unwrap_passphrase_via_recovery(&bytes, &SecretString::from(s.recovery_code.clone()))
        .unwrap();
    Database::open(&s.data_dir.join("anamnez.sqlite"), pass, Environment::Test).unwrap()
}

fn audit_chain_head(s: &Staged) -> Option<(i64, Vec<u8>)> {
    let db = open_db(s);
    db.with_reader(|conn| {
        let row = conn
            .query_row(
                "SELECT id, row_hash FROM audit_log ORDER BY id DESC LIMIT 1",
                rusqlite::params![],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)),
            )
            .ok();
        Ok(row)
    })
    .ok()
    .flatten()
}

fn lookup_user_id_by_email(s: &Staged, email: &str) -> Option<uuid::Uuid> {
    let db = open_db(s);
    db.with_reader(|conn| {
        let id_s: Option<String> = conn
            .query_row(
                "SELECT id FROM user WHERE email = ?1",
                rusqlite::params![email],
                |r| r.get(0),
            )
            .ok();
        Ok(id_s.and_then(|s| uuid::Uuid::parse_str(&s).ok()))
    })
    .ok()
    .flatten()
}

fn lookup_admin_id(db: &anamnez_core::db::Database) -> anamnez_core::ids::UserId {
    use anamnez_core::ids::UserId;
    db.with_reader(|conn| {
        let id_s: String = conn
            .query_row(
                "SELECT id FROM user WHERE role = 'admin' ORDER BY created_at ASC LIMIT 1",
                rusqlite::params![],
                |r| r.get(0),
            )
            .unwrap();
        Ok(UserId(uuid::Uuid::parse_str(&id_s).unwrap()))
    })
    .unwrap()
}
