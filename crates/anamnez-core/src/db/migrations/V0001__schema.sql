-- README §Data Modelling + §Storage — base schema for anamnez-core.
--
-- All tables are STRICT. UUIDs are stored as TEXT (RFC 4122 hyphenated lowercase).
-- Timestamps are stored as TEXT (RFC 3339, UTC). Booleans are INTEGER 0/1.

-- ─── Users, workstations, sessions ────────────────────────────────────────────

CREATE TABLE user (
    id            TEXT PRIMARY KEY NOT NULL,
    email         TEXT NOT NULL UNIQUE,
    display_name  TEXT NOT NULL,
    role          TEXT NOT NULL CHECK (role IN ('admin', 'provider')),
    password_hash TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    disabled_at   TEXT
) STRICT;

CREATE TABLE workstation (
    id               TEXT PRIMARY KEY NOT NULL,
    label            TEXT NOT NULL,
    mode             TEXT NOT NULL CHECK (mode IN ('bound', 'shared')),
    bound_user_id    TEXT REFERENCES user(id) ON DELETE RESTRICT,
    cert_serial      TEXT NOT NULL UNIQUE,
    cert_fingerprint TEXT NOT NULL UNIQUE,
    enrolled_at      TEXT NOT NULL,
    enrolled_by      TEXT NOT NULL REFERENCES user(id) ON DELETE RESTRICT,
    last_seen_at     TEXT,
    revoked_at       TEXT,
    revoked_reason   TEXT,
    CHECK ((mode = 'bound') = (bound_user_id IS NOT NULL))
) STRICT;

CREATE TABLE auth_session (
    id                   TEXT PRIMARY KEY NOT NULL,
    user_id              TEXT NOT NULL REFERENCES user(id) ON DELETE RESTRICT,
    device_id            TEXT NOT NULL REFERENCES workstation(id) ON DELETE RESTRICT,
    refresh_token_hash   BLOB NOT NULL,
    refresh_expires_at   TEXT NOT NULL,
    absolute_expires_at  TEXT NOT NULL,
    created_at           TEXT NOT NULL,
    last_seen_at         TEXT NOT NULL,
    revoked_at           TEXT
) STRICT;

-- ─── Patients & access ────────────────────────────────────────────────────────

