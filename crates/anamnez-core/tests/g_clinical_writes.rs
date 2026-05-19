//! Subsystem G — Clinical writes. README §Data Modelling, §Storage → Code systems.

#![allow(clippy::wildcard_imports)]

use anamnez_core::allergy::{self, AllergyPatch, AllergySeverity, AllergyStatus, NewAllergy};
use anamnez_core::code_systems::CodeSystem;
use anamnez_core::consent::{self, ConsentPurpose};
use anamnez_core::ids::UserId;
use anamnez_core::medication::{
    self, MedicationPatch, MedicationRoute, MedicationStatus, NewMedication,
};
use anamnez_core::observation::{
    self, ExtractedBy, NewObservation, ObservationPatch, ObservationStatus,
};
use anamnez_core::patient::{self, NewPatient, PatientPatch, SexAssignedAtBirth};
use anamnez_core::patient_access::{self, AccessLevel};
use anamnez_core::source_document::{self, NewSourceDocument, SourceDocumentType};
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

// ─── Code-system scope assertions ────────────────────────────────────────────

#[test]
fn observation_scope_excludes_skrs_vp() {
    assert!(!CodeSystem::SkrsVp.is_observation_scope());
    assert!(CodeSystem::Atc.is_observation_scope());
    assert!(CodeSystem::Icd10Tm.is_observation_scope());
    assert!(CodeSystem::Loinc.is_observation_scope());
    assert!(CodeSystem::Sut.is_observation_scope());
    assert!(CodeSystem::AnamnezSym.is_observation_scope());
    assert!(CodeSystem::Titck.is_observation_scope());
}

#[test]
fn encounter_reason_scope_is_icd10tm_anamnez_sym_skrs_vp() {
    assert!(CodeSystem::Icd10Tm.is_encounter_reason_scope());
    assert!(CodeSystem::AnamnezSym.is_encounter_reason_scope());
    assert!(CodeSystem::SkrsVp.is_encounter_reason_scope());
    assert!(!CodeSystem::Atc.is_encounter_reason_scope());
    assert!(!CodeSystem::Loinc.is_encounter_reason_scope());
}

#[test]
fn medication_scope_is_atc_or_titck() {
    assert!(CodeSystem::Atc.is_medication_scope());
    assert!(CodeSystem::Titck.is_medication_scope());
    assert!(!CodeSystem::Icd10Tm.is_medication_scope());
    assert!(!CodeSystem::Loinc.is_medication_scope());
}

#[test]
fn allergy_scope_is_atc_only_at_mvp() {
    assert!(CodeSystem::Atc.is_allergy_scope());
    assert!(!CodeSystem::Titck.is_allergy_scope());
    assert!(!CodeSystem::Icd10Tm.is_allergy_scope());
}

// ─── DB-layer CHECK constraints (no public API touch) ────────────────────────

#[test]
fn observation_final_status_requires_code_at_db_layer() {
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
                 (id, given_names, family_name, date_of_birth, sex_assigned_at_birth, created_by, created_at, updated_at) \
                 VALUES ('p1', '[TEST] A', '[TEST] L', '2000-01-01', 'female', 'u1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![],
            )?;
            let err = conn.execute(
                "INSERT INTO observation \
                 (id, patient_id, recorded_at, effective_period_start, display_text, status, extracted_by) \
                 VALUES ('o1', 'p1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'foo', 'final', 'manual')",
                params![],
            )
            .err()
            .expect("final without code must violate CHECK");
            assert!(format!("{err}").to_ascii_lowercase().contains("check"), "got: {err}");
            Ok(())
        })
        .expect("final-requires-code test");
}

#[test]
fn observation_preliminary_status_allows_null_code() {
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
                 (id, given_names, family_name, date_of_birth, sex_assigned_at_birth, created_by, created_at, updated_at) \
                 VALUES ('p1', '[TEST] A', '[TEST] L', '2000-01-01', 'female', 'u1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![],
            )?;
            let n = conn.execute(
                "INSERT INTO observation \
                 (id, patient_id, recorded_at, effective_period_start, display_text, status, extracted_by) \
                 VALUES ('o1', 'p1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'foo', 'preliminary', 'manual')",
                params![],
            )?;
            assert_eq!(n, 1);
            Ok(())
        })
        .expect("preliminary-allows-null test");
}

