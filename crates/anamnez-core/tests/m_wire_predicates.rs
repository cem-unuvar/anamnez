//! Subsystem M — Wire-layer predicates owned by core. README §Workstation client → Wire protocol.

#![allow(clippy::wildcard_imports)]

use anamnez_core::auth::client_version::Version;
use anamnez_core::auth::stepup::StepUpAction;
use anamnez_core::auth::{self, password};
use anamnez_core::ids::{UserId, WorkstationId};
use anamnez_core::test_support::prelude::*;
use anamnez_core::wire::predicates;
use anamnez_core::Error;
use rusqlite::params;
use secrecy::SecretString;

fn seed_user(temp: &TempDb, email: &str, pw: &str) -> UserId {
    let id = UserId::new();
    let h = password::hash(SecretString::from(pw.to_owned())).expect("hash");
    temp.db
        .with_writer(|conn| {
            conn.execute(
                "INSERT INTO user (id, email, display_name, role, password_hash, created_at) \
                 VALUES (?1, ?2, 'T', 'provider', ?3, '2026-01-01T00:00:00Z')",
                params![id.as_uuid().to_string(), email, h],
            )?;
            Ok(())
        })
        .expect("seed");
    id
}

fn seed_ws(temp: &TempDb, who: UserId) -> WorkstationId {
    let id = WorkstationId::new();
    temp.db
        .with_writer(|conn| {
            conn.execute(
                "INSERT INTO workstation \
                 (id, label, mode, bound_user_id, cert_serial, cert_fingerprint, enrolled_at, enrolled_by) \
                 VALUES (?1, 'x', 'bound', ?2, ?3, ?4, '2026-01-01T00:00:00Z', ?2)",
                params![
                    id.as_uuid().to_string(),
                    who.as_uuid().to_string(),
                    format!("s-{}", id.as_uuid()),
                    format!("f-{}", id.as_uuid()),
                ],
            )?;
            Ok(())
        })
        .expect("seed ws");
    id
}

#[test]
fn revoked_session_rejected_on_next_request() {
    let temp = TempDb::new().expect("TempDb opens");
    let u = seed_user(&temp, "a@x", "pw");
    let w = seed_ws(&temp, u);
    let outcome =
        auth::login(&temp.db, "a@x", SecretString::from("pw".to_owned()), w).expect("login");

    predicates::check_session_revoked(&temp.db, outcome.session_id).expect("not yet revoked");

    auth::revoke(&temp.db, outcome.session_id).expect("revoke");
    let err =
        predicates::check_session_revoked(&temp.db, outcome.session_id).expect_err("must reject");
    matches!(err, Error::Revoked)
        .then_some(())
        .expect("expected Revoked");
}

#[test]
fn requires_stepup_returns_action_for_documented_set() {
    let exemplars = [
        ("user.create", StepUpAction::UserCreate),
        ("user.disable", StepUpAction::UserDisable),
        ("workstation.revoke", StepUpAction::WorkstationRevoke),
        ("patient.export", StepUpAction::PatientDossierExport),
        ("query.large_download", StepUpAction::LargeQueryDownload),
        (
            "retention.policy_change",
            StepUpAction::RetentionPolicyChange,
        ),
        (
            "workstation.enroll",
            StepUpAction::WorkstationEnrollmentString,
        ),
    ];
    for (s, want) in exemplars {
        assert_eq!(predicates::requires_stepup(s), Some(want), "for {s}");
    }
    assert_eq!(predicates::requires_stepup("patient.view"), None);
    assert_eq!(predicates::requires_stepup("observation.create"), None);
}

#[test]
fn outdated_client_returns_outdated_client_error_with_min_and_got() {
    let min = Version {
        major: 1,
        minor: 4,
        patch: 0,
    };
    let err = predicates::check_client_version(&min, "1.3.9").expect_err("must reject");
    match err {
        Error::OutdatedClient { min, got } => {
            assert_eq!(min, "1.4.0");
            assert_eq!(got, "1.3.9");
        }
        other => panic!("expected OutdatedClient, got {other:?}"),
    }
    predicates::check_client_version(&min, "1.4.0").expect("ok at floor");
    predicates::check_client_version(&min, "1.5.0").expect("ok above floor");
    predicates::check_client_version(&min, "2.0.0").expect("ok above floor");
}
