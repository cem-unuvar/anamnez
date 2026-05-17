//! First-boot CSV bootstrap: read `code-systems/<system>/normalized.csv` and populate
//! the lookup tables in one transaction.
//!
//! Extra provenance columns (`description_tr_source` on ATC, `long_name_tr_source` on LOINC)
//! present in the CSV but not in the README schema are dropped at load time and recorded
//! in the `LoadReport`.

use crate::code_systems::turkish::casefold;
use crate::code_systems::CodeSystem;
use crate::error::{Error, Result};
use rusqlite::{params, Transaction};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct LoadReport {
    pub system: CodeSystem,
    pub rows_inserted: usize,
    pub dropped_columns: Vec<String>,
}

/// Load one system from its `normalized.csv`. Each call runs in its own transaction.
pub fn load_csv(
    conn: &mut rusqlite::Connection,
    system: CodeSystem,
    csv_path: &Path,
) -> Result<LoadReport> {
    let tx = conn.transaction()?;
    let report = match system {
        CodeSystem::Atc => load_atc(&tx, csv_path)?,
        CodeSystem::Titck => load_titck(&tx, csv_path)?,
        CodeSystem::Icd10Tm => load_icd10tm(&tx, csv_path)?,
        CodeSystem::Loinc => load_loinc(&tx, csv_path)?,
        CodeSystem::Sut => load_sut(&tx, csv_path)?,
        CodeSystem::SkrsVp => load_skrs_vp(&tx, csv_path)?,
        CodeSystem::AnamnezSym => load_anamnez_sym(&tx, csv_path)?,
    };
    tx.commit()?;
    Ok(report)
}

/// Load all seven systems from the canonical `code-systems/` directory.
pub fn bootstrap_from_repo(
    conn: &mut rusqlite::Connection,
    code_systems_root: &Path,
) -> Result<Vec<LoadReport>> {
    // Order matters for foreign keys: ATC must be loaded before TİTCK because
    // `drug_titck.atc_code` references `drug_atc(atc_code)`. ICD-10-TM must be
    // loaded before ANAMNEZ-SYM because `symptom_anamnez.icd10_suggestion`
    // references `icd10_tm(code)`.
    let systems = [
        (CodeSystem::Atc, "atc"),
        (CodeSystem::Icd10Tm, "icd10-tm"),
        (CodeSystem::Titck, "titck"),
        (CodeSystem::Loinc, "loinc"),
        (CodeSystem::Sut, "sut"),
        (CodeSystem::SkrsVp, "skrs-vp"),
        (CodeSystem::AnamnezSym, "anamnez-sym"),
    ];
    let mut reports = Vec::with_capacity(systems.len());
    for (sys, subdir) in systems {
        let path: PathBuf = code_systems_root.join(subdir).join("normalized.csv");
        reports.push(load_csv(conn, sys, &path)?);
    }
    Ok(reports)
}

fn open_reader(path: &Path) -> Result<csv::Reader<std::fs::File>> {
    let f = std::fs::File::open(path)
        .map_err(|e| Error::Invariant(string_leak(&format!("open {}: {e}", path.display()))))?;
    Ok(csv::ReaderBuilder::new().has_headers(true).from_reader(f))
}

fn load_atc(tx: &Transaction, path: &Path) -> Result<LoadReport> {
    let mut reader = open_reader(path)?;
    let mut insert_lookup = tx.prepare(
        "INSERT INTO drug_atc (atc_code, description_en, description_tr) VALUES (?1, ?2, ?3)",
    )?;
    let mut insert_fts =
        tx.prepare("INSERT INTO fts_drug_atc (atc_code, folded_display) VALUES (?1, ?2)")?;
    let mut rows = 0usize;

    for record in reader.records() {
        let row = record?;
        let atc_code = row
            .get(0)
            .ok_or(Error::Invariant("ATC: missing atc_code"))?;
        let description_en = empty_to_none(row.get(1));
        let description_tr = empty_to_none(row.get(2));
        // index 3 = description_tr_source (provenance, dropped)
        insert_lookup.execute(params![atc_code, description_en, description_tr])?;
        let folded = folded_display(&[description_tr.as_deref(), description_en.as_deref()]);
        insert_fts.execute(params![atc_code, folded])?;
        rows += 1;
    }
    Ok(LoadReport {
        system: CodeSystem::Atc,
        rows_inserted: rows,
        dropped_columns: vec!["description_tr_source".into()],
    })
}

