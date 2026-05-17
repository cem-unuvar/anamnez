//! FTS5 wrappers around the code-systems lookup tables. Inputs are pre-folded through
//! [`super::turkish::casefold`] before insert and before query.

use super::CodeSystem;
use crate::error::Result;
use rusqlite::{params, Connection};

const fn vtable_for(system: CodeSystem) -> &'static str {
    match system {
        CodeSystem::Atc => "fts_drug_atc",
        CodeSystem::Titck => "fts_drug_titck",
        CodeSystem::Icd10Tm => "fts_icd10_tm",
        CodeSystem::Loinc => "fts_loinc",
        CodeSystem::Sut => "fts_procedure_sut",
        CodeSystem::SkrsVp => "fts_visit_purpose_skrs",
        CodeSystem::AnamnezSym => "fts_symptom_anamnez",
    }
}

const fn code_column_for(system: CodeSystem) -> &'static str {
    match system {
        CodeSystem::Atc => "atc_code",
        CodeSystem::Titck => "barcode",
        CodeSystem::Sut => "sut_code",
        _ => "code",
    }
}

/// Search the FTS5 vtable for `system` with a query that the caller has already folded.
/// Returns the matching codes in rank order (most relevant first).
pub fn search_codes(
    conn: &Connection,
    system: CodeSystem,
    folded_query: &str,
    limit: usize,
) -> Result<Vec<String>> {
    if folded_query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let vtable = vtable_for(system);
    let code_col = code_column_for(system);
    let sql = format!(
        "SELECT {code_col} FROM {vtable} WHERE folded_display MATCH ?1 ORDER BY rank LIMIT ?2"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![fts_match_expr(folded_query), limit as i64], |r| {
        r.get::<_, String>(0)
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// Build a tolerant FTS5 MATCH expression: each whitespace-delimited token becomes
/// a prefix match (`token*`), and tokens are AND-combined.
fn fts_match_expr(folded_query: &str) -> String {
    let tokens: Vec<String> = folded_query
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"*", t.replace('"', "\"\"")))
        .collect();
    tokens.join(" ")
}
