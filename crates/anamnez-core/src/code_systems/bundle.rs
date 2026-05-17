//! Signed code-systems bundle apply (README §Storage → Bundle distribution).
//!
//! **On-disk format.** A bundle is a single JSON document on disk holding the
//! manifest and per-system row payload, accompanied by a sidecar Ed25519
//! signature at `<bundle_path>.sig` over the bundle bytes. README calls the
//! shipped artifact `anamnez-codesystems-<YYYYqN>.tar.zst.sig`; the zstd-of-tar
//! encoding is a later optimization. The MVP-internal format here is
//! deliberately simple so we can sign, verify, and diff-apply without pulling
//! in tar+zstd just to round-trip a handful of rows.
//!
//! **Apply pipeline.** Verify signature against the embedded pubkey → begin one
//! transaction → for each system, insert rows new to the bundle, update changed
//! rows by PK, and retire rows present in the DB but absent from the bundle
//! (only for tables that carry a `retired_at` column — `drug_titck`,
//! `procedure_sut`, `visit_purpose_skrs`, `symptom_anamnez`) → append one
//! `codesystems.update` audit row with per-table counts → commit.

use crate::audit;
use crate::audit::action::Action;
use crate::code_systems::pubkey;
use crate::code_systems::CodeSystem;
use crate::error::{Error, Result};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use jiff::Timestamp;
use rusqlite::{params, params_from_iter, types::Value as SqlValue, Connection, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleManifest {
    pub version: String,
    pub checksum_sha256: String,
    pub built_at: Timestamp,
    pub source_revision_dates: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub manifest: BundleManifest,
    /// Keyed by `CodeSystem::as_str()`. Row shape matches the lookup-table
    /// columns (see `migrations/V0001__schema.sql`).
    pub rows: BTreeMap<String, Vec<JsonValue>>,
}

#[derive(Debug, Clone, Default)]
pub struct ApplyReport {
    pub inserted: usize,
    pub updated: usize,
    pub retired: usize,
    pub manifest: Option<BundleManifest>,
}

/// Verify a bundle's signature against the embedded dev pubkey and parse the
/// manifest. Returns `Error::InvalidBundleSignature` on any verification or
/// parse failure — bundles that don't round-trip are treated as untrusted, per
/// README §Development "fail loudly".
pub fn verify(bundle_path: &Path) -> Result<BundleManifest> {
    Ok(read_and_verify(bundle_path)?.manifest)
}

/// Verify and apply a bundle to `conn` in one transaction. Returns
/// `Error::InvalidBundleSignature` on signature failure.
pub fn apply(conn: &mut Connection, bundle_path: &Path) -> Result<ApplyReport> {
    let bundle = read_and_verify(bundle_path)?;
    let tx = conn.transaction()?;

    let retired_at = bundle.manifest.built_at;
    let mut report = ApplyReport {
        manifest: Some(bundle.manifest.clone()),
        ..ApplyReport::default()
    };
    let mut per_table: Vec<(&'static str, TableCounts)> = Vec::new();

    for (system_tag, rows) in &bundle.rows {
        let system = CodeSystem::parse_tag(system_tag)?;
        let counts = apply_system(&tx, system, rows, retired_at)?;
        report.inserted += counts.inserted;
        report.updated += counts.updated;
        report.retired += counts.retired;
        per_table.push((system.as_str(), counts));
    }

    let metadata = serde_json::json!({
        "version": bundle.manifest.version,
        "built_at": bundle.manifest.built_at.to_string(),
        "per_table": per_table
            .iter()
            .map(|(name, c)| serde_json::json!({
                "system": name,
                "inserted": c.inserted,
                "updated": c.updated,
                "retired": c.retired,
            }))
            .collect::<Vec<_>>(),
    });

    audit::append_in_conn(
        &tx,
        retired_at,
        audit::AppendInput {
            actor_user_id: None,
            auth_session_id: None,
            action: Action::CodesystemsUpdate,
            target_type: "codesystems_bundle".to_owned(),
            target_id: bundle.manifest.version.clone(),
            patient_id: None,
            metadata,
        },
    )?;

    tx.commit()?;
    Ok(report)
}

/// Path of the sidecar signature file for `bundle_path`. Public because the
/// signing tool (`xtask record-fixture`-style, or test setup) writes here.
#[must_use]
pub fn sig_path_for(bundle_path: &Path) -> PathBuf {
    let mut p = bundle_path.as_os_str().to_owned();
    p.push(".sig");
    PathBuf::from(p)
}

// ─── internals ────────────────────────────────────────────────────────────────

fn read_and_verify(bundle_path: &Path) -> Result<Bundle> {
    let bytes = std::fs::read(bundle_path)?;
    let sig_bytes =
        std::fs::read(sig_path_for(bundle_path)).map_err(|_| Error::InvalidBundleSignature)?;
    if sig_bytes.len() != Signature::BYTE_SIZE {
        return Err(Error::InvalidBundleSignature);
    }
    let mut sig_arr = [0u8; Signature::BYTE_SIZE];
    sig_arr.copy_from_slice(&sig_bytes);
    let signature = Signature::from_bytes(&sig_arr);

    let pubkey_bytes = pubkey::embedded();
    if pubkey_bytes.len() != 32 {
        return Err(Error::InvalidBundleSignature);
    }
    let mut pk_arr = [0u8; 32];
    pk_arr.copy_from_slice(pubkey_bytes);
    let verifying = VerifyingKey::from_bytes(&pk_arr).map_err(|_| Error::InvalidBundleSignature)?;

    verifying
        .verify(&bytes, &signature)
        .map_err(|_| Error::InvalidBundleSignature)?;

    serde_json::from_slice(&bytes).map_err(|_| Error::InvalidBundleSignature)
}

#[derive(Debug, Default, Clone, Copy)]
struct TableCounts {
    inserted: usize,
    updated: usize,
    retired: usize,
}

fn apply_system(
    tx: &Transaction,
    system: CodeSystem,
    rows: &[JsonValue],
    retired_at: Timestamp,
) -> Result<TableCounts> {
    let spec = table_spec(system);
    let mut bundle_pks = BTreeSet::<String>::new();
    let mut counts = TableCounts::default();

    for row in rows {
        let obj = row
            .as_object()
            .ok_or(Error::Invariant("bundle row must be a JSON object"))?;
        let pk_value = obj
            .get(spec.pk_column)
            .ok_or(Error::Invariant("bundle row missing primary key"))?;
        let pk = json_as_text(pk_value).ok_or(Error::Invariant("bundle row PK must be text"))?;
        bundle_pks.insert(pk.clone());

        let new_values: Vec<SqlValue> = spec
            .columns
            .iter()
            .map(|c| json_to_sql(obj.get(*c).unwrap_or(&JsonValue::Null)))
            .collect();

        let existing = fetch_row(tx, spec, &pk)?;
        match existing {
            None => {
                let placeholders = (1..=spec.columns.len())
                    .map(|i| format!("?{i}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let sql = format!(
                    "INSERT INTO {} ({}) VALUES ({})",
                    spec.table,
                    spec.columns.join(", "),
                    placeholders
                );
                tx.execute(&sql, params_from_iter(new_values.iter()))?;
                counts.inserted += 1;
            }
            Some(prev) if !sql_values_equal(&prev, &new_values) => {
                let set_clause = spec
                    .columns
                    .iter()
                    .enumerate()
                    .map(|(i, c)| format!("{c} = ?{}", i + 1))
                    .collect::<Vec<_>>()
                    .join(", ");
                let sql = format!(
                    "UPDATE {} SET {} WHERE {} = ?{}",
                    spec.table,
                    set_clause,
                    spec.pk_column,
                    spec.columns.len() + 1
                );
                let mut p = new_values.clone();
                p.push(SqlValue::Text(pk.clone()));
                tx.execute(&sql, params_from_iter(p.iter()))?;
                counts.updated += 1;
            }
            Some(_) => { /* identical row, no-op */ }
        }
    }

    if let Some(retired_col) = spec.retired_at_column {
        let mut stmt = tx.prepare(&format!(
            "SELECT {pk} FROM {table} WHERE {retired_col} IS NULL",
            pk = spec.pk_column,
            table = spec.table,
            retired_col = retired_col,
        ))?;
        let live_pks: Vec<String> = stmt
            .query_map(params![], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let retired_ts = retired_at.to_string();
        for pk in live_pks {
            if !bundle_pks.contains(&pk) {
                tx.execute(
                    &format!(
                        "UPDATE {table} SET {retired_col} = ?1 WHERE {pk_col} = ?2",
                        table = spec.table,
                        retired_col = retired_col,
                        pk_col = spec.pk_column,
                    ),
                    params![retired_ts, pk],
                )?;
                counts.retired += 1;
            }
        }
    }

    Ok(counts)
}

fn fetch_row(tx: &Transaction, spec: &TableSpec, pk: &str) -> Result<Option<Vec<SqlValue>>> {
    let sql = format!(
        "SELECT {} FROM {} WHERE {} = ?1",
        spec.columns.join(", "),
        spec.table,
        spec.pk_column
    );
    let mut stmt = tx.prepare(&sql)?;
    let mut rows = stmt.query(params![pk])?;
    if let Some(row) = rows.next()? {
        let mut out = Vec::with_capacity(spec.columns.len());
        for i in 0..spec.columns.len() {
            out.push(row.get::<_, SqlValue>(i)?);
        }
        Ok(Some(out))
    } else {
        Ok(None)
    }
}

fn sql_values_equal(a: &[SqlValue], b: &[SqlValue]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).all(|(x, y)| match (x, y) {
        (SqlValue::Null, SqlValue::Null) => true,
        (SqlValue::Integer(i), SqlValue::Integer(j)) => i == j,
        (SqlValue::Real(i), SqlValue::Real(j)) => i.to_bits() == j.to_bits(),
        (SqlValue::Text(i), SqlValue::Text(j)) => i == j,
        (SqlValue::Blob(i), SqlValue::Blob(j)) => i == j,
        // Cross-type: JSON Number → Integer on DB side might compare against
        // Real-on-DB-side after rusqlite mapping. Treat numeric cross-type as
        // unequal to be safe; the diff-apply will overwrite, which is correct.
        _ => false,
    })
}

struct TableSpec {
    table: &'static str,
    pk_column: &'static str,
    columns: &'static [&'static str],
    retired_at_column: Option<&'static str>,
}

fn table_spec(system: CodeSystem) -> &'static TableSpec {
    match system {
        CodeSystem::Atc => &TableSpec {
            table: "drug_atc",
            pk_column: "atc_code",
            columns: &["atc_code", "description_en", "description_tr"],
            retired_at_column: None,
        },
        CodeSystem::Titck => &TableSpec {
            table: "drug_titck",
            pk_column: "barcode",
            columns: &[
                "barcode",
                "titck_product_code",
                "trade_name",
                "manufacturer",
                "atc_code",
                "active_substance_tr",
                "dosage_form",
                "strength_value",
                "strength_unit",
                "strength_text",
                "package_size_text",
                "rx_only",
                "reimbursable",
                "retired_at",
            ],
            retired_at_column: Some("retired_at"),
        },
        CodeSystem::Icd10Tm => &TableSpec {
            table: "icd10_tm",
            pk_column: "code",
            columns: &[
                "code",
                "description_tr",
                "description_en",
                "parent_code",
                "is_billable",
            ],
            retired_at_column: None,
        },
        CodeSystem::Loinc => &TableSpec {
            table: "loinc",
            pk_column: "code",
            columns: &[
                "code",
                "long_name_en",
                "long_name_tr",
                "component",
                "unit_default",
                "scale_typ",
            ],
            retired_at_column: None,
        },
        CodeSystem::Sut => &TableSpec {
            table: "procedure_sut",
            pk_column: "sut_code",
            columns: &["sut_code", "description_tr", "category", "retired_at"],
            retired_at_column: Some("retired_at"),
        },
        CodeSystem::SkrsVp => &TableSpec {
            table: "visit_purpose_skrs",
            pk_column: "code",
            columns: &["code", "description_tr", "description_en", "retired_at"],
            retired_at_column: Some("retired_at"),
        },
        CodeSystem::AnamnezSym => &TableSpec {
            table: "symptom_anamnez",
            pk_column: "code",
            columns: &[
                "code",
                "display_tr",
                "display_en",
                "icd10_suggestion",
                "body_region",
                "retired_at",
            ],
            retired_at_column: Some("retired_at"),
        },
    }
}

fn json_as_text(v: &JsonValue) -> Option<String> {
    match v {
        JsonValue::String(s) => Some(s.clone()),
        JsonValue::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn json_to_sql(v: &JsonValue) -> SqlValue {
    match v {
        JsonValue::Null => SqlValue::Null,
        JsonValue::Bool(b) => SqlValue::Integer(i64::from(*b)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                SqlValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                SqlValue::Real(f)
            } else {
                SqlValue::Text(n.to_string())
            }
        }
        JsonValue::String(s) => SqlValue::Text(s.clone()),
        other => SqlValue::Text(other.to_string()),
    }
}
