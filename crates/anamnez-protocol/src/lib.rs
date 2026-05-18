//! Wire types shared by `anamnez serve`, CLI subcommands, and the workstation client.
//!
//! Builds for native and `wasm32-unknown-unknown`. The `from-core` feature pulls in
//! `anamnez-core` and provides `From<core::X> for protocol::X` conversions — used by
//! the daemon, never by the wasm-built workstation.

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod access;
pub mod allergy;
pub mod analysis;
pub mod audit;
pub mod auth;
pub mod codesystem;
pub mod consent;
pub mod encounter;
pub mod enroll;
pub mod environment;
pub mod error;
pub mod events;
pub mod health;
pub mod ids;
pub mod medication;
pub mod observation;
pub mod patient;
pub mod source_document;
pub mod stepup;
pub mod versioned;

#[cfg(feature = "from-core")]
pub mod from_core;
