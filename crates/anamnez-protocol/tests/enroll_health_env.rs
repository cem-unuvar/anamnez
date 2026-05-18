//! Round-trip the new wire types added with the workstation slice.

use anamnez_protocol::enroll::{EnrollExchangeRequest, EnrollExchangeResponse};
use anamnez_protocol::environment::Environment;
use anamnez_protocol::health::HealthEnvelope;
use anamnez_protocol::ids::WorkstationId;
use uuid::Uuid;

#[test]
fn environment_renames_lowercase() {
    let s = serde_json::to_string(&Environment::Production).unwrap();
    assert_eq!(s, "\"production\"");
    let s = serde_json::to_string(&Environment::Test).unwrap();
    assert_eq!(s, "\"test\"");
}

#[test]
fn enroll_request_round_trip() {
    let req = EnrollExchangeRequest {
        token: "deadbeef".into(),
        client_version: "1.2.3".into(),
    };
    let s = serde_json::to_string(&req).unwrap();
    let back: EnrollExchangeRequest = serde_json::from_str(&s).unwrap();
    assert_eq!(back.token, "deadbeef");
    assert_eq!(back.client_version, "1.2.3");
}

#[test]
fn enroll_response_round_trip() {
    let resp = EnrollExchangeResponse {
        workstation_id: WorkstationId(Uuid::nil()),
        client_cert_pem: "-----BEGIN CERTIFICATE-----\n…\n".into(),
        client_key_pem: "-----BEGIN PRIVATE KEY-----\n…\n".into(),
        ca_cert_pem: "-----BEGIN CERTIFICATE-----\n…\n".into(),
    };
    let s = serde_json::to_string(&resp).unwrap();
    let back: EnrollExchangeResponse = serde_json::from_str(&s).unwrap();
    assert_eq!(back.workstation_id.as_uuid(), Uuid::nil());
    assert!(back.client_cert_pem.contains("CERTIFICATE"));
}

#[test]
fn health_envelope_round_trip() {
    let env = HealthEnvelope {
        status: "ok".into(),
        version: "0.1.0".into(),
        environment: Environment::Test,
    };
    let s = serde_json::to_string(&env).unwrap();
    assert!(s.contains("\"environment\":\"test\""));
    let back: HealthEnvelope = serde_json::from_str(&s).unwrap();
    assert_eq!(back.environment, Environment::Test);
}
