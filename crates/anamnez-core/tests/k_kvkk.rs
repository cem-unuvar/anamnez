//! Subsystem K — KVKK feature surface. README §Compliance → KVKK-derived features.

#![allow(clippy::wildcard_imports)]

use anamnez_core::audit::{self, Action, AppendInput, BreachScope};
use anamnez_core::auth::stepup::{StepUpAction, StepUpReceipt};
use anamnez_core::ids::{AuthSessionId, UserId};
use anamnez_core::kvkk;
use anamnez_core::patient::{self, NewPatient, SexAssignedAtBirth};
use anamnez_core::patient_access;
use anamnez_core::test_support::prelude::*;
use anamnez_core::Error;
use rusqlite::params;
use serde_json::json;

fn seed_user(temp: &TempDb, label: &str) -> UserId {
    let id = UserId::new();
    temp.db
        .with_writer(|conn| {
            conn.execute(
                "INSERT INTO user (id, email, display_name, role, password_hash, created_at) \
                 VALUES (?1, ?2, ?3, 'provider', '!', '2026-01-01T00:00:00Z')",
                params![id.as_uuid().to_string(), format!("{label}@x"), label],
            )?;
            Ok(())
        })
        .expect("seed user");
    id
}

fn fresh_patient(temp: &TempDb, creator: UserId) -> anamnez_core::ids::PatientId {
    patient::create(
        &temp.db,
        creator,
        NewPatient {
            mrn: None,
            given_names: "[TEST] A".into(),
            family_name: "[TEST] L".into(),
            preferred_name: None,
            date_of_birth: jiff::civil::date(2000, 1, 1),
            sex_assigned_at_birth: SexAssignedAtBirth::Female,
            gender_identity: None,
            email: None,
            phone: None,
            address: None,
            emergency_contact_name: None,
            emergency_contact_phone: None,
            emergency_contact_relationship: None,
        },
    )
    .expect("create")
    .value
    .id
}

#[test]
fn dossier_export_requires_stepup_receipt_and_emits_patient_export_audit() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let patient_id = fresh_patient(&temp, owner);

    let receipt = StepUpReceipt {
        user_id: owner,
        action: StepUpAction::PatientDossierExport,
        issued_at: jiff::Timestamp::now(),
    };
    let _payload = kvkk::export::export(&temp.db, patient_id, receipt).expect("export");

    temp.db
        .with_reader(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action = ?1 AND patient_id = ?2",
                params![
                    Action::PatientExport.as_str(),
                    patient_id.as_uuid().to_string()
                ],
                |r| r.get(0),
            )?;
            assert_eq!(count, 1);
            Ok(())
        })
        .expect("audit check");
}

#[test]
fn dossier_export_rejects_wrong_stepup_action() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let patient_id = fresh_patient(&temp, owner);

    let receipt = StepUpReceipt {
        user_id: owner,
        action: StepUpAction::UserCreate, // wrong action
        issued_at: jiff::Timestamp::now(),
    };
    let err =
        kvkk::export::export(&temp.db, patient_id, receipt).expect_err("wrong receipt rejected");
    matches!(err, Error::StepUpRequired { .. })
        .then_some(())
        .expect("expected StepUpRequired");
}

#[test]
fn suppressed_patient_invisible_to_normal_queries_visible_to_audit_and_sweep() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let patient_id = fresh_patient(&temp, owner);

    assert!(!kvkk::suppression::is_suppressed(&temp.db, patient_id).expect("not suppressed"));
    kvkk::suppression::suppress(&temp.db, owner, patient_id, "user erasure request".into())
        .expect("suppress");
    assert!(kvkk::suppression::is_suppressed(&temp.db, patient_id).expect("now suppressed"));

    // patient::get filters on `suppressed_at IS NULL` — returns NotFound.
    let err = patient::get(&temp.db, owner, patient_id).expect_err("hidden");
    matches!(err, Error::NotFound)
        .then_some(())
        .expect("expected NotFound for suppressed patient");

    // The audit_log row recording the suppression is still visible.
    temp.db
        .with_reader(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE patient_id = ?1 AND metadata LIKE '%suppress%'",
                params![patient_id.as_uuid().to_string()],
                |r| r.get(0),
            )?;
            assert!(count >= 1);
            Ok(())
        })
        .expect("audit visibility");
}

