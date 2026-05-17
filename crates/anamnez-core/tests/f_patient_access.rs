//! Subsystem F — Patient access. README §Tenancy.

#![allow(clippy::wildcard_imports)]

use anamnez_core::ids::UserId;
use anamnez_core::patient::{self, NewPatient, SexAssignedAtBirth};
use anamnez_core::patient_access::{self, caps, AccessLevel};
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
fn caps_match_documented_levels() {
    assert!(caps::can_read(AccessLevel::Owner));
    assert!(caps::can_read(AccessLevel::Collaborator));
    assert!(caps::can_read(AccessLevel::ReadOnly));

    assert!(caps::can_write_clinical(AccessLevel::Owner));
    assert!(caps::can_write_clinical(AccessLevel::Collaborator));
    assert!(!caps::can_write_clinical(AccessLevel::ReadOnly));

    assert!(caps::can_manage_access(AccessLevel::Owner));
    assert!(!caps::can_manage_access(AccessLevel::Collaborator));
    assert!(!caps::can_manage_access(AccessLevel::ReadOnly));

    assert!(caps::can_transfer_ownership(AccessLevel::Owner));
    assert!(!caps::can_transfer_ownership(AccessLevel::Collaborator));
}

#[test]
fn partial_unique_index_allows_only_one_owner_per_patient() {
    let temp = TempDb::new().expect("TempDb opens");
    temp.db
        .with_writer(|conn| {
            conn.execute(
                "INSERT INTO user (id, email, display_name, role, password_hash, created_at) \
                 VALUES ('u1', 'a@x', 'Alice', 'provider', '!', '2026-01-01T00:00:00Z')",
                params![],
            )?;
            conn.execute(
                "INSERT INTO user (id, email, display_name, role, password_hash, created_at) \
                 VALUES ('u2', 'b@x', 'Bob', 'provider', '!', '2026-01-01T00:00:00Z')",
                params![],
            )?;
            conn.execute(
                "INSERT INTO patient \
                 (id, given_names, family_name, date_of_birth, sex_assigned_at_birth, created_by, created_at, updated_at) \
                 VALUES ('p1', '[TEST] A', '[TEST] L', '2000-01-01', 'female', 'u1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![],
            )?;
            conn.execute(
                "INSERT INTO patient_access (patient_id, user_id, level) VALUES ('p1', 'u1', 'owner')",
                params![],
            )?;
            let err = conn
                .execute(
                    "INSERT INTO patient_access (patient_id, user_id, level) VALUES ('p1', 'u2', 'owner')",
                    params![],
                )
                .err()
                .expect("second owner must be rejected");
            assert!(format!("{err}").to_ascii_lowercase().contains("unique"));
            Ok(())
        })
        .expect("partial unique test");
}

#[test]
fn creator_of_patient_auto_inserted_as_owner() {
    let temp = TempDb::new().expect("TempDb opens");
    let creator = seed_user(&temp, "alice");
    let patient_id = fresh_patient(&temp, creator);

    let level = patient_access::level_for(&temp.db, creator, patient_id).expect("level lookup");
    assert_eq!(level, Some(AccessLevel::Owner));
}

#[test]
fn missing_access_row_returns_not_found_not_forbidden() {
    let temp = TempDb::new().expect("TempDb opens");
    let creator = seed_user(&temp, "alice");
    let stranger = seed_user(&temp, "stranger");
    let patient_id = fresh_patient(&temp, creator);

    let err = patient::get(&temp.db, stranger, patient_id).expect_err("stranger has no access");
    matches!(err, Error::NotFound)
        .then_some(())
        .expect("expected NotFound to hide existence");
}

#[test]
fn grant_collaborator_then_revoke() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let collab = seed_user(&temp, "bob");
    let patient_id = fresh_patient(&temp, owner);

    patient_access::grant(
        &temp.db,
        owner,
        patient_id,
        collab,
        AccessLevel::Collaborator,
    )
    .expect("grant");
    assert_eq!(
        patient_access::level_for(&temp.db, collab, patient_id).expect("level"),
        Some(AccessLevel::Collaborator)
    );

    patient_access::revoke(&temp.db, owner, patient_id, collab).expect("revoke");
    assert_eq!(
        patient_access::level_for(&temp.db, collab, patient_id).expect("level"),
        None
    );
}

#[test]
fn grant_owner_directly_is_rejected() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner = seed_user(&temp, "alice");
    let new_user = seed_user(&temp, "bob");
    let patient_id = fresh_patient(&temp, owner);

    let err = patient_access::grant(&temp.db, owner, patient_id, new_user, AccessLevel::Owner)
        .expect_err("grant of owner must be rejected");
    matches!(err, Error::Invariant(_))
        .then_some(())
        .expect("expected Invariant rejection");
}

#[test]
fn ownership_transfer_demotes_and_promotes_in_one_transaction() {
    let temp = TempDb::new().expect("TempDb opens");
    let owner_a = seed_user(&temp, "alice");
    let owner_b = seed_user(&temp, "bob");
    let patient_id = fresh_patient(&temp, owner_a);

    patient_access::transfer_ownership(&temp.db, owner_a, patient_id, owner_a, owner_b)
        .expect("transfer");

    assert_eq!(
        patient_access::level_for(&temp.db, owner_a, patient_id).expect("a level"),
        Some(AccessLevel::Collaborator),
        "previous owner should be demoted to collaborator"
    );
    assert_eq!(
        patient_access::level_for(&temp.db, owner_b, patient_id).expect("b level"),
        Some(AccessLevel::Owner),
        "successor should be promoted to owner"
    );

    let all = patient_access::list(&temp.db, patient_id).expect("list");
    let owners: Vec<_> = all
        .iter()
        .filter(|r| matches!(r.level, AccessLevel::Owner))
        .collect();
    assert_eq!(owners.len(), 1);
}

#[test]
fn test_environment_requires_test_prefix_on_patient_name() {
    let temp = TempDb::new().expect("TempDb opens");
    let creator = seed_user(&temp, "alice");
    let err = patient::create(
        &temp.db,
        creator,
        NewPatient {
            mrn: None,
            given_names: "Real".into(),
            family_name: "Name".into(),
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
    .expect_err("test env must reject non-[TEST] name");
    matches!(err, Error::TestPrefixRequired)
        .then_some(())
        .expect("expected TestPrefixRequired");
}
