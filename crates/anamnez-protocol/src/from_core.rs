//! `From` conversions from `anamnez_core` types to wire DTOs. Daemon-only;
//! `anamnez_core` is not compiled for wasm32.

use anamnez_core as core;

use crate::access as p_acc;
use crate::allergy as p_all;
use crate::audit as p_aud;
use crate::auth as p_auth;
use crate::codesystem as p_cs;
use crate::consent as p_con;
use crate::encounter as p_enc;
use crate::environment as p_env;
use crate::error::ErrorEnvelope;
use crate::events as p_ev;
use crate::ids as p_ids;
use crate::medication as p_med;
use crate::observation as p_obs;
use crate::patient as p_pat;
use crate::source_document as p_doc;
use crate::stepup as p_step;
use crate::versioned::Versioned as PVersioned;

// ─── Environment ──────────────────────────────────────────────────────────────

impl From<core::env::Environment> for p_env::Environment {
    fn from(c: core::env::Environment) -> Self {
        match c {
            core::env::Environment::Production => Self::Production,
            core::env::Environment::Test => Self::Test,
        }
    }
}
impl From<p_env::Environment> for core::env::Environment {
    fn from(p: p_env::Environment) -> Self {
        match p {
            p_env::Environment::Production => Self::Production,
            p_env::Environment::Test => Self::Test,
        }
    }
}

// ─── IDs ──────────────────────────────────────────────────────────────────────

impl From<core::ids::UserId> for p_ids::UserId {
    fn from(c: core::ids::UserId) -> Self {
        Self(c.as_uuid())
    }
}
impl From<p_ids::UserId> for core::ids::UserId {
    fn from(p: p_ids::UserId) -> Self {
        Self(p.0)
    }
}
impl From<core::ids::PatientId> for p_ids::PatientId {
    fn from(c: core::ids::PatientId) -> Self {
        Self(c.as_uuid())
    }
}
impl From<p_ids::PatientId> for core::ids::PatientId {
    fn from(p: p_ids::PatientId) -> Self {
        Self(p.0)
    }
}
impl From<core::ids::EncounterId> for p_ids::EncounterId {
    fn from(c: core::ids::EncounterId) -> Self {
        Self(c.as_uuid())
    }
}
impl From<p_ids::EncounterId> for core::ids::EncounterId {
    fn from(p: p_ids::EncounterId) -> Self {
        Self(p.0)
    }
}
impl From<core::ids::ObservationId> for p_ids::ObservationId {
    fn from(c: core::ids::ObservationId) -> Self {
        Self(c.as_uuid())
    }
}
impl From<p_ids::ObservationId> for core::ids::ObservationId {
    fn from(p: p_ids::ObservationId) -> Self {
        Self(p.0)
    }
}
impl From<core::ids::AllergyId> for p_ids::AllergyId {
    fn from(c: core::ids::AllergyId) -> Self {
        Self(c.as_uuid())
    }
}
impl From<p_ids::AllergyId> for core::ids::AllergyId {
    fn from(p: p_ids::AllergyId) -> Self {
        Self(p.0)
    }
}
impl From<core::ids::MedicationId> for p_ids::MedicationId {
    fn from(c: core::ids::MedicationId) -> Self {
        Self(c.as_uuid())
    }
}
impl From<p_ids::MedicationId> for core::ids::MedicationId {
    fn from(p: p_ids::MedicationId) -> Self {
        Self(p.0)
    }
}
impl From<core::ids::SourceDocumentId> for p_ids::SourceDocumentId {
    fn from(c: core::ids::SourceDocumentId) -> Self {
        Self(c.as_uuid())
    }
}
impl From<p_ids::SourceDocumentId> for core::ids::SourceDocumentId {
    fn from(p: p_ids::SourceDocumentId) -> Self {
        Self(p.0)
    }
}
impl From<core::ids::PatientAnalysisId> for p_ids::PatientAnalysisId {
    fn from(c: core::ids::PatientAnalysisId) -> Self {
        Self(c.as_uuid())
    }
}
impl From<core::ids::PatientConsentId> for p_ids::PatientConsentId {
    fn from(c: core::ids::PatientConsentId) -> Self {
        Self(c.as_uuid())
    }
}
impl From<p_ids::PatientConsentId> for core::ids::PatientConsentId {
    fn from(p: p_ids::PatientConsentId) -> Self {
        Self(p.0)
    }
}
impl From<core::ids::WorkstationId> for p_ids::WorkstationId {
    fn from(c: core::ids::WorkstationId) -> Self {
        Self(c.as_uuid())
    }
}
impl From<core::ids::AuthSessionId> for p_ids::AuthSessionId {
    fn from(c: core::ids::AuthSessionId) -> Self {
        Self(c.as_uuid())
    }
}