CREATE TABLE patient (
    id                              TEXT PRIMARY KEY NOT NULL,
    mrn                             TEXT,
    given_names                     TEXT NOT NULL,
    family_name                     TEXT NOT NULL,
    preferred_name                  TEXT,
    date_of_birth                   TEXT NOT NULL,
    sex_assigned_at_birth           TEXT NOT NULL
        CHECK (sex_assigned_at_birth IN ('female','male','intersex','unknown')),
    gender_identity                 TEXT,
    email                           TEXT,
    phone                           TEXT,
    address                         TEXT,
    emergency_contact_name          TEXT,
    emergency_contact_phone         TEXT,
    emergency_contact_relationship  TEXT,
    created_by                      TEXT NOT NULL REFERENCES user(id) ON DELETE RESTRICT,
    created_at                      TEXT NOT NULL,
    updated_at                      TEXT NOT NULL,
    deceased_at                     TEXT,
    archived_at                     TEXT,
    suppressed_at                   TEXT,
    suppression_reason              TEXT,
    notice_acknowledged_at          TEXT,
    version                         INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE TABLE patient_access (
    patient_id TEXT NOT NULL REFERENCES patient(id) ON DELETE CASCADE,
    user_id    TEXT NOT NULL REFERENCES user(id)    ON DELETE CASCADE,
    level      TEXT NOT NULL CHECK (level IN ('owner','collaborator','read_only')),
    PRIMARY KEY (patient_id, user_id)
) STRICT;

CREATE TABLE patient_consent (
    id                  TEXT PRIMARY KEY NOT NULL,
    patient_id          TEXT NOT NULL REFERENCES patient(id) ON DELETE RESTRICT,
    purpose             TEXT NOT NULL
        CHECK (purpose IN ('lawyer_transfer','research_non_anonymized','other_clinic_referral')),
    granted_at          TEXT NOT NULL,
    granted_by          TEXT NOT NULL REFERENCES user(id) ON DELETE RESTRICT,
    evidence_source_id  TEXT REFERENCES source_document(id) ON DELETE RESTRICT,
    revoked_at          TEXT,
    notes               TEXT,
    version             INTEGER NOT NULL DEFAULT 1
) STRICT;

-- ─── Clinical rows ────────────────────────────────────────────────────────────

CREATE TABLE encounter (
    id                  TEXT PRIMARY KEY NOT NULL,
    patient_id          TEXT NOT NULL REFERENCES patient(id) ON DELETE RESTRICT,
    provider_id         TEXT NOT NULL REFERENCES user(id)    ON DELETE RESTRICT,
    kind                TEXT NOT NULL
        CHECK (kind IN ('in_person','phone','video','async_document')),
    reason_text         TEXT NOT NULL,
    reason_code         TEXT,
    reason_code_system  TEXT
        CHECK (reason_code_system IS NULL OR reason_code_system IN ('ICD10TM','ANAMNEZ-SYM','SKRS-VP')),
    started_at          TEXT NOT NULL,
    ended_at            TEXT,
    status              TEXT NOT NULL
        CHECK (status IN ('in_progress','finished','cancelled')),
    created_at          TEXT NOT NULL,
    version             INTEGER NOT NULL DEFAULT 1,
    CHECK (status <> 'finished' OR (reason_code IS NOT NULL AND reason_code_system IS NOT NULL))
) STRICT;

CREATE TABLE source_document (
    id                       TEXT PRIMARY KEY NOT NULL,
    patient_id               TEXT NOT NULL REFERENCES patient(id) ON DELETE RESTRICT,
    kind                     TEXT NOT NULL CHECK (kind IN ('note','pdf','audio','image')),
    sha256                   TEXT NOT NULL,
    original_filename        TEXT NOT NULL,
    mime_type                TEXT NOT NULL,
    transcription            TEXT,
    ocr_text                 TEXT,
    encounter_id             TEXT REFERENCES encounter(id) ON DELETE RESTRICT,
    uploaded_at              TEXT NOT NULL,
    context_provided_by_user TEXT,
    recorded_by              TEXT NOT NULL REFERENCES user(id) ON DELETE RESTRICT,
    version                  INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE TABLE observation (
    id                      TEXT PRIMARY KEY NOT NULL,
    patient_id              TEXT NOT NULL REFERENCES patient(id) ON DELETE RESTRICT,
    recorded_at             TEXT NOT NULL,
    effective_period_start  TEXT NOT NULL,
    effective_period_end    TEXT,
    code                    TEXT,
    code_system             TEXT
        CHECK (code_system IS NULL OR code_system IN ('ATC','TITCK','ICD10TM','LOINC','SUT','ANAMNEZ-SYM')),
    display_text            TEXT NOT NULL,
    value_quantity_value    REAL,
    value_quantity_unit     TEXT,
    value_string            TEXT,
    value_codeable_system   TEXT
        CHECK (value_codeable_system IS NULL OR value_codeable_system IN ('ATC','TITCK','ICD10TM','LOINC','SUT','SKRS-VP','ANAMNEZ-SYM')),
    value_codeable_code     TEXT,
    status                  TEXT NOT NULL CHECK (status IN ('preliminary','final','amended')),
    is_problem_list_item    INTEGER NOT NULL DEFAULT 0 CHECK (is_problem_list_item IN (0,1)),
    source_id               TEXT REFERENCES source_document(id) ON DELETE RESTRICT,
    encounter_id            TEXT REFERENCES encounter(id) ON DELETE RESTRICT,
    extracted_by            TEXT NOT NULL CHECK (extracted_by IN ('manual','llm')),
    model_version           TEXT,
    confidence              REAL,
    version                 INTEGER NOT NULL DEFAULT 1,
    -- final status requires (code, code_system)
    CHECK (status <> 'final' OR (code IS NOT NULL AND code_system IS NOT NULL)),
    -- (value_codeable_system, value_codeable_code) are co-nullable
    CHECK ((value_codeable_system IS NULL) = (value_codeable_code IS NULL))
) STRICT;

CREATE TABLE extraction (
    id                  TEXT PRIMARY KEY NOT NULL,
    source_document_id  TEXT NOT NULL REFERENCES source_document(id) ON DELETE RESTRICT,
    observation_id      TEXT NOT NULL REFERENCES observation(id)     ON DELETE RESTRICT,
    text_span_start     INTEGER NOT NULL,
    text_span_end       INTEGER NOT NULL,
    confidence          REAL NOT NULL,
    model_version       TEXT NOT NULL,
    reviewed_by         TEXT REFERENCES user(id) ON DELETE RESTRICT,
    reviewed_at         TEXT
) STRICT;

CREATE TABLE allergy (
    id              TEXT PRIMARY KEY NOT NULL,
    patient_id      TEXT NOT NULL REFERENCES patient(id) ON DELETE RESTRICT,
    code            TEXT,
    code_system     TEXT CHECK (code_system IS NULL OR code_system = 'ATC'),
    display_text    TEXT NOT NULL,
    severity        TEXT NOT NULL CHECK (severity IN ('mild','moderate','severe','life_threatening')),
    reaction_text   TEXT,
    status          TEXT NOT NULL CHECK (status IN ('active','inactive','entered_in_error')),
    onset_date      TEXT,
    recorded_at     TEXT NOT NULL,
    recorded_by     TEXT NOT NULL REFERENCES user(id) ON DELETE RESTRICT,
    source_id       TEXT REFERENCES source_document(id) ON DELETE RESTRICT,
    encounter_id    TEXT REFERENCES encounter(id) ON DELETE RESTRICT,
    version         INTEGER NOT NULL DEFAULT 1,
    CHECK ((code IS NULL) = (code_system IS NULL))
) STRICT;

CREATE TABLE medication (
    id              TEXT PRIMARY KEY NOT NULL,
    patient_id      TEXT NOT NULL REFERENCES patient(id) ON DELETE RESTRICT,
    code            TEXT NOT NULL,
    code_system     TEXT NOT NULL CHECK (code_system IN ('ATC','TITCK')),
    display_text    TEXT NOT NULL,
    dose_quantity   REAL,
    dose_unit       TEXT,
    frequency_text  TEXT,
    route           TEXT NOT NULL CHECK (route IN ('oral','iv','im','topical','inhaled','other')),
    started_at      TEXT NOT NULL,
    ended_at        TEXT,
    reason_text     TEXT,
    status          TEXT NOT NULL CHECK (status IN ('active','completed','stopped','entered_in_error')),
    prescriber_id   TEXT REFERENCES user(id) ON DELETE RESTRICT,
    recorded_at     TEXT NOT NULL,
    recorded_by     TEXT NOT NULL REFERENCES user(id) ON DELETE RESTRICT,
    source_id       TEXT REFERENCES source_document(id) ON DELETE RESTRICT,
    encounter_id    TEXT REFERENCES encounter(id) ON DELETE RESTRICT,
    version         INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE TABLE patient_analysis (
    id                    TEXT PRIMARY KEY NOT NULL,
    patient_id            TEXT NOT NULL REFERENCES patient(id) ON DELETE RESTRICT,
    generated_at          TEXT NOT NULL,
    generated_by          TEXT NOT NULL REFERENCES user(id) ON DELETE RESTRICT,
    model_id              TEXT NOT NULL,
    prompt_version        TEXT NOT NULL,
    report_markdown       TEXT NOT NULL,
    scope_observation_ids TEXT NOT NULL
) STRICT;

-- ─── Audit log (append-only, tamper-evident) ─────────────────────────────────

CREATE TABLE audit_log (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    occurred_at       TEXT NOT NULL,
    actor_user_id     TEXT REFERENCES user(id) ON DELETE RESTRICT,
    auth_session_id   TEXT REFERENCES auth_session(id) ON DELETE RESTRICT,
    action            TEXT NOT NULL,
    target_type       TEXT NOT NULL,
    target_id         TEXT NOT NULL,
    patient_id        TEXT REFERENCES patient(id) ON DELETE RESTRICT,
    metadata          TEXT NOT NULL,
    prev_hash         BLOB NOT NULL,
    row_hash          BLOB NOT NULL
) STRICT;

-- ─── Code-system lookup tables ───────────────────────────────────────────────

CREATE TABLE drug_atc (
    atc_code        TEXT PRIMARY KEY NOT NULL,
    description_en  TEXT,
    description_tr  TEXT
) STRICT;

CREATE TABLE drug_titck (
    barcode             TEXT PRIMARY KEY NOT NULL,
    titck_product_code  TEXT NOT NULL,
    trade_name          TEXT NOT NULL,
    manufacturer        TEXT,
    atc_code            TEXT REFERENCES drug_atc(atc_code) ON DELETE RESTRICT,
    active_substance_tr TEXT,
    dosage_form         TEXT,
    strength_value      REAL,
    strength_unit       TEXT,
    strength_text       TEXT,
    package_size_text   TEXT,
    rx_only             INTEGER CHECK (rx_only IS NULL OR rx_only IN (0,1)),
    reimbursable        INTEGER CHECK (reimbursable IS NULL OR reimbursable IN (0,1)),
    retired_at          TEXT
) STRICT;

CREATE TABLE icd10_tm (
    code            TEXT PRIMARY KEY NOT NULL,
    description_tr  TEXT,
    description_en  TEXT,
    parent_code     TEXT REFERENCES icd10_tm(code),
    is_billable     INTEGER NOT NULL DEFAULT 0 CHECK (is_billable IN (0,1))
) STRICT;

CREATE TABLE loinc (
    code           TEXT PRIMARY KEY NOT NULL,
    long_name_en   TEXT,
    long_name_tr   TEXT,
    component      TEXT,
    unit_default   TEXT,
    scale_typ      TEXT
) STRICT;

CREATE TABLE procedure_sut (
    sut_code        TEXT PRIMARY KEY NOT NULL,
    description_tr  TEXT NOT NULL,
    category        TEXT,
    retired_at      TEXT
) STRICT;

CREATE TABLE visit_purpose_skrs (
    code            TEXT PRIMARY KEY NOT NULL,
    description_tr  TEXT NOT NULL,
    description_en  TEXT,
    retired_at      TEXT
) STRICT;

CREATE TABLE symptom_anamnez (
    code              TEXT PRIMARY KEY NOT NULL,
    display_tr        TEXT NOT NULL,
    display_en        TEXT,
    icd10_suggestion  TEXT REFERENCES icd10_tm(code) ON DELETE SET NULL,
    body_region       TEXT,
    retired_at        TEXT
) STRICT;