fn load_titck(tx: &Transaction, path: &Path) -> Result<LoadReport> {
    let mut reader = open_reader(path)?;
    let mut insert_lookup = tx.prepare(
        "INSERT INTO drug_titck \
         (barcode, titck_product_code, trade_name, manufacturer, atc_code, active_substance_tr, \
          dosage_form, strength_value, strength_unit, strength_text, package_size_text, \
          rx_only, reimbursable, retired_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
    )?;
    let mut insert_fts =
        tx.prepare("INSERT INTO fts_drug_titck (barcode, folded_display) VALUES (?1, ?2)")?;
    let mut rows = 0usize;

    for record in reader.records() {
        let r = record?;
        let barcode = r.get(0).ok_or(Error::Invariant("TITCK: missing barcode"))?;
        let titck_product_code = r.get(1).unwrap_or("");
        let trade_name = r.get(2).unwrap_or("");
        let manufacturer = empty_to_none(r.get(3));
        let atc_raw = r.get(4).unwrap_or("");
        // Only attach atc_code if it actually exists in drug_atc to keep the FK intact.
        let atc_code = if atc_raw.is_empty() {
            None
        } else {
            let exists: bool = tx
                .query_row(
                    "SELECT 1 FROM drug_atc WHERE atc_code = ?1",
                    params![atc_raw],
                    |_| Ok(true),
                )
                .unwrap_or(false);
            exists.then(|| atc_raw.to_owned())
        };
        let active_substance_tr = empty_to_none(r.get(5));
        let dosage_form = empty_to_none(r.get(6));
        let strength_value: Option<f64> = empty_to_none(r.get(7)).and_then(|s| s.parse().ok());
        let strength_unit = empty_to_none(r.get(8));
        let strength_text = empty_to_none(r.get(9));
        let package_size_text = empty_to_none(r.get(10));
        let rx_only = parse_optional_bool(r.get(11));
        let reimbursable = parse_optional_bool(r.get(12));
        let retired_at = empty_to_none(r.get(13));

        insert_lookup.execute(params![
            barcode,
            titck_product_code,
            trade_name,
            manufacturer,
            atc_code,
            active_substance_tr,
            dosage_form,
            strength_value,
            strength_unit,
            strength_text,
            package_size_text,
            rx_only,
            reimbursable,
            retired_at,
        ])?;
        let folded = folded_display(&[
            Some(trade_name),
            active_substance_tr.as_deref(),
            manufacturer.as_deref(),
        ]);
        insert_fts.execute(params![barcode, folded])?;
        rows += 1;
    }
    Ok(LoadReport {
        system: CodeSystem::Titck,
        rows_inserted: rows,
        dropped_columns: vec![],
    })
}

