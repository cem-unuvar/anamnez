//! Subsystem I — Extraction + patient analysis. README §Analysis, §Features → Collection.

#![allow(clippy::wildcard_imports)]

use anamnez_core::analysis;
use anamnez_core::audit::Action;
use anamnez_core::ids::UserId;
use anamnez_core::llm::cache_key::CacheKey;
use anamnez_core::llm::extractor::Extractor;
use anamnez_core::llm::{ExtractedObservation, LlmCall, LlmExtractor};
use anamnez_core::patient::{self, NewPatient, SexAssignedAtBirth};
use anamnez_core::test_support::prelude::*;
use async_trait::async_trait;
use rusqlite::params;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn prompt_version_is_set() {
    use anamnez_core::analysis::prompt::{PROMPT_VERSION, SYSTEM_PROMPT};
    assert!(!PROMPT_VERSION.is_empty());
    assert!(SYSTEM_PROMPT.contains("Turkish") || SYSTEM_PROMPT.contains("Türkçe"));
    assert!(SYSTEM_PROMPT.contains("markdown"));
}

/// Inline `LlmExtractor` impl that returns a canned response — easier than wiring
/// the full fixture-cache machinery for unit tests of the consumers.
struct CannedLlm {
    text: String,
}

#[async_trait]
impl LlmExtractor for CannedLlm {
    async fn complete(
        &self,
        call: LlmCall,
    ) -> anamnez_core::Result<anamnez_core::llm::LlmResponse> {
        assert_eq!(call.temperature, 0.0, "temperature must be pinned to 0");
        Ok(anamnez_core::llm::LlmResponse {
            text: self.text.clone(),
        })
    }
}

#[test]
fn extractor_never_invents_codes_returns_null_code_with_preserved_display_text() {
    // Fixture LLM returns an explicit JSON array with code = null + display_text.
    let candidate = json!([
        {
            "code_system": null,
            "code": null,
            "display_text": "boyun ağrısı",
            "text_span": [0, 12],
            "effective_period_start": "2026-01-01T00:00:00Z",
            "effective_period_end": null,
            "value_quantity": null,
            "value_string": null,
            "value_codeable": null,
            "confidence": 0.7
        }
    ]);
    let llm: Arc<dyn LlmExtractor> = Arc::new(CannedLlm {
        text: candidate.to_string(),
    });
    let extractor = Extractor::new(llm, "test-provider".into(), "test-model".into());

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let observations: Vec<ExtractedObservation> = rt
        .block_on(extractor.extract_observations("boyun ağrısı 4 hafta önce başladı", "{}"))
        .expect("extract");
    assert_eq!(observations.len(), 1);
    assert!(observations[0].code.is_none());
    assert!(observations[0].code_system.is_none());
    assert_eq!(observations[0].display_text, "boyun ağrısı");
}

#[test]
fn patient_analysis_generate_persists_row_and_emits_audit() {
    let temp = TempDb::new().expect("TempDb opens");

    // Seed user + patient.
    let creator_id = UserId::new();
    temp.db
        .with_writer(|conn| {
            conn.execute(
                "INSERT INTO user (id, email, display_name, role, password_hash, created_at) \
                 VALUES (?1, 'alice@x', 'Alice', 'provider', '!', '2026-01-01T00:00:00Z')",
                params![creator_id.as_uuid().to_string()],
            )?;
            Ok(())
        })
        .expect("seed user");

    let patient = patient::create(
        &temp.db,
        creator_id,
        NewPatient {
            mrn: None,
            given_names: "[TEST] A".into(),
            family_name: "[TEST] L".into(),
            preferred_name: None,
            date_of_birth: jiff::civil::date(1980, 5, 1),
            sex_assigned_at_birth: SexAssignedAtBirth::Female,
            gender_identity: None,
            email: None,
            phone: None,
            address: None,
            emergency_contact_name: None,
            emergency_contact_phone: None,
            emergency_contact_relationship: None,
        },
    )
    .expect("create");

    let llm: Arc<dyn LlmExtractor> = Arc::new(CannedLlm {
        text: "# Klinik Özet\nHasta görece sağlıklı görünüyor.".into(),
    });

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let report = rt
        .block_on(analysis::generate(
            &temp.db,
            llm,
            patient.value.id,
            creator_id,
            "test-provider",
            "test-model",
        ))
        .expect("generate");

    assert!(report.report_markdown.contains("Klinik Özet"));
    assert_eq!(report.model_id, "test-model");

    temp.db
        .with_reader(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM patient_analysis WHERE patient_id = ?1",
                params![patient.value.id.as_uuid().to_string()],
                |r| r.get(0),
            )?;
            assert_eq!(count, 1);
            let audit_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action = ?1 AND patient_id = ?2",
                params![
                    Action::AnalysisGenerate.as_str(),
                    patient.value.id.as_uuid().to_string()
                ],
                |r| r.get(0),
            )?;
            assert_eq!(audit_count, 1);
            Ok(())
        })
        .expect("audit + persist check");
}

#[test]
fn fixture_cache_resolves_recorded_fixture() {
    // Exercise the full fixture-cache path: pre-write a fixture JSON under the
    // cache key for known inputs, then verify FixtureLlmExtractor reads it back.
    let tmp = TempDir::new().expect("tempdir");
    let provider_id = "test-provider";
    let model_id = "test-model";
    let combined_prompt = "system\n---\nuser";
    let key = CacheKey::compose(
        provider_id,
        model_id,
        combined_prompt,
        r#"{"temperature":0}"#,
    );
    let key_hex = key.hex();

    let mut path: PathBuf = tmp.path().to_owned();
    path.push("llm");
    std::fs::create_dir_all(&path).unwrap();
    path.push(format!("{key_hex}.json"));
    std::fs::write(&path, r#"{"text":"cached response"}"#).unwrap();

    let cache = FixtureCache::new(tmp.path().to_owned(), "llm");
    let llm = FixtureLlmExtractor::new(cache);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let response = rt
        .block_on(llm.complete(LlmCall {
            provider_id: provider_id.into(),
            model_id: model_id.into(),
            system_prompt: "system".into(),
            user_prompt: "user".into(),
            temperature: 0.0,
        }))
        .expect("cached");
    assert_eq!(response.text, "cached response");
}
