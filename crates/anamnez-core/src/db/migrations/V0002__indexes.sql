-- Partial unique enforcing one `owner` per patient.
CREATE UNIQUE INDEX ux_patient_access_owner
    ON patient_access(patient_id)
    WHERE level = 'owner';

-- Common access patterns.
CREATE INDEX ix_observation_patient_recorded   ON observation(patient_id, recorded_at);
CREATE INDEX ix_observation_problem_list       ON observation(patient_id) WHERE is_problem_list_item = 1;
CREATE INDEX ix_encounter_patient_started      ON encounter(patient_id, started_at);
CREATE INDEX ix_allergy_patient                ON allergy(patient_id);
CREATE INDEX ix_medication_patient             ON medication(patient_id);
CREATE INDEX ix_source_document_patient        ON source_document(patient_id);
CREATE INDEX ix_audit_log_patient_occurred     ON audit_log(patient_id, occurred_at);
CREATE INDEX ix_audit_log_session_occurred     ON audit_log(auth_session_id, occurred_at);
CREATE INDEX ix_audit_log_user_occurred        ON audit_log(actor_user_id, occurred_at);
CREATE INDEX ix_auth_session_user              ON auth_session(user_id);
CREATE INDEX ix_extraction_observation         ON extraction(observation_id);
CREATE INDEX ix_extraction_source              ON extraction(source_document_id);
CREATE INDEX ix_patient_consent_patient        ON patient_consent(patient_id);
CREATE INDEX ix_drug_titck_titck_product_code  ON drug_titck(titck_product_code);
CREATE INDEX ix_drug_titck_atc_code            ON drug_titck(atc_code);