// ─── Codesystem ───────────────────────────────────────────────────────────────

impl From<core::code_systems::CodeSystem> for p_cs::CodeSystem {
    fn from(c: core::code_systems::CodeSystem) -> Self {
        use core::code_systems::CodeSystem as C;
        match c {
            C::Atc => Self::Atc,
            C::Titck => Self::Titck,
            C::Icd10Tm => Self::Icd10Tm,
            C::Loinc => Self::Loinc,
            C::Sut => Self::Sut,
            C::SkrsVp => Self::SkrsVp,
            C::AnamnezSym => Self::AnamnezSym,
        }
    }
}
impl From<p_cs::CodeSystem> for core::code_systems::CodeSystem {
    fn from(p: p_cs::CodeSystem) -> Self {
        use p_cs::CodeSystem as P;
        match p {
            P::Atc => Self::Atc,
            P::Titck => Self::Titck,
            P::Icd10Tm => Self::Icd10Tm,
            P::Loinc => Self::Loinc,
            P::Sut => Self::Sut,
            P::SkrsVp => Self::SkrsVp,
            P::AnamnezSym => Self::AnamnezSym,
        }
    }
}

// ─── Versioned ────────────────────────────────────────────────────────────────

impl<C, P: From<C>> From<core::locking::Versioned<C>> for PVersioned<P> {
    fn from(v: core::locking::Versioned<C>) -> Self {
        Self {
            value: v.value.into(),
            version: v.version,
        }
    }
}

// ─── Patient ──────────────────────────────────────────────────────────────────

impl From<core::patient::SexAssignedAtBirth> for p_pat::SexAssignedAtBirth {
    fn from(c: core::patient::SexAssignedAtBirth) -> Self {
        use core::patient::SexAssignedAtBirth as C;
        match c {
            C::Female => Self::Female,
            C::Male => Self::Male,
            C::Intersex => Self::Intersex,
            C::Unknown => Self::Unknown,
        }
    }
}
impl From<p_pat::SexAssignedAtBirth> for core::patient::SexAssignedAtBirth {
    fn from(p: p_pat::SexAssignedAtBirth) -> Self {
        use p_pat::SexAssignedAtBirth as P;
        match p {
            P::Female => Self::Female,
            P::Male => Self::Male,
            P::Intersex => Self::Intersex,
            P::Unknown => Self::Unknown,
        }
    }
}
impl From<core::patient::Patient> for p_pat::Patient {
    fn from(c: core::patient::Patient) -> Self {
        Self {
            id: c.id.into(),
            mrn: c.mrn,
            given_names: c.given_names,
            family_name: c.family_name,
            preferred_name: c.preferred_name,
            date_of_birth: c.date_of_birth,
            sex_assigned_at_birth: c.sex_assigned_at_birth.into(),
            gender_identity: c.gender_identity,
            email: c.email,
            phone: c.phone,
            address: c.address,
            emergency_contact_name: c.emergency_contact_name,
            emergency_contact_phone: c.emergency_contact_phone,
            emergency_contact_relationship: c.emergency_contact_relationship,
            created_by: c.created_by.into(),
            created_at: c.created_at,
            updated_at: c.updated_at,
            deceased_at: c.deceased_at,
            archived_at: c.archived_at,
            suppressed_at: c.suppressed_at,
            suppression_reason: c.suppression_reason,
            notice_acknowledged_at: c.notice_acknowledged_at,
        }
    }
}
impl From<p_pat::NewPatient> for core::patient::NewPatient {
    fn from(p: p_pat::NewPatient) -> Self {
        Self {
            mrn: p.mrn,
            given_names: p.given_names,
            family_name: p.family_name,
            preferred_name: p.preferred_name,
            date_of_birth: p.date_of_birth,
            sex_assigned_at_birth: p.sex_assigned_at_birth.into(),
            gender_identity: p.gender_identity,
            email: p.email,
            phone: p.phone,
            address: p.address,
            emergency_contact_name: p.emergency_contact_name,
            emergency_contact_phone: p.emergency_contact_phone,
            emergency_contact_relationship: p.emergency_contact_relationship,
        }
    }
}
impl From<p_pat::PatientPatch> for core::patient::PatientPatch {
    fn from(p: p_pat::PatientPatch) -> Self {
        Self {
            mrn: p.mrn,
            preferred_name: p.preferred_name,
            email: p.email,
            phone: p.phone,
            address: p.address,
            deceased_at: p.deceased_at,
            archived_at: p.archived_at,
            notice_acknowledged_at: p.notice_acknowledged_at,
        }
    }
}

