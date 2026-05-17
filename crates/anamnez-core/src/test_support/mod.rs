//! Test infrastructure for `anamnez-core` and downstream crates.
//!
//! Gated behind `#[cfg(any(test, feature = "test-support"))]` so production builds
//! never link this code. Step 1.3 fills in the bodies; Step 1.2 declares the surface.

pub mod assertions;
pub mod blob;
pub mod clock;
pub mod code_systems;
pub mod dev_bundle;
pub mod fixture_cache;
pub mod harness;
pub mod llm;
pub mod ocr;
pub mod rng;
pub mod seed;
pub mod sep;
pub mod stt;

pub mod prelude {
    //! `use anamnez_core::test_support::prelude::*;` at the top of every integration test.

    pub use super::assertions::*;
    pub use super::clock::TestClock;
    pub use super::fixture_cache::FixtureCache;
    pub use super::harness::TempDb;
    pub use super::llm::FixtureLlmExtractor;
    pub use super::ocr::FixtureOcrEngine;
    pub use super::rng::DeterministicRng;
    pub use super::seed::TestWorld;
    pub use super::sep::FixtureSep;
    pub use super::stt::FixtureTranscriber;
}