#[test]
fn observation_rejects_skrs_vp_at_db_layer() {
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
                 (id, given_names, family_name, date_of_birth, sex_assigned_at_birth, created_by, created_at, updated_at) \
                 VALUES ('p1', '[TEST] A', '[TEST] L', '2000-01-01', 'female', 'u1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![],
            )?;
            let err = conn
                .execute(
                    "INSERT INTO observation \
                     (id, patient_id, recorded_at, effective_period_start, code, code_system, display_text, status, extracted_by) \
                     VALUES ('o1', 'p1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'X', 'SKRS-VP', 'foo', 'final', 'manual')",
                    params![],
                )
                .err()
                .expect("SKRS-VP must be rejected at DB CHECK");
            assert!(format!("{err}").to_ascii_lowercase().contains("check"));
            Ok(())
        })
        .expect("skrs-vp-rejected test");
}

#[test]
fn encounter_finished_requires_reason_code_and_system() {
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
                 (id, given_names, family_name, date_of_birth, sex_assigned_at_birth, created_by, created_at, updated_at) \
                 VALUES ('p1', '[TEST] A', '[TEST] L', '2000-01-01', 'female', 'u1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![],
            )?;
            let err = conn
                .execute(
                    "INSERT INTO encounter (id, patient_id, provider_id, kind, reason_text, started_at, status, created_at) \
                     VALUES ('e1', 'p1', 'u1', 'in_person', 'check-up', '2026-01-01T00:00:00Z', 'finished', '2026-01-01T00:00:00Z')",
                    params![],
                )
                .err()
                .expect("finished without reason_code must violate CHECK");
            assert!(format!("{err}").to_ascii_lowercase().contains("check"), "got: {err}");
            Ok(())
        })
        .expect("encounter-finished-check test");
}

#[test]
fn allergy_code_and_code_system_are_co_nullable_at_db_layer() {
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
                 (id, given_names, family_name, date_of_birth, sex_assigned_at_birth, created_by, created_at, updated_at) \
                 VALUES ('p1', '[TEST] A', '[TEST] L', '2000-01-01', 'female', 'u1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![],
            )?;
            conn.execute(
                "INSERT INTO allergy (id, patient_id, display_text, severity, status, recorded_at, recorded_by) \
                 VALUES ('a1', 'p1', 'peanuts', 'severe', 'active', '2026-01-01T00:00:00Z', 'u1')",
                params![],
            )?;
            conn.execute(
                "INSERT INTO allergy (id, patient_id, code, code_system, display_text, severity, status, recorded_at, recorded_by) \
                 VALUES ('a2', 'p1', 'J01C', 'ATC', 'penicillins', 'severe', 'active', '2026-01-01T00:00:00Z', 'u1')",
                params![],
            )?;
            let err = conn.execute(
                "INSERT INTO allergy (id, patient_id, code, display_text, severity, status, recorded_at, recorded_by) \
                 VALUES ('a3', 'p1', 'J01C', 'mismatched', 'severe', 'active', '2026-01-01T00:00:00Z', 'u1')",
                params![],
            ).err().expect("only code set without code_system must violate CHECK");
            assert!(format!("{err}").to_ascii_lowercase().contains("check"));
            Ok(())
        })
        .expect("allergy co-nullable");
}

