-- SPEC §Data Modelling — `entered_in_error` joins the observation status set.
--
-- Soft-delete pattern, mirroring allergies and medications. Rows in this status
-- stay in the DB for audit but are hidden from problem lists and timelines.
--
-- SQLite cannot widen a CHECK constraint in place, so the observation table is
-- rebuilt with the same schema plus the new status value. The `extraction`
-- table holds an FK to `observation(id)`; `PRAGMA foreign_keys = OFF` lets us
-- swap the target table without tripping the FK during the operation. After
-- the rename, the FK still points at the table named "observation" — same
-- name, same rows — so referential integrity is preserved.

PRAGMA foreign_keys = OFF;

CREATE TABLE observation_new (
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
    status                  TEXT NOT NULL
        CHECK (status IN ('preliminary','final','amended','entered_in_error')),
    is_problem_list_item    INTEGER NOT NULL DEFAULT 0 CHECK (is_problem_list_item IN (0,1)),
    source_id               TEXT REFERENCES source_document(id) ON DELETE RESTRICT,
    encounter_id            TEXT REFERENCES encounter(id) ON DELETE RESTRICT,
    extracted_by            TEXT NOT NULL CHECK (extracted_by IN ('manual','llm')),
    model_version           TEXT,
    confidence              REAL,
    version                 INTEGER NOT NULL DEFAULT 1,
    -- `final` status still requires (code, code_system). `entered_in_error` is
    -- intentionally exempt because the soft-delete transition keeps the row
    -- as-was apart from the status flip; existing rows already passed the
    -- check when they were 'final'/'preliminary'/'amended', and clearing
    -- (code, code_system) is not a path the API offers.
    CHECK (status <> 'final' OR (code IS NOT NULL AND code_system IS NOT NULL)),
    CHECK ((value_codeable_system IS NULL) = (value_codeable_code IS NULL))
) STRICT;

INSERT INTO observation_new
    SELECT id, patient_id, recorded_at, effective_period_start, effective_period_end,
           code, code_system, display_text,
           value_quantity_value, value_quantity_unit, value_string,
           value_codeable_system, value_codeable_code,
           status, is_problem_list_item, source_id, encounter_id,
           extracted_by, model_version, confidence, version
    FROM observation;

DROP TABLE observation;
ALTER TABLE observation_new RENAME TO observation;

-- Recreate the indexes from V0002 — DROP TABLE removed them.
CREATE INDEX ix_observation_patient_recorded ON observation(patient_id, recorded_at);
CREATE INDEX ix_observation_problem_list     ON observation(patient_id) WHERE is_problem_list_item = 1;

PRAGMA foreign_keys = ON;
