//! Dev signing key for code-systems bundle tests.
//!
//! The seed mirrors `DEV_SIGNING_SEED` in `build.rs`, which writes the matching
//! verifying-key bytes to `$OUT_DIR/codesystems-dev.pub`. Keeping the seed in
//! two files is uncomfortable but unavoidable: `build.rs` runs before this
//! module compiles and cannot import from the crate.

use crate::code_systems::bundle::{self, Bundle, BundleManifest};
use ed25519_dalek::{Signer, SigningKey};
use std::path::{Path, PathBuf};

/// Must equal `DEV_SIGNING_SEED` in `crates/anamnez-core/build.rs`.
pub const SIGNING_SEED: [u8; 32] = *b"anamnez-codesys-dev-key-seed-v01";

#[must_use]
pub fn signing_key() -> SigningKey {
    SigningKey::from_bytes(&SIGNING_SEED)
}

/// Write `bundle` as JSON to `bundle_path` and a sidecar Ed25519 signature
/// (signed with the dev key) to `<bundle_path>.sig`. Returns the bundle path.
pub fn write_signed_bundle(bundle_path: &Path, bundle: &Bundle) -> std::io::Result<PathBuf> {
    let bytes = serde_json::to_vec_pretty(bundle).expect("bundle serialize");
    std::fs::write(bundle_path, &bytes)?;
    let signature = signing_key().sign(&bytes);
    std::fs::write(bundle::sig_path_for(bundle_path), signature.to_bytes())?;
    Ok(bundle_path.to_path_buf())
}

/// Convenience helper for tests: build a manifest with sensible defaults.
#[must_use]
pub fn manifest(version: &str, built_at: jiff::Timestamp) -> BundleManifest {
    BundleManifest {
        version: version.to_owned(),
        checksum_sha256: "0".repeat(64),
        built_at,
        source_revision_dates: Vec::new(),
    }
}
