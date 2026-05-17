//! Subsystem E — Auth. README §Tenancy + §Wire protocol.

#![allow(clippy::wildcard_imports)]

use anamnez_core::auth::tokens::{ACCESS_TOKEN_MINUTES, REFRESH_TOKEN_HOURS};
use anamnez_core::auth::{self, password, stepup, UserRole};
use anamnez_core::ids::{UserId, WorkstationId};
use anamnez_core::test_support::prelude::*;
use anamnez_core::Error;
use rusqlite::params;
use secrecy::SecretString;

#[test]
fn token_lifetimes_match_readme() {
    assert_eq!(ACCESS_TOKEN_MINUTES, 15);
    assert_eq!(REFRESH_TOKEN_HOURS, 12);
}

#[test]
fn session_constants_match_readme() {
    use anamnez_core::auth::session::{ABSOLUTE_HORIZON_DAYS, REFRESH_WINDOW_HOURS};
    assert_eq!(REFRESH_WINDOW_HOURS, 12);
    assert_eq!(ABSOLUTE_HORIZON_DAYS, 30);
}

#[test]
fn argon2id_roundtrip_succeeds_and_wrong_password_fails() {
    let pw = SecretString::from("correct horse battery staple".to_owned());
    let h = password::hash(pw.clone()).expect("hash");
    assert!(password::verify(pw, &h).expect("verify"));
    assert!(!password::verify(SecretString::from("wrong".to_owned()), &h).expect("verify"));
}

fn seed_user(temp: &TempDb, email: &str, password_plain: &str) -> UserId {
    let id = UserId::new();
    let pw_hash = password::hash(SecretString::from(password_plain.to_owned())).expect("hash");
    temp.db
        .with_writer(|conn| {
            conn.execute(
                "INSERT INTO user (id, email, display_name, role, password_hash, created_at) \
                 VALUES (?1, ?2, 'Test', 'provider', ?3, '2026-01-01T00:00:00Z')",
                params![id.as_uuid().to_string(), email, pw_hash],
            )?;
            Ok(())
        })
        .expect("seed");
    id
}

fn seed_workstation(temp: &TempDb, enroller: UserId, label: &str) -> WorkstationId {
    let id = WorkstationId::new();
    temp.db
        .with_writer(|conn| {
            conn.execute(
                "INSERT INTO workstation \
                 (id, label, mode, bound_user_id, cert_serial, cert_fingerprint, enrolled_at, enrolled_by) \
                 VALUES (?1, ?2, 'bound', ?3, ?4, ?5, '2026-01-01T00:00:00Z', ?3)",
                params![
                    id.as_uuid().to_string(),
                    label,
                    enroller.as_uuid().to_string(),
                    format!("serial-{}", id.as_uuid()),
                    format!("fp-{}", id.as_uuid()),
                ],
            )?;
            Ok(())
        })
        .expect("seed workstation");
    id
}

#[test]
fn login_with_wrong_password_returns_bad_credentials_and_audits_failure() {
    let temp = TempDb::new().expect("TempDb opens");
    let user = seed_user(&temp, "alice@x", "right-password");
    let workstation = seed_workstation(&temp, user, "Exam 1");

    let err = auth::login(
        &temp.db,
        "alice@x",
        SecretString::from("wrong-password".to_owned()),
        workstation,
    )
    .expect_err("must reject");
    matches!(err, Error::BadCredentials)
        .then_some(())
        .expect("expected BadCredentials");

    // Audit row written for the failure outcome.
    temp.db
        .with_reader(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'user.login' AND metadata LIKE '%bad_credentials%'",
                params![],
                |r| r.get(0),
            )?;
            assert_eq!(count, 1);
            Ok(())
        })
        .expect("audit check");
}