// ─── Observation ──────────────────────────────────────────────────────────────

impl From<core::observation::ObservationStatus> for p_obs::ObservationStatus {
    fn from(c: core::observation::ObservationStatus) -> Self {
        use core::observation::ObservationStatus as C;
        match c {
            C::Preliminary => Self::Preliminary,
            C::Final => Self::Final,
            C::Amended => Self::Amended,
        }
    }
}
impl From<p_obs::ObservationStatus> for core::observation::ObservationStatus {
    fn from(p: p_obs::ObservationStatus) -> Self {
        use p_obs::ObservationStatus as P;
        match p {
            P::Preliminary => Self::Preliminary,
            P::Final => Self::Final,
            P::Amended => Self::Amended,
        }
    }
}
impl From<core::observation::ExtractedBy> for p_obs::ExtractedBy {
    fn from(c: core::observation::ExtractedBy) -> Self {
        match c {
            core::observation::ExtractedBy::Manual => Self::Manual,
            core::observation::ExtractedBy::Llm => Self::Llm,
        }
    }
}
impl From<p_obs::ExtractedBy> for core::observation::ExtractedBy {
    fn from(p: p_obs::ExtractedBy) -> Self {
        match p {
            p_obs::ExtractedBy::Manual => Self::Manual,
            p_obs::ExtractedBy::Llm => Self::Llm,
        }
    }
}
impl From<core::observation::ValueQuantity> for p_obs::ValueQuantity {
    fn from(c: core::observation::ValueQuantity) -> Self {
        Self {
            value: c.value,
            unit: c.unit,
        }
    }
}
impl From<p_obs::ValueQuantity> for core::observation::ValueQuantity {
    fn from(p: p_obs::ValueQuantity) -> Self {
        Self {
            value: p.value,
            unit: p.unit,
        }
    }
}
impl From<core::observation::ObservationValue> for p_obs::ObservationValue {
    fn from(c: core::observation::ObservationValue) -> Self {
        use core::observation::ObservationValue as C;
        match c {
            C::Quantity(q) => Self::Quantity(q.into()),
            C::String(s) => Self::String(s),
            C::Codeable { code_system, code } => Self::Codeable {
                code_system: code_system.into(),
                code,
            },
        }
    }
}
impl From<p_obs::ObservationValue> for core::observation::ObservationValue {
    fn from(p: p_obs::ObservationValue) -> Self {
        use p_obs::ObservationValue as P;
        match p {
            P::Quantity(q) => Self::Quantity(q.into()),
            P::String(s) => Self::String(s),
            P::Codeable { code_system, code } => Self::Codeable {
                code_system: code_system.into(),
                code,
            },
        }
    }
}
impl From<core::observation::Observation> for p_obs::Observation {
    fn from(c: core::observation::Observation) -> Self {
        Self {
            id: c.id.into(),
            patient_id: c.patient_id.into(),
            recorded_at: c.recorded_at,
            effective_period_start: c.effective_period_start,
            effective_period_end: c.effective_period_end,
            code: c.code,
            code_system: c.code_system.map(Into::into),
            display_text: c.display_text,
            value: c.value.map(Into::into),
            status: c.status.into(),
            is_problem_list_item: c.is_problem_list_item,
            source_id: c.source_id.map(Into::into),
            encounter_id: c.encounter_id.map(Into::into),
            extracted_by: c.extracted_by.into(),
            model_version: c.model_version,
            confidence: c.confidence,
        }
    }
}
impl From<p_obs::NewObservation> for core::observation::NewObservation {
    fn from(p: p_obs::NewObservation) -> Self {
        Self {
            patient_id: p.patient_id.into(),
            effective_period_start: p.effective_period_start,
            effective_period_end: p.effective_period_end,
            code: p.code,
            code_system: p.code_system.map(Into::into),
            display_text: p.display_text,
            value: p.value.map(Into::into),
            status: p.status.into(),
            is_problem_list_item: p.is_problem_list_item,
            source_id: p.source_id.map(Into::into),
            encounter_id: p.encounter_id.map(Into::into),
            extracted_by: p.extracted_by.into(),
            model_version: p.model_version,
            confidence: p.confidence,
        }
    }
}
impl From<p_obs::ObservationPatch> for core::observation::ObservationPatch {
    fn from(p: p_obs::ObservationPatch) -> Self {
        Self {
            effective_period_end: p.effective_period_end,
            code: p.code,
            code_system: p.code_system.map(|c| c.map(Into::into)),
            display_text: p.display_text,
            value: p.value.map(|v| v.map(Into::into)),
            status: p.status.map(Into::into),
            is_problem_list_item: p.is_problem_list_item,
        }
    }
}

