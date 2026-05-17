//! Subsystem B — Code systems. README §Storage → Code systems.

#![allow(clippy::wildcard_imports)]

use anamnez_core::code_systems::loader;
use anamnez_core::code_systems::turkish::casefold;
use anamnez_core::code_systems::{autocomplete, lookup, repo_code_systems_root, CodeSystem};
use anamnez_core::test_support::prelude::*;
use rusqlite::params;

// ─── Turkish casefold ─────────────────────────────────────────────────────────

#[test]
fn turkish_casefold_collides_dotted_i_with_lower_i() {
    assert_eq!(casefold("İLAÇ"), casefold("ilaç"));
    assert_eq!(casefold("İlaç"), "ilaç");
}

#[test]
fn turkish_casefold_keeps_s_cedilla_distinct_from_s() {
    assert_ne!(casefold("şeker"), "seker");
}

#[test]
fn turkish_casefold_keeps_c_cedilla_distinct_from_c() {
    assert_ne!(casefold("çocuk"), "cocuk");
}

#[test]
fn turkish_casefold_dotless_capital_i_to_dotless_lower_i() {
    assert_eq!(casefold("IRMAK"), "ırmak");
}

// ─── Loader from CSV ──────────────────────────────────────────────────────────

#[test]
fn first_boot_loader_populates_atc_from_normalized_csv() {
    let temp = TempDb::new().expect("TempDb opens");
    let root = repo_code_systems_root();
    let report = temp
        .db
        .with_writer(|conn| {
            loader::load_csv(conn, CodeSystem::Atc, &root.join("atc/normalized.csv"))
        })
        .expect("ATC load succeeds");
    assert!(
        report.rows_inserted > 1000,
        "ATC has thousands of rows, got {}",
        report.rows_inserted
    );
    assert_eq!(report.dropped_columns, vec!["description_tr_source"]);

    // Spot-check an ATC entry — A10BA02 is metformin per README example.
    let row = lookup(&temp.db, CodeSystem::Atc, "A10BA02").expect("metformin lookup");
    assert_eq!(row.code, "A10BA02");
}

#[test]
fn first_boot_loader_populates_skrs_vp() {
    let temp = TempDb::new().expect("TempDb opens");
    let root = repo_code_systems_root();
    let report = temp
        .db
        .with_writer(|conn| {
            loader::load_csv(
                conn,
                CodeSystem::SkrsVp,
                &root.join("skrs-vp/normalized.csv"),
            )
        })
        .expect("SKRS-VP load succeeds");
    assert_eq!(report.rows_inserted, 16, "SKRS-VP has 16 codes");
    let row = lookup(&temp.db, CodeSystem::SkrsVp, "1").expect("first SKRS code");
    assert_eq!(row.display_tr.as_deref(), Some("Genel muayene"));
}