fn seed_session(temp: &TempDb, user: UserId) -> AuthSessionId {
    use anamnez_core::ids::WorkstationId;
    let ws = WorkstationId::new();
    let sess = AuthSessionId::new();
    temp.db
        .with_writer(|conn| {
            conn.execute(
                "INSERT INTO workstation \
                 (id, label, mode, bound_user_id, cert_serial, cert_fingerprint, enrolled_at, enrolled_by) \
                 VALUES (?1, 'x', 'bound', ?2, ?3, ?4, '2026-01-01T00:00:00Z', ?2)",
                params![
                    ws.as_uuid().to_string(),
                    user.as_uuid().to_string(),
                    format!("s-{}", ws.as_uuid()),
                    format!("f-{}", ws.as_uuid()),
                ],
            )?;
            conn.execute(
                "INSERT INTO auth_session \
                 (id, user_id, device_id, refresh_token_hash, refresh_expires_at, absolute_expires_at, created_at, last_seen_at) \
                 VALUES (?1, ?2, ?3, X'00', '2099-01-01T00:00:00Z', '2099-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![sess.as_uuid().to_string(), user.as_uuid().to_string(), ws.as_uuid().to_string()],
            )?;
            Ok(())
        })
        .expect("seed session");
    sess
}

#[test]
fn breach_report_by_session_returns_affected_patients_and_actions() {
    let temp = TempDb::new().expect("TempDb opens");
    let user = seed_user(&temp, "actor");
    let patient_id = fresh_patient(&temp, user);
    let session = seed_session(&temp, user);

    for i in 0..3 {
        audit::append(
            &temp.db,
            AppendInput {
                actor_user_id: Some(user),
                auth_session_id: Some(session),
                action: Action::PatientView,
                target_type: "patient".into(),
                target_id: patient_id.as_uuid().to_string(),
                patient_id: Some(patient_id),
                metadata: json!({"i": i}),
            },
        )
        .expect("append");
    }
    let other_session = seed_session(&temp, user);
    audit::append(
        &temp.db,
        AppendInput {
            actor_user_id: Some(user),
            auth_session_id: Some(other_session),
            action: Action::PatientView,
            target_type: "patient".into(),
            target_id: patient_id.as_uuid().to_string(),
            patient_id: Some(patient_id),
            metadata: json!({}),
        },
    )
    .expect("other session row");

    let report =
        kvkk::breach_report::run(&temp.db, BreachScope::BySession(session)).expect("breach");
    assert_eq!(report.len(), 3, "should only see the rows for this session");
    for row in &report {
        assert!(matches!(row.action, Action::PatientView));
        assert_eq!(row.patient_id, Some(patient_id));
    }
}

#[test]
fn breach_report_by_user_and_time_range_returns_affected_list() {
    let temp = TempDb::new().expect("TempDb opens");
    let user = seed_user(&temp, "actor");
    let patient_id = fresh_patient(&temp, user);
    let session = seed_session(&temp, user);

    audit::append(
        &temp.db,
        AppendInput {
            actor_user_id: Some(user),
            auth_session_id: Some(session),
            action: Action::ObservationCreate,
            target_type: "observation".into(),
            target_id: "obs-1".into(),
            patient_id: Some(patient_id),
            metadata: json!({}),
        },
    )
    .expect("append");

    let from = jiff::Timestamp::from_second(0).unwrap();
    let until = jiff::Timestamp::now()
        .checked_add(std::time::Duration::from_secs(60))
        .expect("future ts");
    let report = kvkk::breach_report::run(
        &temp.db,
        BreachScope::ByUser {
            user_id: user,
            from,
            until,
        },
    )
    .expect("breach");
    assert!(report.iter().any(|r| r.target_id == "obs-1"));
}