// ─── Encounter ────────────────────────────────────────────────────────────────

impl From<core::encounter::EncounterKind> for p_enc::EncounterKind {
    fn from(c: core::encounter::EncounterKind) -> Self {
        use core::encounter::EncounterKind as C;
        match c {
            C::InPerson => Self::InPerson,
            C::Phone => Self::Phone,
            C::Video => Self::Video,
            C::AsyncDocument => Self::AsyncDocument,
        }
    }
}
impl From<p_enc::EncounterKind> for core::encounter::EncounterKind {
    fn from(p: p_enc::EncounterKind) -> Self {
        use p_enc::EncounterKind as P;
        match p {
            P::InPerson => Self::InPerson,
            P::Phone => Self::Phone,
            P::Video => Self::Video,
            P::AsyncDocument => Self::AsyncDocument,
        }
    }
}
impl From<core::encounter::EncounterStatus> for p_enc::EncounterStatus {
    fn from(c: core::encounter::EncounterStatus) -> Self {
        use core::encounter::EncounterStatus as C;
        match c {
            C::InProgress => Self::InProgress,
            C::Finished => Self::Finished,
            C::Cancelled => Self::Cancelled,
        }
    }
}
impl From<core::encounter::Encounter> for p_enc::Encounter {
    fn from(c: core::encounter::Encounter) -> Self {
        Self {
            id: c.id.into(),
            patient_id: c.patient_id.into(),
            provider_id: c.provider_id.into(),
            kind: c.kind.into(),
            reason_text: c.reason_text,
            reason_code: c.reason_code,
            reason_code_system: c.reason_code_system.map(Into::into),
            started_at: c.started_at,
            ended_at: c.ended_at,
            status: c.status.into(),
            created_at: c.created_at,
        }
    }
}

// ─── Allergy ──────────────────────────────────────────────────────────────────

impl From<core::allergy::AllergySeverity> for p_all::AllergySeverity {
    fn from(c: core::allergy::AllergySeverity) -> Self {
        use core::allergy::AllergySeverity as C;
        match c {
            C::Mild => Self::Mild,
            C::Moderate => Self::Moderate,
            C::Severe => Self::Severe,
            C::LifeThreatening => Self::LifeThreatening,
        }
    }
}
impl From<p_all::AllergySeverity> for core::allergy::AllergySeverity {
    fn from(p: p_all::AllergySeverity) -> Self {
        use p_all::AllergySeverity as P;
        match p {
            P::Mild => Self::Mild,
            P::Moderate => Self::Moderate,
            P::Severe => Self::Severe,
            P::LifeThreatening => Self::LifeThreatening,
        }
    }
}
impl From<core::allergy::AllergyStatus> for p_all::AllergyStatus {
    fn from(c: core::allergy::AllergyStatus) -> Self {
        use core::allergy::AllergyStatus as C;
        match c {
            C::Active => Self::Active,
            C::Inactive => Self::Inactive,
            C::EnteredInError => Self::EnteredInError,
        }
    }
}
impl From<p_all::AllergyStatus> for core::allergy::AllergyStatus {
    fn from(p: p_all::AllergyStatus) -> Self {
        use p_all::AllergyStatus as P;
        match p {
            P::Active => Self::Active,
            P::Inactive => Self::Inactive,
            P::EnteredInError => Self::EnteredInError,
        }
    }
}
impl From<core::allergy::Allergy> for p_all::Allergy {
    fn from(c: core::allergy::Allergy) -> Self {
        Self {
            id: c.id.into(),
            patient_id: c.patient_id.into(),
            code: c.code,
            code_system: c.code_system.map(Into::into),
            display_text: c.display_text,
            severity: c.severity.into(),
            reaction_text: c.reaction_text,
            status: c.status.into(),
            onset_date: c.onset_date,
            recorded_at: c.recorded_at,
            recorded_by: c.recorded_by.into(),
            source_id: c.source_id.map(Into::into),
            encounter_id: c.encounter_id.map(Into::into),
        }
    }
}
impl From<p_all::NewAllergy> for core::allergy::NewAllergy {
    fn from(p: p_all::NewAllergy) -> Self {
        Self {
            patient_id: p.patient_id.into(),
            code: p.code,
            code_system: p.code_system.map(Into::into),
            display_text: p.display_text,
            severity: p.severity.into(),
            reaction_text: p.reaction_text,
            status: p.status.into(),
            onset_date: p.onset_date,
            source_id: p.source_id.map(Into::into),
            encounter_id: p.encounter_id.map(Into::into),
        }
    }
}
impl From<p_all::AllergyPatch> for core::allergy::AllergyPatch {
    fn from(p: p_all::AllergyPatch) -> Self {
        Self {
            severity: p.severity.map(Into::into),
            reaction_text: p.reaction_text,
            status: p.status.map(Into::into),
            onset_date: p.onset_date,
        }
    }
}

