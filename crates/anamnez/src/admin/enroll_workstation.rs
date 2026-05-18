//! `anamnez admin enroll-workstation` — mint a one-time enrollment token and
//! print the `anamnez://enroll?...` URI the workstation operator pastes into
//! their client.
//!
//! The daemon-side token-exchange route is part of the next slice (workstation
//! client). The pending row lives in `workstation_enrollment` until claimed.

use crate::admin::cli_actor;
use crate::cli::AdminEnrollWorkstationArgs;
use crate::dispatch_common::{data_dir, load_config, open_db, refuse_while_serve_alive};
use anamnez_core::error::{Error, Result};
use anamnez_core::rng::OsRng;
use anamnez_core::user;
use anamnez_core::workstation::{self, Mode, NewEnrollment};
use secrecy::ExposeSecret;
use sha2::{Digest, Sha256};

pub fn run(args: AdminEnrollWorkstationArgs) -> Result<()> {
    refuse_while_serve_alive(args.pid_file.as_deref())?;
    let cfg = load_config(&args.config)?;
    let db = open_db(&cfg)?;
    let admin = cli_actor(&db)?;
    let dd = data_dir(&cfg)?;

    let mode = Mode::parse(&args.mode)?;
    let bound_user_id = match (&args.bind_user_email, mode) {
        (Some(email), Mode::Bound) => {
            Some(user::find_by_email(&db, email)?.ok_or(Error::NotFound)?.id)
        }
        (None, Mode::Bound) => {
            return Err(Error::Invariant(
                "--bind-user-email required for mode=bound",
            ))
        }
        (Some(_), Mode::Shared) => {
            return Err(Error::Invariant(
                "--bind-user-email not allowed with mode=shared",
            ))
        }
        (None, Mode::Shared) => None,
    };

    let server_cert_pem = std::fs::read_to_string(dd.join("tls").join("server_cert.pem"))?;
    let fingerprint = fingerprint_sha256_hex_of_pem_leaf(&server_cert_pem)?;

    let minted = workstation::mint_enrollment(
        &db,
        admin,
        &OsRng,
        NewEnrollment {
            label: args.label.clone(),
            mode,
            bound_user_id,
            host: args.host.clone(),
            server_fingerprint_sha256: fingerprint,
        },
    )?;

    println!(
        "anamnez admin enroll-workstation: enrollment_id={}",
        minted.enrollment_id
    );
    println!("  uri:   {}", minted.uri);
    println!(
        "  token: {} (one-time; redeem via /v1/enroll/exchange)",
        minted.token.expose_secret()
    );
    Ok(())
}

/// SHA-256 of the **DER-encoded** leaf cert (matches the workstation client's pin
/// verifier — see `anamnez_client_core::transport_native::pin_verifier`).
fn fingerprint_sha256_hex_of_pem_leaf(pem: &str) -> Result<String> {
    let mut cursor = std::io::Cursor::new(pem.as_bytes());
    let der = rustls_pemfile::certs(&mut cursor)
        .next()
        .ok_or(Error::Invariant("server_cert.pem: empty"))?
        .map_err(|_| Error::Invariant("server_cert.pem: invalid PEM"))?;
    let mut h = Sha256::new();
    h.update(der.as_ref());
    let out = h.finalize();
    let mut s = String::with_capacity(out.len() * 2);
    for b in out {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    Ok(s)
}