#[test]
fn medication_code_system_restricted_to_atc_or_titck_at_db_layer() {
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
                 (id, given_names, family_name, date_of_birth, sex_assigned_at_birth, created_by, created_at, updated_at) \
                 VALUES ('p1', '[TEST] A', '[TEST] L', '2000-01-01', 'female', 'u1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![],
            )?;
            let err = conn.execute(
                "INSERT INTO medication \
                 (id, patient_id, code, code_system, display_text, route, started_at, status, recorded_at, recorded_by) \
                 VALUES ('m1', 'p1', 'X', 'ICD10TM', 'metformin', 'oral', '2026-01-01T00:00:00Z', 'active', '2026-01-01T00:00:00Z', 'u1')",
                params![],
            ).err().expect("ICD10TM medication must violate CHECK");
            assert!(format!("{err}").to_ascii_lowercase().contains("check"));
            Ok(())
        })
        .expect("medication code_system test");
}

// ─── Public API: observation::create / amend / problem_list ─────────────────

#[test]
fn observation_create_via_public_api_with_invalid_pair_returns_code_system_mismatch() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);

    let err = observation::create(
        &temp.db,
        owner,
        NewObservation {
            patient_id: pid,
            effective_period_start: jiff::Timestamp::now(),
            effective_period_end: None,
            code: "NOTACODE".into(),
            code_system: CodeSystem::Icd10Tm,
            display_text: "made up".into(),
            value: None,
            status: ObservationStatus::Preliminary,
            is_problem_list_item: false,
            source_id: None,
            encounter_id: None,
            extracted_by: ExtractedBy::Manual,
            model_version: None,
            confidence: None,
        },
    )
    .expect_err("invalid (code_system, code) must reject");
    matches!(err, Error::CodeSystemMismatch { .. })
        .then_some(())
        .expect("expected CodeSystemMismatch");
}

#[test]
fn observation_create_rejects_skrs_vp_at_api_layer() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);

    let err = observation::create(
        &temp.db,
        owner,
        NewObservation {
            patient_id: pid,
            effective_period_start: jiff::Timestamp::now(),
            effective_period_end: None,
            code: "1".into(),
            code_system: CodeSystem::SkrsVp,
            display_text: "wrong scope".into(),
            value: None,
            status: ObservationStatus::Preliminary,
            is_problem_list_item: false,
            source_id: None,
            encounter_id: None,
            extracted_by: ExtractedBy::Manual,
            model_version: None,
            confidence: None,
        },
    )
    .expect_err("SKRS-VP must be rejected for observations");
    matches!(err, Error::CodeSystemNotAllowed { .. })
        .then_some(())
        .expect("expected CodeSystemNotAllowed");
}

#[test]
fn observation_amend_is_in_place_version_bump() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);

    // Codes are required on every observation; seed a single ANAMNEZ-SYM row
    // so create + amend's lookup_in_conn succeeds without loading the full CSV.
    insert_symptom(&temp, "ANAMNEZ-SYM-0042", "boyun ağrısı");

    let v1 = observation::create(
        &temp.db,
        owner,
        NewObservation {
            patient_id: pid,
            effective_period_start: jiff::Timestamp::now(),
            effective_period_end: None,
            code: "ANAMNEZ-SYM-0042".into(),
            code_system: CodeSystem::AnamnezSym,
            display_text: "boyun ağrısı".into(),
            value: None,
            status: ObservationStatus::Preliminary,
            is_problem_list_item: false,
            source_id: None,
            encounter_id: None,
            extracted_by: ExtractedBy::Manual,
            model_version: None,
            confidence: None,
        },
    )
    .expect("create");

    let v2 = observation::amend(
        &temp.db,
        owner,
        v1.value.id,
        v1.version,
        ObservationPatch {
            display_text: Some("boyun ağrısı, hafif".into()),
            ..Default::default()
        },
    )
    .expect("amend");

    assert_eq!(v2.version, v1.version + 1);
    assert_eq!(
        v2.value.id, v1.value.id,
        "id must be stable across amendments"
    );
    assert!(matches!(v2.value.status, ObservationStatus::Amended));
    assert_eq!(v2.value.display_text, "boyun ağrısı, hafif");
}

