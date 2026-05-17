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
