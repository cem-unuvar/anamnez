//! Development tasks: `record-fixture` for LLM/OCR/STT, signing scripts, build pipelines.
//!
//! Phase 1: stub `record-fixture` subcommand. Real model calls land alongside Phase 6 fixtures.

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("record-fixture") => {
            eprintln!(
                "xtask record-fixture: phase 1 stub — implement when real LLM/OCR/STT impls land"
            );
            std::process::exit(1);
        }
        Some(other) => {
            eprintln!("xtask: unknown subcommand `{other}`");
            std::process::exit(2);
        }
        None => {
            eprintln!("xtask: subcommands — record-fixture");
            std::process::exit(2);
        }
    }
}