#[test]
fn problem_list_returns_active_final_icd10tm_observations() {
    use anamnez_core::code_systems::loader;
    use anamnez_core::code_systems::{repo_code_systems_root, CodeSystem};

    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);

    // Load ICD-10-TM so the (code_system, code) check passes for a real code.
    let root = repo_code_systems_root();
    temp.db
        .with_writer(|conn| {
            loader::load_csv(
                conn,
                CodeSystem::Icd10Tm,
                &root.join("icd10-tm/normalized.csv"),
            )
        })
        .expect("ICD-10-TM load");

    // Pick a known ICD-10-TM code from the loaded data.
    let known: String = temp
        .db
        .with_reader(|conn| {
            let s: String = conn.query_row(
                "SELECT code FROM icd10_tm WHERE code = 'A00.0' OR code = 'A00' LIMIT 1",
                params![],
                |r| r.get(0),
            )?;
            Ok(s)
        })
        .expect("pick known code");

    let _final_problem = observation::create(
        &temp.db,
        owner,
        NewObservation {
            patient_id: pid,
            effective_period_start: jiff::Timestamp::now(),
            effective_period_end: None,
            code: known.clone(),
            code_system: CodeSystem::Icd10Tm,
            display_text: "kolera".into(),
            value: None,
            status: ObservationStatus::Final,
            is_problem_list_item: true,
            source_id: None,
            encounter_id: None,
            extracted_by: ExtractedBy::Manual,
            model_version: None,
            confidence: None,
        },
    )
    .expect("create problem list final");

    // A non-problem observation shouldn't appear in the problem list.
    let _ = observation::create(
        &temp.db,
        owner,
        NewObservation {
            patient_id: pid,
            effective_period_start: jiff::Timestamp::now(),
            effective_period_end: None,
            code: known.clone(),
            code_system: CodeSystem::Icd10Tm,
            display_text: "secondary mention".into(),
            value: None,
            status: ObservationStatus::Final,
            is_problem_list_item: false,
            source_id: None,
            encounter_id: None,
            extracted_by: ExtractedBy::Manual,
            model_version: None,
            confidence: None,
        },
    )
    .expect("create non-problem final");

    let problems = observation::problem_list(&temp.db, owner, pid).expect("problem_list");
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].value.code.as_deref(), Some(known.as_str()));
    assert!(problems[0].value.is_problem_list_item);
}

#[test]
fn observation_mark_entered_in_error_hides_row_from_problem_list_and_writes_audit() {
    use anamnez_core::code_systems::loader;
    use anamnez_core::code_systems::{repo_code_systems_root, CodeSystem};

    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);

    // Need a real ICD-10-TM code for the (code, code_system) FK check.
    let root = repo_code_systems_root();
    temp.db
        .with_writer(|conn| {
            loader::load_csv(
                conn,
                CodeSystem::Icd10Tm,
                &root.join("icd10-tm/normalized.csv"),
            )
        })
        .expect("ICD-10-TM load");
    let known: String = temp
        .db
        .with_reader(|conn| {
            let s: String = conn.query_row(
                "SELECT code FROM icd10_tm WHERE code = 'A00.0' OR code = 'A00' LIMIT 1",
                params![],
                |r| r.get(0),
            )?;
            Ok(s)
        })
        .expect("pick known code");

    let v1 = observation::create(
        &temp.db,
        owner,
        NewObservation {
            patient_id: pid,
            effective_period_start: jiff::Timestamp::now(),
            effective_period_end: None,
            code: known.clone(),
            code_system: CodeSystem::Icd10Tm,
            display_text: "mistaken entry".into(),
            value: None,
            status: ObservationStatus::Final,
            is_problem_list_item: true,
            source_id: None,
            encounter_id: None,
            extracted_by: ExtractedBy::Manual,
            model_version: None,
            confidence: None,
        },
    )
    .expect("create");
    assert_eq!(
        observation::problem_list(&temp.db, owner, pid)
            .expect("pre")
            .len(),
        1,
        "problem appears before retraction",
    );

    let v2 = observation::mark_entered_in_error(&temp.db, owner, v1.value.id, v1.version)
        .expect("mark_entered_in_error");

    assert_eq!(v2.version, v1.version + 1);
    assert!(matches!(
        v2.value.status,
        ObservationStatus::EnteredInError
    ));
    assert!(
        observation::problem_list(&temp.db, owner, pid)
            .expect("post")
            .is_empty(),
        "entered-in-error row must drop out of the problem list",
    );
    assert!(
        observation::list_by_patient(&temp.db, owner, pid)
            .expect("list")
            .is_empty(),
        "entered-in-error row must drop out of list_by_patient too",
    );
    assert_eq!(last_audit_action(&temp), "observation.entered_in_error");

    // Idempotent rejection: second call on the same row returns
    // InvalidStateTransition rather than blindly bumping the version again.
    let err = observation::mark_entered_in_error(&temp.db, owner, v1.value.id, v2.version)
        .expect_err("second mark must be rejected");
    matches!(err, Error::InvalidStateTransition { .. })
        .then_some(())
        .expect("expected InvalidStateTransition");
}