fn load_icd10tm(tx: &Transaction, path: &Path) -> Result<LoadReport> {
    let mut reader = open_reader(path)?;
    let mut insert_lookup = tx.prepare(
        "INSERT INTO icd10_tm (code, description_tr, description_en, parent_code, is_billable) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    let mut insert_fts =
        tx.prepare("INSERT INTO fts_icd10_tm (code, folded_display) VALUES (?1, ?2)")?;
    let mut rows = 0usize;

    // ICD-10-TM is hierarchical (parent_code refers to itself). Two-pass: first
    // load all codes with parent_code NULL, then update parent_code from the CSV.
    // Simpler: defer FK by collecting rows and inserting in topological order; or
    // turn off foreign_keys for this transaction. The cleanest approach is to
    // insert in two passes — load codes first, then fix up parent_code references.
    let mut deferred_parents: Vec<(String, String)> = Vec::new();

    for record in reader.records() {
        let r = record?;
        let code = r.get(0).ok_or(Error::Invariant("ICD10TM: missing code"))?;
        let description_tr = empty_to_none(r.get(1));
        let description_en = empty_to_none(r.get(2));
        let parent_code_raw = empty_to_none(r.get(3));
        let is_billable = parse_optional_bool(r.get(4)).unwrap_or(0);

        insert_lookup.execute(params![
            code,
            description_tr,
            description_en,
            Option::<String>::None,
            is_billable
        ])?;
        if let Some(parent) = parent_code_raw {
            deferred_parents.push((code.to_owned(), parent));
        }
        let folded = folded_display(&[description_tr.as_deref(), description_en.as_deref()]);
        insert_fts.execute(params![code, folded])?;
        rows += 1;
    }

    // Wire up parent_code now that all rows exist.
    for (code, parent) in deferred_parents {
        tx.execute(
            "UPDATE icd10_tm SET parent_code = ?1 WHERE code = ?2 AND EXISTS (SELECT 1 FROM icd10_tm WHERE code = ?1)",
            params![parent, code],
        )?;
    }

    Ok(LoadReport {
        system: CodeSystem::Icd10Tm,
        rows_inserted: rows,
        dropped_columns: vec![],
    })
}

fn load_loinc(tx: &Transaction, path: &Path) -> Result<LoadReport> {
    let mut reader = open_reader(path)?;
    let mut insert_lookup = tx.prepare(
        "INSERT INTO loinc (code, long_name_en, long_name_tr, component, unit_default, scale_typ) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    let mut insert_fts =
        tx.prepare("INSERT INTO fts_loinc (code, folded_display) VALUES (?1, ?2)")?;
    let mut rows = 0usize;

    for record in reader.records() {
        let r = record?;
        let code = r.get(0).ok_or(Error::Invariant("LOINC: missing code"))?;
        let long_name_en = empty_to_none(r.get(1));
        let long_name_tr = empty_to_none(r.get(2));
        // index 3 = long_name_tr_source (provenance, dropped)
        let component = empty_to_none(r.get(4));
        let unit_default = empty_to_none(r.get(5));
        let scale_typ = empty_to_none(r.get(6));
        insert_lookup.execute(params![
            code,
            long_name_en,
            long_name_tr,
            component,
            unit_default,
            scale_typ
        ])?;
        let folded = folded_display(&[long_name_tr.as_deref(), long_name_en.as_deref()]);
        insert_fts.execute(params![code, folded])?;
        rows += 1;
    }
    Ok(LoadReport {
        system: CodeSystem::Loinc,
        rows_inserted: rows,
        dropped_columns: vec!["long_name_tr_source".into()],
    })
}

fn load_sut(tx: &Transaction, path: &Path) -> Result<LoadReport> {
    let mut reader = open_reader(path)?;
    let mut insert_lookup = tx.prepare(
        "INSERT INTO procedure_sut (sut_code, description_tr, category, retired_at) VALUES (?1, ?2, ?3, ?4)",
    )?;
    let mut insert_fts =
        tx.prepare("INSERT INTO fts_procedure_sut (sut_code, folded_display) VALUES (?1, ?2)")?;
    let mut rows = 0usize;
    for record in reader.records() {
        let r = record?;
        let code = r.get(0).ok_or(Error::Invariant("SUT: missing code"))?;
        let description_tr = r.get(1).unwrap_or("").to_owned();
        let category = empty_to_none(r.get(2));
        let retired_at = empty_to_none(r.get(3));
        insert_lookup.execute(params![code, description_tr, category, retired_at])?;
        let folded = casefold(&description_tr);
        insert_fts.execute(params![code, folded])?;
        rows += 1;
    }
    Ok(LoadReport {
        system: CodeSystem::Sut,
        rows_inserted: rows,
        dropped_columns: vec![],
    })
}

fn load_skrs_vp(tx: &Transaction, path: &Path) -> Result<LoadReport> {
    let mut reader = open_reader(path)?;
    let mut insert_lookup = tx.prepare(
        "INSERT INTO visit_purpose_skrs (code, description_tr, description_en, retired_at) VALUES (?1, ?2, ?3, ?4)",
    )?;
    let mut insert_fts =
        tx.prepare("INSERT INTO fts_visit_purpose_skrs (code, folded_display) VALUES (?1, ?2)")?;
    let mut rows = 0usize;
    for record in reader.records() {
        let r = record?;
        let code = r.get(0).ok_or(Error::Invariant("SKRS-VP: missing code"))?;
        let description_tr = r.get(1).unwrap_or("").to_owned();
        let description_en = empty_to_none(r.get(2));
        let retired_at = empty_to_none(r.get(3));
        insert_lookup.execute(params![code, description_tr, description_en, retired_at])?;
        let folded = folded_display(&[Some(&description_tr), description_en.as_deref()]);
        insert_fts.execute(params![code, folded])?;
        rows += 1;
    }
    Ok(LoadReport {
        system: CodeSystem::SkrsVp,
        rows_inserted: rows,
        dropped_columns: vec![],
    })
}

fn load_anamnez_sym(tx: &Transaction, path: &Path) -> Result<LoadReport> {
    let mut reader = open_reader(path)?;
    let mut insert_lookup = tx.prepare(
        "INSERT INTO symptom_anamnez (code, display_tr, display_en, icd10_suggestion, body_region, retired_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    let mut insert_fts =
        tx.prepare("INSERT INTO fts_symptom_anamnez (code, folded_display) VALUES (?1, ?2)")?;
    let mut rows = 0usize;
    for record in reader.records() {
        let r = record?;
        let code = r
            .get(0)
            .ok_or(Error::Invariant("ANAMNEZ-SYM: missing code"))?;
        let display_tr = r.get(1).unwrap_or("").to_owned();
        let display_en = empty_to_none(r.get(2));
        let icd10_raw = empty_to_none(r.get(3));
        // Only attach icd10_suggestion if it actually exists in icd10_tm to keep FK.
        let icd10_suggestion = match icd10_raw {
            None => None,
            Some(s) => {
                let exists: bool = tx
                    .query_row("SELECT 1 FROM icd10_tm WHERE code = ?1", params![s], |_| {
                        Ok(true)
                    })
                    .unwrap_or(false);
                exists.then_some(s)
            }
        };
        let body_region = empty_to_none(r.get(4));
        let retired_at = empty_to_none(r.get(5));
        insert_lookup.execute(params![
            code,
            display_tr,
            display_en,
            icd10_suggestion,
            body_region,
            retired_at
        ])?;
        let folded = folded_display(&[Some(&display_tr), display_en.as_deref()]);
        insert_fts.execute(params![code, folded])?;
        rows += 1;
    }
    Ok(LoadReport {
        system: CodeSystem::AnamnezSym,
        rows_inserted: rows,
        dropped_columns: vec![],
    })
}

fn empty_to_none(s: Option<&str>) -> Option<String> {
    match s {
        None => None,
        Some("") => None,
        Some(other) => Some(other.to_owned()),
    }
}

fn parse_optional_bool(s: Option<&str>) -> Option<i64> {
    match s {
        None | Some("") => None,
        Some("true") | Some("True") | Some("TRUE") | Some("1") => Some(1),
        Some("false") | Some("False") | Some("FALSE") | Some("0") => Some(0),
        _ => None,
    }
}

fn folded_display(parts: &[Option<&str>]) -> String {
    let mut s = String::new();
    for (i, p) in parts.iter().enumerate() {
        if let Some(text) = p {
            if i > 0 {
                s.push(' ');
            }
            s.push_str(text);
        }
    }
    casefold(&s)
}

fn string_leak(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}