// ─── Medication ───────────────────────────────────────────────────────────────

impl From<core::medication::MedicationRoute> for p_med::MedicationRoute {
    fn from(c: core::medication::MedicationRoute) -> Self {
        use core::medication::MedicationRoute as C;
        match c {
            C::Oral => Self::Oral,
            C::Iv => Self::Iv,
            C::Im => Self::Im,
            C::Topical => Self::Topical,
            C::Inhaled => Self::Inhaled,
            C::Other => Self::Other,
        }
    }
}
impl From<p_med::MedicationRoute> for core::medication::MedicationRoute {
    fn from(p: p_med::MedicationRoute) -> Self {
        use p_med::MedicationRoute as P;
        match p {
            P::Oral => Self::Oral,
            P::Iv => Self::Iv,
            P::Im => Self::Im,
            P::Topical => Self::Topical,
            P::Inhaled => Self::Inhaled,
            P::Other => Self::Other,
        }
    }
}
impl From<core::medication::MedicationStatus> for p_med::MedicationStatus {
    fn from(c: core::medication::MedicationStatus) -> Self {
        use core::medication::MedicationStatus as C;
        match c {
            C::Active => Self::Active,
            C::Completed => Self::Completed,
            C::Stopped => Self::Stopped,
            C::EnteredInError => Self::EnteredInError,
        }
    }
}
impl From<p_med::MedicationStatus> for core::medication::MedicationStatus {
    fn from(p: p_med::MedicationStatus) -> Self {
        use p_med::MedicationStatus as P;
        match p {
            P::Active => Self::Active,
            P::Completed => Self::Completed,
            P::Stopped => Self::Stopped,
            P::EnteredInError => Self::EnteredInError,
        }
    }
}
impl From<core::medication::Medication> for p_med::Medication {
    fn from(c: core::medication::Medication) -> Self {
        Self {
            id: c.id.into(),
            patient_id: c.patient_id.into(),
            code: c.code,
            code_system: c.code_system.into(),
            display_text: c.display_text,
            dose_quantity: c.dose_quantity,
            dose_unit: c.dose_unit,
            frequency_text: c.frequency_text,
            route: c.route.into(),
            started_at: c.started_at,
            ended_at: c.ended_at,
            reason_text: c.reason_text,
            status: c.status.into(),
            prescriber_id: c.prescriber_id.map(Into::into),
            recorded_at: c.recorded_at,
            recorded_by: c.recorded_by.into(),
            source_id: c.source_id.map(Into::into),
            encounter_id: c.encounter_id.map(Into::into),
        }
    }
}
impl From<p_med::NewMedication> for core::medication::NewMedication {
    fn from(p: p_med::NewMedication) -> Self {
        Self {
            patient_id: p.patient_id.into(),
            code: p.code,
            code_system: p.code_system.into(),
            display_text: p.display_text,
            dose_quantity: p.dose_quantity,
            dose_unit: p.dose_unit,
            frequency_text: p.frequency_text,
            route: p.route.into(),
            started_at: p.started_at,
            ended_at: p.ended_at,
            reason_text: p.reason_text,
            status: p.status.into(),
            prescriber_id: p.prescriber_id.map(Into::into),
            source_id: p.source_id.map(Into::into),
            encounter_id: p.encounter_id.map(Into::into),
        }
    }
}
impl From<p_med::MedicationPatch> for core::medication::MedicationPatch {
    fn from(p: p_med::MedicationPatch) -> Self {
        Self {
            dose_quantity: p.dose_quantity,
            dose_unit: p.dose_unit,
            frequency_text: p.frequency_text,
            ended_at: p.ended_at,
            status: p.status.map(Into::into),
        }
    }
}