// ─── Helpers shared by subsystem-G tests ─────────────────────────────────────

fn last_audit_action(temp: &TempDb) -> String {
    temp.db
        .with_reader(|conn| {
            let s: String = conn.query_row(
                "SELECT action FROM audit_log ORDER BY id DESC LIMIT 1",
                params![],
                |r| r.get(0),
            )?;
            Ok(s)
        })
        .expect("read last audit action")
}

fn insert_atc(temp: &TempDb, code: &str, description_tr: &str) {
    temp.db
        .with_writer(|conn| {
            conn.execute(
                "INSERT INTO drug_atc (atc_code, description_en, description_tr) VALUES (?1, ?2, ?3)",
                params![code, "", description_tr],
            )?;
            Ok(())
        })
        .expect("seed drug_atc row");
}

fn insert_symptom(temp: &TempDb, code: &str, display_tr: &str) {
    temp.db
        .with_writer(|conn| {
            conn.execute(
                "INSERT INTO symptom_anamnez (code, display_tr, display_en, body_region) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![code, display_tr, "neck pain", "head_neck"],
            )?;
            Ok(())
        })
        .expect("seed symptom_anamnez row");
}

// ─── Public API: allergy::create / amend ─────────────────────────────────────

#[test]
fn allergy_create_via_public_api_inserts_row_and_audits() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);

    let v1 = allergy::create(
        &temp.db,
        owner,
        NewAllergy {
            patient_id: pid,
            code: None,
            code_system: None,
            display_text: "peanuts".into(),
            severity: AllergySeverity::Severe,
            reaction_text: Some("anaphylaxis".into()),
            status: AllergyStatus::Active,
            onset_date: None,
            source_id: None,
            encounter_id: None,
        },
    )
    .expect("create");

    assert_eq!(v1.version, 1);
    assert_eq!(v1.value.display_text, "peanuts");
    assert_eq!(last_audit_action(&temp), "allergy.create");
}

#[test]
fn allergy_create_rejects_non_atc_code_system() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);

    let err = allergy::create(
        &temp.db,
        owner,
        NewAllergy {
            patient_id: pid,
            code: Some("E11.9".into()),
            code_system: Some(CodeSystem::Icd10Tm),
            display_text: "wrong scope".into(),
            severity: AllergySeverity::Mild,
            reaction_text: None,
            status: AllergyStatus::Active,
            onset_date: None,
            source_id: None,
            encounter_id: None,
        },
    )
    .expect_err("non-ATC must be rejected");
    matches!(err, Error::CodeSystemNotAllowed { .. })
        .then_some(())
        .expect("expected CodeSystemNotAllowed");
}

