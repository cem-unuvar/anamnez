//! Ed25519 public key embedded at build time for code-systems bundle verification.
//!
//! Dev path: `build.rs` derives the pubkey deterministically from
//! `DEV_SIGNING_SEED` and writes it to `$OUT_DIR/codesystems-dev.pub`, which we
//! `include_bytes!` here. The matching signing key lives in
//! `test_support::dev_bundle`. Production builds override at startup via the
//! `ANAMNEZ_CODESYSTEM_PUBKEY` env var pointing at a real pubkey file.

const EMBEDDED_PUBKEY: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/codesystems-dev.pub"));

#[must_use]
pub fn embedded() -> &'static [u8] {
    EMBEDDED_PUBKEY
}