// ─── Source Document ──────────────────────────────────────────────────────────

impl From<core::source_document::SourceDocumentType> for p_doc::SourceDocumentType {
    fn from(c: core::source_document::SourceDocumentType) -> Self {
        use core::source_document::SourceDocumentType as C;
        match c {
            C::Note => Self::Note,
            C::Pdf => Self::Pdf,
            C::Audio => Self::Audio,
            C::Image => Self::Image,
        }
    }
}
impl From<p_doc::SourceDocumentType> for core::source_document::SourceDocumentType {
    fn from(p: p_doc::SourceDocumentType) -> Self {
        use p_doc::SourceDocumentType as P;
        match p {
            P::Note => Self::Note,
            P::Pdf => Self::Pdf,
            P::Audio => Self::Audio,
            P::Image => Self::Image,
        }
    }
}
impl From<core::source_document::SourceDocument> for p_doc::SourceDocument {
    fn from(c: core::source_document::SourceDocument) -> Self {
        Self {
            id: c.id.into(),
            patient_id: c.patient_id.into(),
            kind: c.kind.into(),
            sha256: c.sha256,
            original_filename: c.original_filename,
            mime_type: c.mime_type,
            transcription: c.transcription,
            ocr_text: c.ocr_text,
            encounter_id: c.encounter_id.map(Into::into),
            uploaded_at: c.uploaded_at,
            context_provided_by_user: c.context_provided_by_user,
            recorded_by: c.recorded_by.into(),
        }
    }
}
impl From<p_doc::NewSourceDocument> for core::source_document::NewSourceDocument {
    fn from(p: p_doc::NewSourceDocument) -> Self {
        Self {
            patient_id: p.patient_id.into(),
            kind: p.kind.into(),
            sha256: p.sha256,
            original_filename: p.original_filename,
            mime_type: p.mime_type,
            transcription: p.transcription,
            ocr_text: p.ocr_text,
            encounter_id: p.encounter_id.map(Into::into),
            context_provided_by_user: p.context_provided_by_user,
        }
    }
}

// ─── Consent ──────────────────────────────────────────────────────────────────

impl From<core::consent::ConsentPurpose> for p_con::ConsentPurpose {
    fn from(c: core::consent::ConsentPurpose) -> Self {
        use core::consent::ConsentPurpose as C;
        match c {
            C::LawyerTransfer => Self::LawyerTransfer,
            C::ResearchNonAnonymized => Self::ResearchNonAnonymized,
            C::OtherClinicReferral => Self::OtherClinicReferral,
        }
    }
}
impl From<p_con::ConsentPurpose> for core::consent::ConsentPurpose {
    fn from(p: p_con::ConsentPurpose) -> Self {
        use p_con::ConsentPurpose as P;
        match p {
            P::LawyerTransfer => Self::LawyerTransfer,
            P::ResearchNonAnonymized => Self::ResearchNonAnonymized,
            P::OtherClinicReferral => Self::OtherClinicReferral,
        }
    }
}
impl From<core::consent::PatientConsent> for p_con::PatientConsent {
    fn from(c: core::consent::PatientConsent) -> Self {
        Self {
            id: c.id.into(),
            patient_id: c.patient_id.into(),
            purpose: c.purpose.into(),
            granted_at: c.granted_at,
            granted_by: c.granted_by.into(),
            evidence_source_id: c.evidence_source_id.map(Into::into),
            revoked_at: c.revoked_at,
            notes: c.notes,
        }
    }
}

// ─── Access ───────────────────────────────────────────────────────────────────

