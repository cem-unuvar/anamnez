//! `CodeSystem` — serde tags identical to `anamnez_core::code_systems::kinds::CodeSystem`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CodeSystem {
    #[serde(rename = "ATC")]
    Atc,
    #[serde(rename = "TITCK")]
    Titck,
    #[serde(rename = "ICD10TM")]
    Icd10Tm,
    #[serde(rename = "LOINC")]
    Loinc,
    #[serde(rename = "SUT")]
    Sut,
    #[serde(rename = "SKRS-VP")]
    SkrsVp,
    #[serde(rename = "ANAMNEZ-SYM")]
    AnamnezSym,
}

/// One hit from `GET /v1/codesystems/search`. FTS5-ranked, most relevant first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub code_system: CodeSystem,
    pub code: String,
    pub display_tr: Option<String>,
    pub display_en: Option<String>,
    pub is_retired: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub hits: Vec<SearchHit>,
}