#[test]
fn allergy_create_rejects_code_without_code_system() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);

    let err = allergy::create(
        &temp.db,
        owner,
        NewAllergy {
            patient_id: pid,
            code: Some("J01C".into()),
            code_system: None,
            display_text: "mismatched".into(),
            severity: AllergySeverity::Severe,
            reaction_text: None,
            status: AllergyStatus::Active,
            onset_date: None,
            source_id: None,
            encounter_id: None,
        },
    )
    .expect_err("code without code_system must be rejected");
    matches!(err, Error::Invariant(_))
        .then_some(())
        .expect("expected Invariant");
}

#[test]
fn allergy_amend_optimistic_lock_returns_conflict_on_stale_version() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);

    let v1 = allergy::create(
        &temp.db,
        owner,
        NewAllergy {
            patient_id: pid,
            code: None,
            code_system: None,
            display_text: "peanuts".into(),
            severity: AllergySeverity::Mild,
            reaction_text: None,
            status: AllergyStatus::Active,
            onset_date: None,
            source_id: None,
            encounter_id: None,
        },
    )
    .expect("create");

    let v2 = allergy::amend(
        &temp.db,
        owner,
        v1.value.id,
        v1.version,
        AllergyPatch {
            severity: Some(AllergySeverity::Severe),
            ..Default::default()
        },
    )
    .expect("amend");
    assert_eq!(v2.version, v1.version + 1);
    assert!(matches!(v2.value.severity, AllergySeverity::Severe));
    assert_eq!(last_audit_action(&temp), "allergy.amend");

    let err = allergy::amend(
        &temp.db,
        owner,
        v1.value.id,
        v1.version, // stale
        AllergyPatch {
            status: Some(AllergyStatus::Inactive),
            ..Default::default()
        },
    )
    .expect_err("stale version must conflict");
    matches!(err, Error::Conflict { .. })
        .then_some(())
        .expect("expected Conflict");
}

// ─── Public API: medication::create / amend ──────────────────────────────────

#[test]
fn medication_create_via_public_api_with_invalid_pair_returns_code_system_mismatch() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);

    let err = medication::create(
        &temp.db,
        owner,
        NewMedication {
            patient_id: pid,
            code: "NOTACODE".into(),
            code_system: CodeSystem::Atc,
            display_text: "made up".into(),
            dose_quantity: None,
            dose_unit: None,
            frequency_text: None,
            route: MedicationRoute::Oral,
            started_at: jiff::Timestamp::now(),
            ended_at: None,
            reason_text: None,
            status: MedicationStatus::Active,
            prescriber_id: None,
            source_id: None,
            encounter_id: None,
        },
    )
    .expect_err("invalid ATC code must reject");
    matches!(err, Error::CodeSystemMismatch { .. })
        .then_some(())
        .expect("expected CodeSystemMismatch");
}

#[test]
fn medication_create_rejects_skrs_vp_at_api_layer() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);

    let err = medication::create(
        &temp.db,
        owner,
        NewMedication {
            patient_id: pid,
            code: "1".into(),
            code_system: CodeSystem::SkrsVp,
            display_text: "wrong scope".into(),
            dose_quantity: None,
            dose_unit: None,
            frequency_text: None,
            route: MedicationRoute::Oral,
            started_at: jiff::Timestamp::now(),
            ended_at: None,
            reason_text: None,
            status: MedicationStatus::Active,
            prescriber_id: None,
            source_id: None,
            encounter_id: None,
        },
    )
    .expect_err("SKRS-VP medication must reject");
    matches!(err, Error::CodeSystemNotAllowed { .. })
        .then_some(())
        .expect("expected CodeSystemNotAllowed");
}

