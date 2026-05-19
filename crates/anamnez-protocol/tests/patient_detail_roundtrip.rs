//! Wire round-trip for `PatientDetail`. The shape ships across a real HTTP
//! boundary (daemon → reqwest JSON decoder on the workstation), so any field
//! that fails to deserialize stalls the patient page with
//! "serde: error decoding response body". These tests pin the JSON shape so
//! a future protocol change can't silently break the workstation client.

use anamnez_protocol::access::AccessLevel;
use anamnez_protocol::encounter::{Encounter, EncounterKind, EncounterStatus};
use anamnez_protocol::ids::{EncounterId, PatientId, UserId};
use anamnez_protocol::patient::{Patient, PatientDetail, SexAssignedAtBirth};
use anamnez_protocol::versioned::Versioned;
use jiff::Timestamp;
use uuid::Uuid;

fn uuid_from(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn sample_patient() -> Patient {
    Patient {
        id: PatientId(uuid_from(1)),
        mrn: Some("MRN-001".into()),
        given_names: "Ayşe".into(),
        family_name: "Yılmaz".into(),
        preferred_name: None,
        date_of_birth: "1980-03-15".parse().unwrap(),
        sex_assigned_at_birth: SexAssignedAtBirth::Female,
        gender_identity: None,
        email: None,
        phone: None,
        address: None,
        emergency_contact_name: None,
        emergency_contact_phone: None,
        emergency_contact_relationship: None,
        created_by: UserId(uuid_from(99)),
        created_at: Timestamp::now(),
        updated_at: Timestamp::now(),
        deceased_at: None,
        archived_at: None,
        suppressed_at: None,
        suppression_reason: None,
        notice_acknowledged_at: None,
    }
}

fn sample_encounter(
    enc_id_seed: u128,
    patient_id: PatientId,
    status: EncounterStatus,
) -> Encounter {
    Encounter {
        id: EncounterId(uuid_from(enc_id_seed)),
        patient_id,
        provider_id: UserId(uuid_from(99)),
        kind: EncounterKind::InPerson,
        reason_text: "boyun ağrısı kontrolü".into(),
        reason_code: None,
        reason_code_system: None,
        started_at: Timestamp::now(),
        ended_at: None,
        status,
        created_at: Timestamp::now(),
    }
}

#[test]
fn patient_detail_roundtrips_with_empty_collections() {
    let patient = sample_patient();
    let detail = PatientDetail {
        patient: patient.clone(),
        access_level: AccessLevel::Owner,
        problem_list: vec![],
        allergies: vec![],
        medications: vec![],
        encounters: vec![],
        active_encounter_observations: vec![],
    };
    let s = serde_json::to_string(&detail).expect("serialize");
    let back: PatientDetail = serde_json::from_str(&s).expect("deserialize");
    assert_eq!(back.patient.id, patient.id);
    assert!(back.encounters.is_empty());
}

#[test]
fn patient_detail_roundtrips_with_versioned_encounter() {
    // The bug we're guarding against: the workstation UI needs `Versioned`
    // wrappers on encounters so it can call `finish_encounter` with the right
    // `expected_version` after hydrating from a `PatientDetail` response. If
    // the JSON shape drifts (e.g. encounter is serialized without the wrapper
    // by an outdated daemon), the client's reqwest `.json()` fails with
    // "serde: error decoding response body" and the patient page never renders.
    let patient = sample_patient();
    let enc = sample_encounter(10, patient.id, EncounterStatus::InProgress);
    let detail = PatientDetail {
        patient: patient.clone(),
        access_level: AccessLevel::Collaborator,
        problem_list: vec![],
        allergies: vec![],
        medications: vec![],
        encounters: vec![Versioned::new(enc.clone(), 1)],
        active_encounter_observations: vec![],
    };

    let s = serde_json::to_string(&detail).expect("serialize");
    // The serialized form has to expose the {value, version} shape so the
    // workstation client deserializes encounters as `Versioned<Encounter>`.
    assert!(
        s.contains("\"version\":1"),
        "expected versioned encounter shape, got: {s}",
    );
    assert!(
        s.contains("\"reason_text\":\"boyun ağrısı kontrolü\""),
        "expected encounter payload inside `value`, got: {s}",
    );

    let back: PatientDetail = serde_json::from_str(&s).expect("deserialize");
    assert_eq!(back.encounters.len(), 1);
    let got = &back.encounters[0];
    assert_eq!(got.version, 1);
    assert_eq!(got.value.id, enc.id);
    assert!(matches!(got.value.status, EncounterStatus::InProgress));
    assert_eq!(got.value.reason_text, "boyun ağrısı kontrolü");
}

#[test]
fn patient_detail_in_progress_encounter_is_findable() {
    // The workstation hydrates `active_encounter` by scanning `d.encounters`
    // for the first `InProgress` entry. Make sure that filter behaves on a
    // round-tripped payload (statuses are `rename_all = snake_case`, so a
    // typo on the wire would silently break the filter).
    let patient = sample_patient();
    let finished = sample_encounter(10, patient.id, EncounterStatus::Finished);
    let in_progress = sample_encounter(20, patient.id, EncounterStatus::InProgress);

    let detail = PatientDetail {
        patient,
        access_level: AccessLevel::Owner,
        problem_list: vec![],
        allergies: vec![],
        medications: vec![],
        encounters: vec![
            Versioned::new(finished, 2),
            Versioned::new(in_progress.clone(), 1),
        ],
        active_encounter_observations: vec![],
    };
    let s = serde_json::to_string(&detail).expect("serialize");
    let back: PatientDetail = serde_json::from_str(&s).expect("deserialize");

    let active = back
        .encounters
        .iter()
        .find(|v| matches!(v.value.status, EncounterStatus::InProgress));
    let active = active.expect("expected to find an InProgress encounter");
    assert_eq!(active.value.id, in_progress.id);
    assert_eq!(active.version, 1);
}

#[test]
fn patient_detail_rejects_unwrapped_encounter_legacy_shape() {
    // Regression for the bug: if a server (or test fixture) still emits the
    // legacy `encounters: [Encounter, ...]` shape WITHOUT the `Versioned`
    // wrapper, deserialization must fail loudly. Without this test, a daemon
    // built from an older protocol version could quietly hand back the wrong
    // shape and the client would only fail at runtime with the opaque
    // "serde: error decoding response body" message.
    let patient = sample_patient();
    let pid_json = serde_json::to_string(&patient.id).unwrap();
    let uid_json = serde_json::to_string(&UserId(uuid_from(99))).unwrap();
    let eid_json = serde_json::to_string(&EncounterId(uuid_from(10))).unwrap();
    let started = Timestamp::now().to_string();
    let legacy = format!(
        r#"{{
            "patient": {patient_json},
            "access_level": "owner",
            "problem_list": [],
            "allergies": [],
            "medications": [],
            "encounters": [
              {{
                "id": {eid_json},
                "patient_id": {pid_json},
                "provider_id": {uid_json},
                "kind": "in_person",
                "reason_text": "legacy",
                "reason_code": null,
                "reason_code_system": null,
                "started_at": "{started}",
                "ended_at": null,
                "status": "in_progress",
                "created_at": "{started}"
              }}
            ]
        }}"#,
        patient_json = serde_json::to_string(&patient).unwrap(),
    );

    let err = serde_json::from_str::<PatientDetail>(&legacy).unwrap_err();
    // The exact message comes from serde and complains about the missing
    // `value` field inside the legacy encounter element. We only assert it
    // fails to deserialize — pinning the message string would be brittle
    // across serde versions.
    let msg = err.to_string();
    assert!(
        msg.contains("value") || msg.contains("encounter"),
        "expected the error to reference the encounter `value` field; got: {msg}",
    );
}
