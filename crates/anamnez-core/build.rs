//! Build-time sanity: assert that the `code-systems/<system>/normalized.csv` files
//! exist at the expected paths and emit `cargo:rerun-if-changed` for each. Also
//! derives the dev code-systems Ed25519 pubkey from a fixed seed and writes it to
//! `$OUT_DIR/codesystems-dev.pub` so `pubkey::EMBEDDED_PUBKEY` and the matching
//! `test_support::dev_bundle::signing_key()` stay in sync without committing the
//! pubkey bytes.

use std::path::PathBuf;

const SYSTEMS: &[&str] = &[
    "atc",
    "titck",
    "icd10-tm",
    "loinc",
    "sut",
    "skrs-vp",
    "anamnez-sym",
];

/// Fixed seed for the dev code-systems signing key. Must match
/// `test_support::dev_bundle::SIGNING_SEED`. Dev-only — production builds override
/// the embedded pubkey via the `ANAMNEZ_CODESYSTEM_PUBKEY` env var (see pubkey.rs).
const DEV_SIGNING_SEED: [u8; 32] = *b"anamnez-codesys-dev-key-seed-v01";

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .expect("crate has parent")
        .parent()
        .expect("workspace has parent")
        .to_path_buf();

    for system in SYSTEMS {
        let csv = repo_root
            .join("code-systems")
            .join(system)
            .join("normalized.csv");
        assert!(
            csv.exists(),
            "expected code-systems CSV at {} — was it moved?",
            csv.display()
        );
        println!("cargo:rerun-if-changed={}", csv.display());
    }

    let signing = ed25519_dalek::SigningKey::from_bytes(&DEV_SIGNING_SEED);
    let pubkey_bytes = signing.verifying_key().to_bytes();
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));
    let out_path = out_dir.join("codesystems-dev.pub");
    std::fs::write(&out_path, pubkey_bytes).expect("write dev pubkey");

    println!("cargo:rerun-if-changed=build.rs");
}