#[test]
fn medication_amend_in_place_version_bump() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);
    insert_atc(&temp, "A10BA02", "metformin");

    let v1 = medication::create(
        &temp.db,
        owner,
        NewMedication {
            patient_id: pid,
            code: "A10BA02".into(),
            code_system: CodeSystem::Atc,
            display_text: "metformin".into(),
            dose_quantity: Some(500.0),
            dose_unit: Some("mg".into()),
            frequency_text: Some("günde 2 kez".into()),
            route: MedicationRoute::Oral,
            started_at: jiff::Timestamp::now(),
            ended_at: None,
            reason_text: None,
            status: MedicationStatus::Active,
            prescriber_id: None,
            source_id: None,
            encounter_id: None,
        },
    )
    .expect("create");
    assert_eq!(v1.version, 1);
    assert_eq!(last_audit_action(&temp), "medication.create");

    let v2 = medication::amend(
        &temp.db,
        owner,
        v1.value.id,
        v1.version,
        MedicationPatch {
            status: Some(MedicationStatus::Stopped),
            ended_at: Some(Some(jiff::Timestamp::now())),
            ..Default::default()
        },
    )
    .expect("amend");
    assert_eq!(v2.version, v1.version + 1);
    assert_eq!(v2.value.id, v1.value.id);
    assert!(matches!(v2.value.status, MedicationStatus::Stopped));
    assert!(v2.value.ended_at.is_some());
    assert_eq!(last_audit_action(&temp), "medication.amend");
}

// ─── Public API: source_document::create / get ───────────────────────────────

#[test]
fn source_document_create_inserts_row_and_audits() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);

    let v1 = source_document::create(
        &temp.db,
        owner,
        NewSourceDocument {
            patient_id: pid,
            kind: SourceDocumentType::Pdf,
            sha256: "abc123".into(),
            original_filename: "lab.pdf".into(),
            mime_type: "application/pdf".into(),
            transcription: None,
            ocr_text: Some("LDL 85 mg/dL".into()),
            encounter_id: None,
            context_provided_by_user: Some("lab report".into()),
        },
    )
    .expect("create");
    assert_eq!(v1.version, 1);
    assert_eq!(v1.value.sha256, "abc123");
    assert_eq!(last_audit_action(&temp), "source_document.create");
}

#[test]
fn source_document_get_returns_versioned_row() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);

    let v1 = source_document::create(
        &temp.db,
        owner,
        NewSourceDocument {
            patient_id: pid,
            kind: SourceDocumentType::Image,
            sha256: "ff00".into(),
            original_filename: "xray.png".into(),
            mime_type: "image/png".into(),
            transcription: None,
            ocr_text: None,
            encounter_id: None,
            context_provided_by_user: None,
        },
    )
    .expect("create");

    let got = source_document::get(&temp.db, owner, v1.value.id).expect("get");
    assert_eq!(got.version, 1);
    assert_eq!(got.value.sha256, "ff00");
    assert_eq!(got.value.original_filename, "xray.png");
}

#[test]
fn source_document_get_denied_without_access() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let stranger = seed_user(&temp, "stranger");
    let pid = fresh_patient(&temp, owner);

    let v1 = source_document::create(
        &temp.db,
        owner,
        NewSourceDocument {
            patient_id: pid,
            kind: SourceDocumentType::Note,
            sha256: "deadbeef".into(),
            original_filename: "note.txt".into(),
            mime_type: "text/plain".into(),
            transcription: None,
            ocr_text: None,
            encounter_id: None,
            context_provided_by_user: None,
        },
    )
    .expect("create");

    let err = source_document::get(&temp.db, stranger, v1.value.id)
        .expect_err("stranger must not see source document");
    matches!(err, Error::NotFound)
        .then_some(())
        .expect("expected NotFound");
}

// ─── Public API: consent::record / revoke / has_active ───────────────────────

#[test]
fn consent_record_inserts_row_with_has_active_true() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);

    assert!(
        !consent::has_active(&temp.db, pid, ConsentPurpose::LawyerTransfer).expect("has_active"),
        "no consent recorded yet"
    );

    let v1 = consent::record(
        &temp.db,
        owner,
        pid,
        ConsentPurpose::LawyerTransfer,
        None,
        Some("transfer to Ahmet Yılmaz, attorney".into()),
    )
    .expect("record");
    assert_eq!(v1.version, 1);
    assert!(v1.value.revoked_at.is_none());
    assert_eq!(last_audit_action(&temp), "consent.record");

    assert!(
        consent::has_active(&temp.db, pid, ConsentPurpose::LawyerTransfer)
            .expect("has_active after record")
    );
    assert!(
        !consent::has_active(&temp.db, pid, ConsentPurpose::ResearchNonAnonymized)
            .expect("has_active other purpose"),
        "different purpose should be unaffected"
    );
}