#[test]
fn access_review_returns_grants_silent_at_six_months() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let other = seed_user(&temp, "bob");
    let patient_id = fresh_patient(&temp, owner);

    patient_access::grant(
        &temp.db,
        owner,
        patient_id,
        other,
        patient_access::AccessLevel::Collaborator,
    )
    .expect("grant");

    let silent = kvkk::access_review::silent_grants(&temp.db).expect("silent");
    // `other` has never touched the patient → silent.
    assert!(silent
        .iter()
        .any(|r| r.user_id == other && r.patient_id == patient_id));

    kvkk::access_review::mark_completed(&temp.db, owner).expect("mark completed");
    temp.db
        .with_reader(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action = ?1",
                params![Action::AccessReviewCompleted.as_str()],
                |r| r.get(0),
            )?;
            assert_eq!(count, 1);
            Ok(())
        })
        .expect("audit check");
}

#[test]
fn disable_user_refuses_sole_owner_case_and_emits_ownership_transfer_on_success() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let successor = seed_user(&temp, "bob");
    let patient_id = fresh_patient(&temp, owner);

    // No successor provided → refusal.
    let err =
        kvkk::ownership_transfer::disable_user_with_successors(&temp.db, owner, owner, vec![])
            .expect_err("must refuse sole owner");
    matches!(err, Error::SoleOwnerOfPatient { .. })
        .then_some(())
        .expect("expected SoleOwnerOfPatient");

    // With a successor, the disable succeeds and emits both transfer and disable audits.
    kvkk::ownership_transfer::disable_user_with_successors(
        &temp.db,
        owner,
        owner,
        vec![(patient_id, successor)],
    )
    .expect("disable");

    temp.db
        .with_reader(|conn| {
            let transfer_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action = ?1",
                params![Action::PatientOwnershipTransfer.as_str()],
                |r| r.get(0),
            )?;
            assert_eq!(transfer_count, 1);
            let disable_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action = ?1",
                params![Action::UserDisable.as_str()],
                |r| r.get(0),
            )?;
            assert_eq!(disable_count, 1);
            Ok(())
        })
        .expect("audit checks");
}

#[test]
fn retention_sweep_emits_audit_row_with_per_table_counts() {
    let temp = TempDb::new().expect("TempDb opens");
    let now = jiff::Timestamp::now();
    let report = kvkk::retention::sweep(&temp.db, now).expect("sweep");
    assert_eq!(
        report.deleted_by_table.len(),
        4,
        "audit_log + auth_session + user + patient"
    );

    temp.db
        .with_reader(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action = ?1",
                params![Action::RetentionSweep.as_str()],
                |r| r.get(0),
            )?;
            assert_eq!(count, 1);
            Ok(())
        })
        .expect("audit check");
}

#[test]
fn retention_sweep_hard_deletes_audit_rows_past_ten_year_horizon() {
    let temp = TempDb::new().expect("TempDb opens");
    // Seed an old audit_log row directly (bypassing append) with occurred_at 15 years ago.
    temp.db
        .with_writer(|conn| {
            conn.execute(
                "INSERT INTO audit_log \
                 (occurred_at, actor_user_id, auth_session_id, action, target_type, target_id, patient_id, metadata, prev_hash, row_hash) \
                 VALUES ('2010-01-01T00:00:00Z', NULL, NULL, 'patient.view', 'patient', 'old', NULL, '{}', X'00', ZEROBLOB(32))",
                params![],
            )?;
            Ok(())
        })
        .expect("seed old row");

    let report = kvkk::retention::sweep(&temp.db, jiff::Timestamp::now()).expect("sweep");
    let audit_entry = report
        .deleted_by_table
        .iter()
        .find(|(t, _)| t == "audit_log")
        .expect("audit_log entry");
    assert!(
        audit_entry.1 >= 1,
        "should have deleted at least the seeded 2010 row"
    );
}
