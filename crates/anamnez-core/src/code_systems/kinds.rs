//! `CodeSystem` enum + observation- / encounter-scoped accessors.

use crate::error::Error;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CodeSystem {
    /// WHO Anatomical Therapeutic Chemical — drug active substance.
    #[serde(rename = "ATC")]
    Atc,
    /// TİTCK product code (GTIN-13 barcode) — Turkish-market drug products.
    #[serde(rename = "TITCK")]
    Titck,
    /// ICD-10-TM (Turkish modification) — diagnoses and conditions.
    #[serde(rename = "ICD10TM")]
    Icd10Tm,
    /// LOINC — labs and measurements.
    #[serde(rename = "LOINC")]
    Loinc,
    /// SUT (Sağlık Uygulama Tebliği) — procedures.
    #[serde(rename = "SUT")]
    Sut,
    /// SKRS Başvuru Nedeni — procedural visit purpose. Encounter-only.
    #[serde(rename = "SKRS-VP")]
    SkrsVp,
    /// Custom curated list of symptoms and clinical findings.
    #[serde(rename = "ANAMNEZ-SYM")]
    AnamnezSym,
}

impl CodeSystem {
    /// Codes valid on `observation.code_system` — the observation-scoped subset.
    /// `SKRS-VP` is encounter-only and is rejected here.
    #[must_use]
    pub fn is_observation_scope(self) -> bool {
        !matches!(self, Self::SkrsVp)
    }

    /// Codes valid on `encounter.reason_code_system` — `ICD10TM`, `ANAMNEZ-SYM`, `SKRS-VP`.
    #[must_use]
    pub fn is_encounter_reason_scope(self) -> bool {
        matches!(self, Self::Icd10Tm | Self::AnamnezSym | Self::SkrsVp)
    }

    /// Codes valid on `medication.code_system` — `ATC`, `TITCK`.
    #[must_use]
    pub fn is_medication_scope(self) -> bool {
        matches!(self, Self::Atc | Self::Titck)
    }

    /// Codes valid on `allergy.code_system` when set — `ATC` only at MVP.
    #[must_use]
    pub fn is_allergy_scope(self) -> bool {
        matches!(self, Self::Atc)
    }

    /// Wire/serde tag — the string used in `audit_log`, JSON envelopes, and migrations.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Atc => "ATC",
            Self::Titck => "TITCK",
            Self::Icd10Tm => "ICD10TM",
            Self::Loinc => "LOINC",
            Self::Sut => "SUT",
            Self::SkrsVp => "SKRS-VP",
            Self::AnamnezSym => "ANAMNEZ-SYM",
        }
    }

    pub fn parse_tag(s: &str) -> Result<Self, Error> {
        match s {
            "ATC" => Ok(Self::Atc),
            "TITCK" => Ok(Self::Titck),
            "ICD10TM" => Ok(Self::Icd10Tm),
            "LOINC" => Ok(Self::Loinc),
            "SUT" => Ok(Self::Sut),
            "SKRS-VP" => Ok(Self::SkrsVp),
            "ANAMNEZ-SYM" => Ok(Self::AnamnezSym),
            _ => Err(Error::Invariant("unknown code system tag")),
        }
    }
}