impl From<core::patient_access::AccessLevel> for p_acc::AccessLevel {
    fn from(c: core::patient_access::AccessLevel) -> Self {
        use core::patient_access::AccessLevel as C;
        match c {
            C::Owner => Self::Owner,
            C::Collaborator => Self::Collaborator,
            C::ReadOnly => Self::ReadOnly,
        }
    }
}
impl From<p_acc::AccessLevel> for core::patient_access::AccessLevel {
    fn from(p: p_acc::AccessLevel) -> Self {
        use p_acc::AccessLevel as P;
        match p {
            P::Owner => Self::Owner,
            P::Collaborator => Self::Collaborator,
            P::ReadOnly => Self::ReadOnly,
        }
    }
}
impl From<core::patient_access::PatientAccess> for p_acc::PatientAccess {
    fn from(c: core::patient_access::PatientAccess) -> Self {
        Self {
            patient_id: c.patient_id.into(),
            user_id: c.user_id.into(),
            level: c.level.into(),
        }
    }
}

// ─── Auth ─────────────────────────────────────────────────────────────────────

impl From<core::auth::UserRole> for p_aud::UserRole {
    fn from(c: core::auth::UserRole) -> Self {
        match c {
            core::auth::UserRole::Admin => Self::Admin,
            core::auth::UserRole::Provider => Self::Provider,
        }
    }
}
impl From<p_aud::UserRole> for core::auth::UserRole {
    fn from(p: p_aud::UserRole) -> Self {
        match p {
            p_aud::UserRole::Admin => Self::Admin,
            p_aud::UserRole::Provider => Self::Provider,
        }
    }
}
impl From<core::auth::User> for p_auth::User {
    fn from(c: core::auth::User) -> Self {
        Self {
            id: c.id.into(),
            email: c.email,
            display_name: c.display_name,
            role: c.role.into(),
            created_at: c.created_at,
            disabled_at: c.disabled_at,
        }
    }
}

// ─── Step-up ──────────────────────────────────────────────────────────────────

impl From<core::auth::stepup::StepUpAction> for p_step::StepUpAction {
    fn from(c: core::auth::stepup::StepUpAction) -> Self {
        use core::auth::stepup::StepUpAction as C;
        match c {
            C::UserCreate => Self::UserCreate,
            C::UserModify => Self::UserModify,
            C::PatientAccessGrantToNewUser => Self::PatientAccessGrantToNewUser,
            C::UserDisable => Self::UserDisable,
            C::WorkstationRevoke => Self::WorkstationRevoke,
            C::PatientDossierExport => Self::PatientDossierExport,
            C::LargeQueryDownload => Self::LargeQueryDownload,
            C::RetentionPolicyChange => Self::RetentionPolicyChange,
            C::WorkstationEnrollmentString => Self::WorkstationEnrollmentString,
        }
    }
}

// ─── Analysis ─────────────────────────────────────────────────────────────────

