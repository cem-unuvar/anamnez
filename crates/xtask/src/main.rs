//! Development tasks.
//!
//! Subcommands:
//!   - `record-fixture` (stub) — placeholder for LLM/OCR/STT fixture recording.
//!   - `dev-up` — bootstrap a local data_dir (if missing) and launch `anamnez serve`.
//!   - `dev-seed` — talk HTTP to the running daemon, mint an enrollment URI for the
//!                  Tauri workstation, and create a `[TEST]` patient.
//!
//! `dev-up` and `dev-seed` share the on-disk layout described in `paths.rs`:
//!
//!     target/dev-data/
//!       anamnez.sqlite
//!       wrap_sep.bin            # FixtureSep-wrapped
//!       wrap_recovery.bin       # Argon2id-wrapped — used at daemon boot
//!       config.toml
//!       tls/
//!         ca_cert.pem, ca_key.pem, server_cert.pem, server_key.pem
//!       dev-workstation/
//!         cert.pem, key.pem     # mTLS identity for the xtask itself
//!       .recovery-code          # plaintext — only used to set ANAMNEZ_RECOVERY_CODE
//!       .last-enrollment-uri    # the URI dev-seed prints, persisted between runs

mod dev_seed;
mod dev_up;
mod paths;

fn main() {
    let mut args = std::env::args().skip(1);
    let sub = args.next();
    let rest: Vec<String> = args.collect();
    let code = match sub.as_deref() {
        Some("record-fixture") => {
            eprintln!(
                "xtask record-fixture: phase 1 stub — implement when real LLM/OCR/STT impls land"
            );
            1
        }
        Some("dev-up") => match dev_up::run(rest) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("dev-up: {e}");
                1
            }
        },
        Some("dev-seed") => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio rt");
            match rt.block_on(dev_seed::run(rest)) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("dev-seed: {e}");
                    1
                }
            }
        }
        Some(other) => {
            eprintln!("xtask: unknown subcommand `{other}`");
            2
        }
        None => {
            eprintln!("xtask: subcommands — record-fixture, dev-up, dev-seed");
            2
        }
    };
    std::process::exit(code);
}
