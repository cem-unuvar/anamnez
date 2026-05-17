//! `*Patch` JSON shape: absent / null / set must round-trip cleanly.

use anamnez_protocol::observation::ObservationPatch;
use anamnez_protocol::patient::PatientPatch;

#[test]
fn patient_patch_absent_field_deserializes_as_none() {
    let p: PatientPatch = serde_json::from_str("{}").unwrap();
    assert!(p.mrn.is_none());
    assert!(p.email.is_none());
}

#[test]
fn patient_patch_null_field_deserializes_as_some_none() {
    let p: PatientPatch = serde_json::from_str(r#"{"mrn": null}"#).unwrap();
    assert!(matches!(p.mrn, Some(None)));
}

#[test]
fn patient_patch_set_field_deserializes_as_some_some() {
    let p: PatientPatch = serde_json::from_str(r#"{"mrn": "ABC-42"}"#).unwrap();
    assert!(matches!(p.mrn, Some(Some(ref s)) if s == "ABC-42"));
}

#[test]
fn patient_patch_unset_fields_are_omitted_on_serialize() {
    let p = PatientPatch {
        mrn: Some(Some("ABC".into())),
        ..PatientPatch::default()
    };
    let s = serde_json::to_string(&p).unwrap();
    assert!(s.contains("\"mrn\":\"ABC\""));
    assert!(!s.contains("preferred_name"));
}

#[test]
fn observation_patch_distinguishes_null_from_absent_and_value() {
    let absent: ObservationPatch = serde_json::from_str("{}").unwrap();
    let null: ObservationPatch = serde_json::from_str(r#"{"code": null}"#).unwrap();
    let set: ObservationPatch = serde_json::from_str(r#"{"code": "X42"}"#).unwrap();
    assert!(absent.code.is_none());
    assert!(matches!(null.code, Some(None)));
    assert!(matches!(set.code, Some(Some(ref s)) if s == "X42"));
}
