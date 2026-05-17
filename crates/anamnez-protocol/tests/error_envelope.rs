//! `ErrorEnvelope` JSON: every variant round-trips through serde.

use anamnez_protocol::error::ErrorEnvelope;

fn rt(v: ErrorEnvelope) -> ErrorEnvelope {
    let s = serde_json::to_string(&v).expect("serialize");
    serde_json::from_str(&s).expect("deserialize")
}

#[test]
fn conflict_carries_version_and_state() {
    let env = ErrorEnvelope::Conflict {
        current_version: 7,
        new_state_json: "{\"id\":\"x\"}".into(),
    };
    let back = rt(env);
    match back {
        ErrorEnvelope::Conflict {
            current_version,
            new_state_json,
        } => {
            assert_eq!(current_version, 7);
            assert!(new_state_json.contains("\"id\""));
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn step_up_required_carries_action() {
    let env = ErrorEnvelope::StepUpRequired {
        action: "user.create".into(),
    };
    let s = serde_json::to_string(&env).unwrap();
    assert!(s.contains("\"kind\":\"step_up_required\""));
    assert!(s.contains("\"action\":\"user.create\""));
}

#[test]
fn outdated_client_kinds() {
    let env = ErrorEnvelope::OutdatedClient {
        min: "1.2.3".into(),
        got: "1.0.0".into(),
    };
    let s = serde_json::to_string(&env).unwrap();
    assert!(s.contains("\"kind\":\"outdated_client\""));
}

#[test]
fn all_variants_round_trip() {
    for env in [
        ErrorEnvelope::NotFound,
        ErrorEnvelope::Forbidden,
        ErrorEnvelope::BadCredentials,
        ErrorEnvelope::Revoked,
        ErrorEnvelope::SessionExpired,
        ErrorEnvelope::TestPrefixRequired,
        ErrorEnvelope::RetiredCode { code: "X".into() },
        ErrorEnvelope::Internal {
            detail: "internal error".into(),
        },
    ] {
        let s = serde_json::to_string(&env).unwrap();
        let _back: ErrorEnvelope = serde_json::from_str(&s).unwrap();
    }
}