#[test]
fn consent_revoke_sets_revoked_at_and_has_active_false() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);

    let v1 = consent::record(
        &temp.db,
        owner,
        pid,
        ConsentPurpose::ResearchNonAnonymized,
        None,
        None,
    )
    .expect("record");

    let v2 = consent::revoke(&temp.db, owner, v1.value.id, v1.version).expect("revoke");
    assert_eq!(v2.version, v1.version + 1);
    assert!(v2.value.revoked_at.is_some());
    assert_eq!(last_audit_action(&temp), "consent.revoke");

    assert!(
        !consent::has_active(&temp.db, pid, ConsentPurpose::ResearchNonAnonymized)
            .expect("has_active after revoke"),
        "revoked consent must not count as active"
    );
}

#[test]
fn consent_revoke_conflict_on_stale_version() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);

    let v1 = consent::record(
        &temp.db,
        owner,
        pid,
        ConsentPurpose::OtherClinicReferral,
        None,
        None,
    )
    .expect("record");
    let _v2 = consent::revoke(&temp.db, owner, v1.value.id, v1.version).expect("revoke");

    let err = consent::revoke(&temp.db, owner, v1.value.id, v1.version)
        .expect_err("stale version must conflict");
    matches!(err, Error::Conflict { .. })
        .then_some(())
        .expect("expected Conflict");
}

// ─── Public API: patient::update ─────────────────────────────────────────────

#[test]
fn patient_update_in_place_version_bump() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);

    let initial = patient::get(&temp.db, owner, pid).expect("get");
    assert_eq!(initial.version, 1);
    assert!(initial.value.phone.is_none());

    let updated = patient::update(
        &temp.db,
        owner,
        pid,
        initial.version,
        PatientPatch {
            phone: Some(Some("+90 555 000 0000".into())),
            preferred_name: Some(Some("Maria".into())),
            ..Default::default()
        },
    )
    .expect("update");
    assert_eq!(updated.version, initial.version + 1);
    assert_eq!(updated.value.phone.as_deref(), Some("+90 555 000 0000"));
    assert_eq!(updated.value.preferred_name.as_deref(), Some("Maria"));
    assert_eq!(last_audit_action(&temp), "patient.update");
}

#[test]
fn patient_update_returns_conflict_on_stale_version() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let pid = fresh_patient(&temp, owner);

    let v1 = patient::get(&temp.db, owner, pid).expect("get");
    let _v2 = patient::update(
        &temp.db,
        owner,
        pid,
        v1.version,
        PatientPatch {
            email: Some(Some("a@b".into())),
            ..Default::default()
        },
    )
    .expect("first update");

    let err = patient::update(
        &temp.db,
        owner,
        pid,
        v1.version, // stale
        PatientPatch {
            email: Some(Some("c@d".into())),
            ..Default::default()
        },
    )
    .expect_err("stale must conflict");
    matches!(err, Error::Conflict { .. })
        .then_some(())
        .expect("expected Conflict");
}

#[test]
fn patient_update_denied_to_read_only_user() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let ro = seed_user(&temp, "bob");
    let pid = fresh_patient(&temp, owner);

    patient_access::grant(&temp.db, owner, pid, ro, AccessLevel::ReadOnly).expect("grant");

    let v1 = patient::get(&temp.db, ro, pid).expect("ro can read");

    let err = patient::update(
        &temp.db,
        ro,
        pid,
        v1.version,
        PatientPatch {
            phone: Some(Some("+90 555 111 2222".into())),
            ..Default::default()
        },
    )
    .expect_err("read-only must not update");
    matches!(err, Error::Forbidden)
        .then_some(())
        .expect("expected Forbidden");
}