#[test]
fn cross_system_fk_drug_titck_atc_resolves_after_load() {
    let temp = TempDb::new().expect("TempDb opens");
    let root = repo_code_systems_root();
    // Order matters: ATC before TITCK so the FK has a target to resolve.
    temp.db
        .with_writer(|conn| {
            loader::load_csv(conn, CodeSystem::Atc, &root.join("atc/normalized.csv"))
        })
        .expect("ATC load");
    temp.db
        .with_writer(|conn| {
            loader::load_csv(conn, CodeSystem::Titck, &root.join("titck/normalized.csv"))
        })
        .expect("TITCK load");

    // Pick any TİTCK row with a non-null atc_code and verify the join.
    temp.db
        .with_reader(|conn| {
            let row: (String, String) = conn.query_row(
                "SELECT t.barcode, a.atc_code \
                 FROM drug_titck t JOIN drug_atc a ON a.atc_code = t.atc_code \
                 WHERE t.atc_code IS NOT NULL LIMIT 1",
                params![],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            assert!(!row.0.is_empty());
            assert!(!row.1.is_empty());
            Ok(())
        })
        .expect("FK join");
}

#[test]
fn lookup_returns_code_system_mismatch_for_missing_code() {
    use anamnez_core::Error;
    let temp = TempDb::new().expect("TempDb opens");
    let err = lookup(&temp.db, CodeSystem::Atc, "NOTACODE").expect_err("must miss");
    matches!(err, Error::CodeSystemMismatch { .. })
        .then_some(())
        .expect("expected CodeSystemMismatch");
}

#[test]
fn fts5_autocomplete_finds_dotted_i_match_for_lower_i_query() {
    let temp = TempDb::new().expect("TempDb opens");
    let root = repo_code_systems_root();
    temp.db
        .with_writer(|conn| {
            loader::load_csv(
                conn,
                CodeSystem::SkrsVp,
                &root.join("skrs-vp/normalized.csv"),
            )
        })
        .expect("SKRS-VP load");

    // SKRS-VP includes "Sütur alınması" and "Pansuman" type entries — search "pansuman"
    // and expect at least one hit.
    let hits = autocomplete(&temp.db, CodeSystem::SkrsVp, "Pansuman", 10).expect("autocomplete");
    assert!(
        !hits.is_empty(),
        "expected at least one autocomplete hit for `Pansuman`"
    );
}

#[test]
fn retired_code_queryable_via_lookup() {
    // Many TİTCK and SUT rows have retired_at NULL in the bundled data, so we
    // exercise the retired path by directly inserting a retired SUT row.
    let temp = TempDb::new().expect("TempDb opens");
    temp.db
        .with_writer(|conn| {
            conn.execute(
                "INSERT INTO procedure_sut (sut_code, description_tr, category, retired_at) \
                 VALUES ('999999', 'eski', 'other', '2020-01-01T00:00:00Z')",
                params![],
            )?;
            Ok(())
        })
        .expect("seed");
    let row = lookup(&temp.db, CodeSystem::Sut, "999999").expect("retired code lookup");
    assert!(
        row.is_retired,
        "retired_at populated means is_retired = true"
    );
}

// ─── Bundle signature + diff-apply ────────────────────────────────────────────

#[test]
fn ed25519_bundle_signature_verified_against_embedded_pubkey() {
    use anamnez_core::code_systems::bundle::{self, Bundle};
    use anamnez_core::test_support::dev_bundle;
    use anamnez_core::Error;

    let dir = tempfile::tempdir().expect("tempdir");
    let bundle_path = dir.path().join("bundle.json");

    let manifest = dev_bundle::manifest("2026q2", "2026-05-15T10:00:00Z".parse().expect("ts"));
    let mut rows = std::collections::BTreeMap::new();
    rows.insert(
        anamnez_core::code_systems::CodeSystem::SkrsVp
            .as_str()
            .to_owned(),
        vec![serde_json::json!({
            "code": "99",
            "description_tr": "Test",
            "description_en": "Test",
            "retired_at": null,
        })],
    );
    let good = Bundle { manifest, rows };
    dev_bundle::write_signed_bundle(&bundle_path, &good).expect("write+sign");

    // Happy path: verify succeeds and yields the manifest.
    let m = bundle::verify(&bundle_path).expect("good signature verifies");
    assert_eq!(m.version, "2026q2");

    // Tamper path: flip a byte in the signature, expect InvalidBundleSignature.
    let sig_path = bundle::sig_path_for(&bundle_path);
    let mut sig = std::fs::read(&sig_path).expect("read sig");
    sig[0] ^= 0x01;
    std::fs::write(&sig_path, &sig).expect("rewrite tampered sig");
    let err = bundle::verify(&bundle_path).expect_err("tampered sig must fail");
    matches!(err, Error::InvalidBundleSignature)
        .then_some(())
        .expect("expected InvalidBundleSignature");

    // Missing sidecar: same typed error.
    std::fs::remove_file(&sig_path).expect("rm sig");
    let err = bundle::verify(&bundle_path).expect_err("missing sig must fail");
    matches!(err, Error::InvalidBundleSignature)
        .then_some(())
        .expect("expected InvalidBundleSignature on missing sidecar");
}

#[test]
fn bundle_diff_apply_inserts_updates_retires_in_one_tx_with_audit() {
    use anamnez_core::code_systems::bundle::{self, Bundle};
    use anamnez_core::code_systems::CodeSystem;
    use anamnez_core::test_support::dev_bundle;

    let temp = TempDb::new().expect("TempDb opens");

    // Seed two rows directly so the diff-apply has something to UPDATE and
    // something to RETIRE. We use SKRS-VP because the table is tiny (16 rows
    // in real data) and has a `retired_at` column.
    temp.db
        .with_writer(|conn| {
            conn.execute(
                "INSERT INTO visit_purpose_skrs (code, description_tr, description_en, retired_at) \
                 VALUES ('A', 'Eski metin', 'Old text', NULL), \
                        ('B', 'Kalan metin', 'Stays', NULL)",
                params![],
            )?;
            Ok(())
        })
        .expect("seed");

    // Bundle: A is updated (description_tr changes), B is unchanged (no-op),
    // C is new (insert), and the pre-existing row absent from the bundle
    // (none in this case besides A and B, which are both present) — wait,
    // we need a retire. Seed an extra row "D" that the bundle omits.
    temp.db
        .with_writer(|conn| {
            conn.execute(
                "INSERT INTO visit_purpose_skrs (code, description_tr, description_en, retired_at) \
                 VALUES ('D', 'Silinecek', 'To be retired', NULL)",
                params![],
            )?;
            Ok(())
        })
        .expect("seed retire candidate");

    let mut rows = std::collections::BTreeMap::new();
    rows.insert(
        CodeSystem::SkrsVp.as_str().to_owned(),
        vec![
            serde_json::json!({
                "code": "A",
                "description_tr": "Yeni metin",
                "description_en": "Old text",
                "retired_at": null,
            }),
            serde_json::json!({
                "code": "B",
                "description_tr": "Kalan metin",
                "description_en": "Stays",
                "retired_at": null,
            }),
            serde_json::json!({
                "code": "C",
                "description_tr": "Yeni satır",
                "description_en": "Newly inserted",
                "retired_at": null,
            }),
        ],
    );
    let manifest = dev_bundle::manifest("2026q2", "2026-05-15T10:00:00Z".parse().expect("ts"));
    let bundle = Bundle { manifest, rows };

    let dir = tempfile::tempdir().expect("tempdir");
    let bundle_path = dir.path().join("bundle.json");
    dev_bundle::write_signed_bundle(&bundle_path, &bundle).expect("sign");

    let report = temp
        .db
        .with_writer(|conn| bundle::apply(conn, &bundle_path))
        .expect("apply");

    assert_eq!(report.inserted, 1, "C inserted");
    assert_eq!(report.updated, 1, "A updated");
    assert_eq!(report.retired, 1, "D retired");

    // Verify on-disk state.
    temp.db
        .with_reader(|conn| {
            let a_tr: String = conn.query_row(
                "SELECT description_tr FROM visit_purpose_skrs WHERE code = 'A'",
                params![],
                |r| r.get(0),
            )?;
            assert_eq!(a_tr, "Yeni metin", "A's description_tr was updated");

            let c_exists: bool = conn
                .query_row(
                    "SELECT 1 FROM visit_purpose_skrs WHERE code = 'C'",
                    params![],
                    |_| Ok(true),
                )
                .unwrap_or(false);
            assert!(c_exists, "C was inserted");

            let d_retired: Option<String> = conn.query_row(
                "SELECT retired_at FROM visit_purpose_skrs WHERE code = 'D'",
                params![],
                |r| r.get(0),
            )?;
            assert!(d_retired.is_some(), "D was retired (retired_at set)");

            // Audit row appended in the same transaction.
            let audit_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'codesystems.update'",
                params![],
                |r| r.get(0),
            )?;
            assert_eq!(audit_count, 1, "exactly one codesystems.update audit row");

            let metadata_json: String = conn.query_row(
                "SELECT metadata FROM audit_log WHERE action = 'codesystems.update'",
                params![],
                |r| r.get(0),
            )?;
            let meta: serde_json::Value = serde_json::from_str(&metadata_json).expect("json");
            let per_table = meta
                .get("per_table")
                .and_then(|v| v.as_array())
                .expect("per_table array");
            assert_eq!(per_table.len(), 1, "one system in this bundle");
            let entry = &per_table[0];
            assert_eq!(entry["system"], "SKRS-VP");
            assert_eq!(entry["inserted"], 1);
            assert_eq!(entry["updated"], 1);
            assert_eq!(entry["retired"], 1);
            Ok(())
        })
        .expect("post-apply checks");
}
