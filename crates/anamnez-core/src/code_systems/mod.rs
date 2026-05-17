//! README §Storage → Code systems — lookup tables, autocomplete (FTS5), bundle distribution.

pub mod bundle;
pub mod fts;
pub mod kinds;
pub mod loader;
pub mod pubkey;
pub mod turkish;

pub use kinds::CodeSystem;

use crate::code_systems::turkish::casefold;
use crate::db::Database;
use crate::error::{Error, Result};
use rusqlite::{params, Connection};

#[derive(Debug, Clone)]
pub struct LookupRow {
    pub code_system: CodeSystem,
    pub code: String,
    pub display_tr: Option<String>,
    pub display_en: Option<String>,
    pub is_retired: bool,
}

/// Look up a `(code_system, code)` pair in the relevant lookup table.
/// Returns `Err(Error::CodeSystemMismatch)` if the pair is not found.
pub fn lookup(db: &Database, system: CodeSystem, code: &str) -> Result<LookupRow> {
    db.with_reader(|conn| lookup_in_conn(conn, system, code))
}

/// Same as [`lookup`] but on a borrowed connection (for clinical writes that already
/// hold the writer lock and want to validate `(code_system, code)` inline).
pub fn lookup_in_conn(conn: &Connection, system: CodeSystem, code: &str) -> Result<LookupRow> {
    let result = match system {
        CodeSystem::Atc => conn
            .query_row(
                "SELECT description_tr, description_en FROM drug_atc WHERE atc_code = ?1",
                params![code],
                |r| {
                    Ok(LookupRow {
                        code_system: system,
                        code: code.to_owned(),
                        display_tr: r.get(0)?,
                        display_en: r.get(1)?,
                        is_retired: false,
                    })
                },
            )
            .ok(),
        CodeSystem::Titck => conn
            .query_row(
                "SELECT trade_name, active_substance_tr, retired_at FROM drug_titck WHERE barcode = ?1",
                params![code],
                |r| {
                    let trade: String = r.get(0)?;
                    let active: Option<String> = r.get(1)?;
                    let retired: Option<String> = r.get(2)?;
                    Ok(LookupRow {
                        code_system: system,
                        code: code.to_owned(),
                        display_tr: Some(trade),
                        display_en: active,
                        is_retired: retired.is_some(),
                    })
                },
            )
            .ok(),
        CodeSystem::Icd10Tm => conn
            .query_row(
                "SELECT description_tr, description_en FROM icd10_tm WHERE code = ?1",
                params![code],
                |r| {
                    Ok(LookupRow {
                        code_system: system,
                        code: code.to_owned(),
                        display_tr: r.get(0)?,
                        display_en: r.get(1)?,
                        is_retired: false,
                    })
                },
            )
            .ok(),
        CodeSystem::Loinc => conn
            .query_row(
                "SELECT long_name_tr, long_name_en FROM loinc WHERE code = ?1",
                params![code],
                |r| {
                    Ok(LookupRow {
                        code_system: system,
                        code: code.to_owned(),
                        display_tr: r.get(0)?,
                        display_en: r.get(1)?,
                        is_retired: false,
                    })
                },
            )
            .ok(),
        CodeSystem::Sut => conn
            .query_row(
                "SELECT description_tr, retired_at FROM procedure_sut WHERE sut_code = ?1",
                params![code],
                |r| {
                    let tr: String = r.get(0)?;
                    let retired: Option<String> = r.get(1)?;
                    Ok(LookupRow {
                        code_system: system,
                        code: code.to_owned(),
                        display_tr: Some(tr),
                        display_en: None,
                        is_retired: retired.is_some(),
                    })
                },
            )
            .ok(),
        CodeSystem::SkrsVp => conn
            .query_row(
                "SELECT description_tr, description_en, retired_at FROM visit_purpose_skrs WHERE code = ?1",
                params![code],
                |r| {
                    let tr: String = r.get(0)?;
                    let en: Option<String> = r.get(1)?;
                    let retired: Option<String> = r.get(2)?;
                    Ok(LookupRow {
                        code_system: system,
                        code: code.to_owned(),
                        display_tr: Some(tr),
                        display_en: en,
                        is_retired: retired.is_some(),
                    })
                },
            )
            .ok(),
        CodeSystem::AnamnezSym => conn
            .query_row(
                "SELECT display_tr, display_en, retired_at FROM symptom_anamnez WHERE code = ?1",
                params![code],
                |r| {
                    let tr: String = r.get(0)?;
                    let en: Option<String> = r.get(1)?;
                    let retired: Option<String> = r.get(2)?;
                    Ok(LookupRow {
                        code_system: system,
                        code: code.to_owned(),
                        display_tr: Some(tr),
                        display_en: en,
                        is_retired: retired.is_some(),
                    })
                },
            )
            .ok(),
    };
    result.ok_or_else(|| Error::CodeSystemMismatch {
        code_system: system.as_str().to_owned(),
        code: code.to_owned(),
    })
}

/// Autocomplete query over the relevant FTS5 vtable.
pub fn autocomplete(
    db: &Database,
    system: CodeSystem,
    query: &str,
    limit: usize,
) -> Result<Vec<LookupRow>> {
    let folded = casefold(query);
    db.with_reader(|conn| {
        let codes = fts::search_codes(conn, system, &folded, limit)?;
        let mut out = Vec::with_capacity(codes.len());
        for code in codes {
            if let Ok(row) = lookup_in_conn(conn, system, &code) {
                out.push(row);
            }
        }
        Ok(out)
    })
}

/// Repo-root code-systems directory, computed at compile time from the workspace layout.
#[must_use]
pub fn repo_code_systems_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate has parent")
        .parent()
        .expect("workspace has parent")
        .join("code-systems")
}