impl From<core::analysis::PatientAnalysis> for crate::analysis::PatientAnalysis {
    fn from(c: core::analysis::PatientAnalysis) -> Self {
        Self {
            id: c.id.into(),
            patient_id: c.patient_id.into(),
            generated_at: c.generated_at,
            generated_by: c.generated_by.into(),
            model_id: c.model_id,
            prompt_version: c.prompt_version,
            report_markdown: c.report_markdown,
            scope_observation_ids: c
                .scope_observation_ids
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

// ─── Audit Action ─────────────────────────────────────────────────────────────

impl From<core::audit::Action> for p_aud::Action {
    fn from(c: core::audit::Action) -> Self {
        use core::audit::Action as C;
        match c {
            C::PatientView => Self::PatientView,
            C::PatientUpdate => Self::PatientUpdate,
            C::PatientExport => Self::PatientExport,
            C::PatientOwnershipTransfer => Self::PatientOwnershipTransfer,
            C::ObservationCreate => Self::ObservationCreate,
            C::ObservationAmend => Self::ObservationAmend,
            C::AllergyCreate => Self::AllergyCreate,
            C::AllergyAmend => Self::AllergyAmend,
            C::MedicationCreate => Self::MedicationCreate,
            C::MedicationAmend => Self::MedicationAmend,
            C::SourceDocumentCreate => Self::SourceDocumentCreate,
            C::ConsentRecord => Self::ConsentRecord,
            C::ConsentRevoke => Self::ConsentRevoke,
            C::EncounterStart => Self::EncounterStart,
            C::EncounterFinish => Self::EncounterFinish,
            C::EncounterCancel => Self::EncounterCancel,
            C::UserLogin => Self::UserLogin,
            C::UserCreate => Self::UserCreate,
            C::UserModify => Self::UserModify,
            C::UserDisable => Self::UserDisable,
            C::WorkstationEnroll => Self::WorkstationEnroll,
            C::WorkstationRevoke => Self::WorkstationRevoke,
            C::PatientAccessGrant => Self::PatientAccessGrant,
            C::PatientAccessRevoke => Self::PatientAccessRevoke,
            C::AnalysisGenerate => Self::AnalysisGenerate,
            C::CodesystemsUpdate => Self::CodesystemsUpdate,
            C::AccessReviewCompleted => Self::AccessReviewCompleted,
            C::RetentionSweep => Self::RetentionSweep,
        }
    }
}

// ─── Error ────────────────────────────────────────────────────────────────────

impl From<&core::Error> for ErrorEnvelope {
    fn from(e: &core::Error) -> Self {
        use core::Error as C;
        match e {
            C::Conflict {
                current_version,
                new_state_json,
            } => Self::Conflict {
                current_version: *current_version,
                new_state_json: new_state_json.clone(),
            },
            C::NotFound => Self::NotFound,
            C::Forbidden => Self::Forbidden,
            C::BadCredentials => Self::BadCredentials,
            C::Revoked => Self::Revoked,
            C::SessionExpired => Self::SessionExpired,
            C::StepUpRequired { action } => Self::StepUpRequired {
                action: (*action).to_string(),
            },
            C::OutdatedClient { min, got } => Self::OutdatedClient {
                min: min.clone(),
                got: got.clone(),
            },
            C::CodeSystemNotAllowed {
                code_system,
                context,
            } => Self::CodeSystemNotAllowed {
                code_system: code_system.clone(),
                context: (*context).to_string(),
            },
            C::CodeSystemMismatch { code_system, code } => Self::CodeSystemMismatch {
                code_system: code_system.clone(),
                code: code.clone(),
            },
            C::RetiredCode { code } => Self::RetiredCode { code: code.clone() },
            C::InvalidStateTransition { from, to } => Self::InvalidStateTransition {
                from: (*from).to_string(),
                to: (*to).to_string(),
            },
            C::TestPrefixRequired => Self::TestPrefixRequired,
            C::SoleOwnerOfPatient { patient_id } => Self::SoleOwnerOfPatient {
                patient_id: patient_id.clone(),
            },
            // Internal / boot-only / wrapped IO: collapse to opaque body. Daemon logs the
            // real error server-side; the wire envelope never carries internal detail.
            C::AuditTamper { .. }
            | C::Invariant(_)
            | C::Db(_)
            | C::Io(_)
            | C::Serde(_)
            | C::Csv(_)
            | C::EnvironmentMarkerMismatch { .. }
            | C::SchemaVersionMismatch { .. }
            | C::InvalidBundleSignature => Self::Internal {
                detail: "internal error".into(),
            },
        }
    }
}

// ─── Server events ────────────────────────────────────────────────────────────
// (Server-side constructs events directly from internal state — no inverse needed.)

impl p_ev::ServerEvent {
    /// Construct from a `core` action shape — convenience for handler emit sites.
    #[must_use]
    pub fn observation_amended_elsewhere(
        id: u64,
        patient_id: core::ids::PatientId,
        observation_id: core::ids::ObservationId,
        by_user_id: core::ids::UserId,
    ) -> Self {
        Self {
            id,
            payload: p_ev::ServerEventPayload::ObservationAmendedElsewhere {
                patient_id: patient_id.into(),
                observation_id: observation_id.into(),
                by_user_id: by_user_id.into(),
            },
        }
    }
    #[must_use]
    pub fn patient_access_changed(
        id: u64,
        patient_id: core::ids::PatientId,
        user_id: core::ids::UserId,
        level: Option<core::patient_access::AccessLevel>,
    ) -> Self {
        Self {
            id,
            payload: p_ev::ServerEventPayload::PatientAccessChanged {
                patient_id: patient_id.into(),
                user_id: user_id.into(),
                level: level.map(Into::into),
            },
        }
    }
    #[must_use]
    pub fn forced_logout(id: u64, reason: String) -> Self {
        Self {
            id,
            payload: p_ev::ServerEventPayload::ForcedLogout { reason },
        }
    }
}