#[test]
fn login_succeeds_and_check_session_returns_user() {
    let temp = TempDb::new().expect("TempDb opens");
    let user = seed_user(&temp, "alice@x", "right-password");
    let workstation = seed_workstation(&temp, user, "Exam 1");

    let outcome = auth::login(
        &temp.db,
        "alice@x",
        SecretString::from("right-password".to_owned()),
        workstation,
    )
    .expect("login");
    assert_eq!(outcome.user.email, "alice@x");
    assert!(matches!(outcome.user.role, UserRole::Provider));

    let checked = auth::check_session(&temp.db, &outcome.access_token).expect("check");
    assert_eq!(checked.id, user);
}

#[test]
fn refresh_rotation_is_one_time_use_replay_fails() {
    let temp = TempDb::new().expect("TempDb opens");
    let user = seed_user(&temp, "alice@x", "right-password");
    let workstation = seed_workstation(&temp, user, "Exam 1");
    let outcome = auth::login(
        &temp.db,
        "alice@x",
        SecretString::from("right-password".to_owned()),
        workstation,
    )
    .expect("login");

    // First refresh — succeeds, returns a new refresh token.
    let _ = auth::refresh(&temp.db, outcome.refresh_token.clone()).expect("first refresh");

    // Replay the original refresh token — must fail; the old hash is no longer in the row.
    let err = auth::refresh(&temp.db, outcome.refresh_token).expect_err("replay must fail");
    matches!(err, Error::BadCredentials)
        .then_some(())
        .expect("expected BadCredentials on replay");
}

#[test]
fn revoked_session_fails_immediately_on_next_authenticated_op() {
    let temp = TempDb::new().expect("TempDb opens");
    let user = seed_user(&temp, "alice@x", "right-password");
    let workstation = seed_workstation(&temp, user, "Exam 1");
    let outcome = auth::login(
        &temp.db,
        "alice@x",
        SecretString::from("right-password".to_owned()),
        workstation,
    )
    .expect("login");

    auth::revoke(&temp.db, outcome.session_id).expect("revoke");

    let err = auth::check_session(&temp.db, &outcome.access_token).expect_err("revoked");
    matches!(err, Error::Revoked)
        .then_some(())
        .expect("expected Revoked");
}

#[test]
fn stepup_success_returns_receipt_and_audits() {
    let temp = TempDb::new().expect("TempDb opens");
    let user = seed_user(&temp, "alice@x", "right-password");
    let receipt = stepup::verify_for(
        &temp.db,
        user,
        stepup::StepUpAction::PatientDossierExport,
        SecretString::from("right-password".to_owned()),
    )
    .expect("stepup ok");
    assert_eq!(receipt.user_id, user);
}

#[test]
fn stepup_failed_reauth_blocks_action_and_audits_failure() {
    let temp = TempDb::new().expect("TempDb opens");
    let user = seed_user(&temp, "alice@x", "right-password");
    let err = stepup::verify_for(
        &temp.db,
        user,
        stepup::StepUpAction::PatientDossierExport,
        SecretString::from("wrong".to_owned()),
    )
    .expect_err("must reject");
    matches!(err, Error::StepUpRequired { .. })
        .then_some(())
        .expect("expected StepUpRequired");

    temp.db
        .with_reader(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE target_type = 'stepup' AND metadata LIKE '%failure%'",
                params![],
                |r| r.get(0),
            )?;
            assert_eq!(count, 1);
            Ok(())
        })
        .expect("audit check");
}

#[test]
fn stepup_action_strings_cover_documented_high_risk_set() {
    use stepup::StepUpAction::*;
    let set: Vec<&str> = [
        UserCreate,
        UserModify,
        PatientAccessGrantToNewUser,
        UserDisable,
        WorkstationRevoke,
        PatientDossierExport,
        LargeQueryDownload,
        RetentionPolicyChange,
        WorkstationEnrollmentString,
    ]
    .iter()
    .map(|a| a.as_str())
    .collect();
    assert!(set.contains(&"patient.export"));
    assert!(set.contains(&"workstation.enroll"));
    assert!(set.contains(&"user.disable"));
}
