//! Subsystem D — Optimistic locking. README §Storage → Concurrency.

#![allow(clippy::wildcard_imports)]

use anamnez_core::ids::UserId;
use anamnez_core::locking::Versioned;
use anamnez_core::code_systems::CodeSystem;
use anamnez_core::observation::{self, NewObservation, ObservationPatch, ObservationStatus};
use anamnez_core::patient::{self, NewPatient, SexAssignedAtBirth};
use anamnez_core::test_support::prelude::*;
use anamnez_core::Error;
use rusqlite::params;

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
    let p = patient::create(
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
    .expect("create patient");
    p.value.id
}

#[test]
fn versioned_holds_value_and_version() {
    let v = Versioned::new("hello", 7);
    assert_eq!(v.value, "hello");
    assert_eq!(v.version, 7);
}

#[test]
fn raw_optimistic_update_bumps_version_only_when_expected_matches() {
    let temp = TempDb::new().expect("TempDb opens");
    temp.db
        .with_writer(|conn| {
            conn.execute(
                "INSERT INTO user (id, email, display_name, role, password_hash, created_at) \
                 VALUES ('u1', 'a@x', 'A', 'provider', '!', '2026-01-01T00:00:00Z')",
                params![],
            )?;
            conn.execute(
                "INSERT INTO patient \
                 (id, given_names, family_name, date_of_birth, sex_assigned_at_birth, created_by, created_at, updated_at, version) \
                 VALUES ('p1', '[TEST] A', '[TEST] L', '2000-01-01', 'female', 'u1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 1)",
                params![],
            )?;
            let n = conn.execute(
                "UPDATE patient SET preferred_name = 'X', version = version + 1 WHERE id = 'p1' AND version = 1",
                params![],
            )?;
            assert_eq!(n, 1);
            let v_after: i64 = conn.query_row("SELECT version FROM patient WHERE id = 'p1'", params![], |r| r.get(0))?;
            assert_eq!(v_after, 2);
            let n = conn.execute(
                "UPDATE patient SET preferred_name = 'Y', version = version + 1 WHERE id = 'p1' AND version = 1",
                params![],
            )?;
            assert_eq!(n, 0);
            Ok(())
        })
        .expect("locking smoke");
}

#[test]
fn observation_stale_version_returns_typed_conflict_with_new_state() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);

    // Codes are required on every observation; seed a single ANAMNEZ-SYM row
    // so create + amend's lookup_in_conn succeeds.
    temp.db
        .with_writer(|conn| {
            conn.execute(
                "INSERT INTO symptom_anamnez (code, display_tr, display_en, body_region) \
                 VALUES ('ANAMNEZ-SYM-0042', 'boyun ağrısı', 'neck pain', 'head_neck')",
                params![],
            )?;
            Ok(())
        })
        .expect("seed symptom row");

    let now = jiff::Timestamp::now();
    let v1 = observation::create(
        &temp.db,
        owner,
        NewObservation {
            patient_id: pid,
            effective_period_start: now,
            effective_period_end: None,
            code: "ANAMNEZ-SYM-0042".into(),
            code_system: CodeSystem::AnamnezSym,
            display_text: "boyun ağrısı".into(),
            value: None,
            status: ObservationStatus::Preliminary,
            is_problem_list_item: false,
            source_id: None,
            encounter_id: None,
            extracted_by: anamnez_core::observation::ExtractedBy::Manual,
            model_version: None,
            confidence: None,
        },
    )
    .expect("create");

    let _v2 = observation::amend(
        &temp.db,
        owner,
        v1.value.id,
        v1.version,
        ObservationPatch {
            display_text: Some("revised".into()),
            ..Default::default()
        },
    )
    .expect("first amend");

    let err = observation::amend(
        &temp.db,
        owner,
        v1.value.id,
        v1.version,
        ObservationPatch {
            display_text: Some("third".into()),
            ..Default::default()
        },
    )
    .expect_err("stale version must conflict");

    match err {
        Error::Conflict {
            current_version,
            new_state_json,
        } => {
            assert_eq!(current_version, 2);
            assert!(
                new_state_json.contains("revised"),
                "conflict payload should expose current state"
            );
        }
        other => panic!("expected Error::Conflict, got {other:?}"),
    }
}

#[test]
fn encounter_stale_version_returns_typed_conflict_or_invalid_transition() {
    use anamnez_core::encounter::{self, EncounterKind};

    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);

    let v1 = encounter::start(
        &temp.db,
        pid,
        owner,
        EncounterKind::InPerson,
        "checkup".into(),
    )
    .expect("start");
    let _v2 = encounter::cancel(&temp.db, owner, v1.value.id, v1.version).expect("cancel");

    let err = encounter::cancel(&temp.db, owner, v1.value.id, v1.version)
        .expect_err("stale version must conflict");
    matches!(
        err,
        Error::Conflict { .. } | Error::InvalidStateTransition { .. }
    )
    .then_some(())
    .expect("expected Conflict or InvalidStateTransition");
}
