//! Subsystem C — Audit log integrity. README §Storage → Audit log integrity.

#![allow(clippy::wildcard_imports)]

use anamnez_core::audit::{self, Action, AppendInput};
use anamnez_core::test_support::prelude::*;
use rusqlite::params;
use serde_json::json;

#[test]
fn before_update_trigger_aborts_with_audit_immutable_message() {
    let temp = TempDb::new().expect("TempDb opens");
    temp.db
        .with_writer(|conn| {
            conn.execute(
                "INSERT INTO audit_log \
                 (occurred_at, actor_user_id, auth_session_id, action, target_type, target_id, patient_id, metadata, prev_hash, row_hash) \
                 VALUES ('2026-01-01T00:00:00Z', NULL, NULL, 'patient.view', 'patient', 'p1', NULL, '{}', X'00', X'00')",
                params![],
            )?;
            let err = conn
                .execute("UPDATE audit_log SET action = 'observation.create'", params![])
                .err()
                .expect("update on audit_log must be aborted");
            assert!(format!("{err}").contains("audit immutable"));
            Ok(())
        })
        .expect("trigger smoke");
}

#[test]
fn before_delete_trigger_aborts_with_audit_immutable_message() {
    let temp = TempDb::new().expect("TempDb opens");
    temp.db
        .with_writer(|conn| {
            conn.execute(
                "INSERT INTO audit_log \
                 (occurred_at, actor_user_id, auth_session_id, action, target_type, target_id, patient_id, metadata, prev_hash, row_hash) \
                 VALUES ('2026-01-01T00:00:00Z', NULL, NULL, 'patient.view', 'patient', 'p1', NULL, '{}', X'00', X'00')",
                params![],
            )?;
            let err = conn
                .execute("DELETE FROM audit_log", params![])
                .err()
                .expect("delete must be aborted");
            assert!(format!("{err}").contains("audit immutable"));
            Ok(())
        })
        .expect("trigger smoke");
}

#[test]
fn append_genesis_row_uses_zero_prev_hash() {
    let temp = TempDb::new().expect("TempDb opens");
    let id = audit::append(
        &temp.db,
        AppendInput {
            actor_user_id: None,
            auth_session_id: None,
            action: Action::PatientView,
            target_type: "patient".into(),
            target_id: "p1".into(),
            patient_id: None,
            metadata: json!({}),
        },
    )
    .expect("genesis append succeeds");
    assert_eq!(id.as_i64(), 1);

    temp.db
        .with_reader(|conn| {
            let prev_hash: Vec<u8> = conn.query_row(
                "SELECT prev_hash FROM audit_log WHERE id = 1",
                params![],
                |r| r.get(0),
            )?;
            assert_eq!(prev_hash, vec![0u8; 32]);
            Ok(())
        })
        .expect("genesis prev_hash check");
}

#[test]
fn append_chains_prev_hash_to_previous_row_hash() {
    let temp = TempDb::new().expect("TempDb opens");
    let a = audit::append(
        &temp.db,
        AppendInput {
            actor_user_id: None,
            auth_session_id: None,
            action: Action::PatientView,
            target_type: "patient".into(),
            target_id: "p1".into(),
            patient_id: None,
            metadata: json!({"phase": 1}),
        },
    )
    .expect("first append");
    let _b = audit::append(
        &temp.db,
        AppendInput {
            actor_user_id: None,
            auth_session_id: None,
            action: Action::PatientView,
            target_type: "patient".into(),
            target_id: "p2".into(),
            patient_id: None,
            metadata: json!({"phase": 2}),
        },
    )
    .expect("second append");

    temp.db
        .with_reader(|conn| {
            let row_hash_a: Vec<u8> = conn.query_row(
                "SELECT row_hash FROM audit_log WHERE id = ?1",
                params![a.as_i64()],
                |r| r.get(0),
            )?;
            let prev_hash_b: Vec<u8> = conn.query_row(
                "SELECT prev_hash FROM audit_log WHERE id = ?1",
                params![a.as_i64() + 1],
                |r| r.get(0),
            )?;
            assert_eq!(row_hash_a, prev_hash_b, "B.prev_hash must equal A.row_hash");
            Ok(())
        })
        .expect("chain check");
}

