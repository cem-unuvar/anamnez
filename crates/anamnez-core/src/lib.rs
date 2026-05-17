//! anamnez-core — server-side functionality for the anamnez clinical records appliance.
//!
//! See `/Users/Shared/code/anamnez/README.md` for the binding spec.

pub mod allergy;
pub mod analysis;
pub mod audit;
pub mod auth;
pub mod blobs;
pub mod bootstrap;
pub mod code_systems;
pub mod config;
pub mod consent;
pub mod db;
pub mod encounter;
pub mod env;
pub mod error;
pub mod ids;
pub mod key_custody;
pub mod kvkk;
pub mod llm;
pub mod locking;
pub mod medication;
pub mod observation;
pub mod ocr;
pub mod patient;
pub mod patient_access;
pub mod rng;
pub mod source_document;
pub mod stt;
pub mod time;
pub mod user;
pub mod wire;
pub mod workstation;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub use error::{Error, Result};