#[test]
fn row_hash_composition_matches_spec_for_known_inputs() {
    let temp = TempDb::new().expect("TempDb opens");
    // patient_id stays None so we don't have to seed a patient row (FK).
    let id = audit::append(
        &temp.db,
        AppendInput {
            actor_user_id: None,
            auth_session_id: None,
            action: Action::ObservationCreate,
            target_type: "observation".into(),
            target_id: "obs-1".into(),
            patient_id: None,
            metadata: json!({"k":"v"}),
        },
    )
    .expect("append");

    temp.db
        .with_reader(|conn| {
            let row = conn.query_row(
                "SELECT prev_hash, row_hash, occurred_at FROM audit_log WHERE id = ?1",
                params![id.as_i64()],
                |r| {
                    let prev: Vec<u8> = r.get(0)?;
                    let stored: Vec<u8> = r.get(1)?;
                    let occ: String = r.get(2)?;
                    Ok((prev, stored, occ))
                },
            )?;
            let occ: jiff::Timestamp = row.2.parse().expect("ts parse");
            let recomputed = anamnez_core::audit::hash::compute(
                &row.0,
                id,
                occ,
                None,
                None,
                Action::ObservationCreate,
                "observation",
                "obs-1",
                None,
                &json!({"k":"v"}),
            );
            assert_eq!(row.1, recomputed.to_vec());
            Ok(())
        })
        .expect("compose check");
}

#[test]
fn verify_chain_passes_on_clean_chain() {
    let temp = TempDb::new().expect("TempDb opens");
    for i in 0..5 {
        audit::append(
            &temp.db,
            AppendInput {
                actor_user_id: None,
                auth_session_id: None,
                action: Action::PatientView,
                target_type: "patient".into(),
                target_id: format!("p{i}"),
                patient_id: None,
                metadata: json!({"i": i}),
            },
        )
        .expect("append");
    }
    let report = audit::verify::verify_chain(&temp.db).expect("chain intact");
    assert_eq!(report.rows_verified, 5);
}

#[test]
fn verify_chain_returns_audit_tamper_with_offending_row_id() {
    use anamnez_core::Error;

    let temp = TempDb::new().expect("TempDb opens");
    let _ = audit::append(
        &temp.db,
        AppendInput {
            actor_user_id: None,
            auth_session_id: None,
            action: Action::PatientView,
            target_type: "patient".into(),
            target_id: "p1".into(),
            patient_id: None,
            metadata: json!({"phase": 1}),
        },
    )
    .expect("first append");
    let b = audit::append(
        &temp.db,
        AppendInput {
            actor_user_id: None,
            auth_session_id: None,
            action: Action::PatientView,
            target_type: "patient".into(),
            target_id: "p2".into(),
            patient_id: None,
            metadata: json!({"phase": 2}),
        },
    )
    .expect("second append");

    // Simulate an attacker with raw file access: drop the protective trigger and
    // mutate the row_hash so the chain no longer verifies.
    temp.db
        .with_writer(|conn| {
            conn.execute("DROP TRIGGER trg_audit_log_no_update", params![])?;
            conn.execute(
                "UPDATE audit_log SET row_hash = X'FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF' WHERE id = ?1",
                params![b.as_i64()],
            )?;
            Ok(())
        })
        .expect("tamper");

    let err = audit::verify::verify_chain(&temp.db).expect_err("must detect tamper");
    matches!(err, Error::AuditTamper { row_id } if row_id == b.as_i64())
        .then_some(())
        .expect("expected AuditTamper at tampered row id");
}
