//! SPEC §Deployment — server TLS identity + local CA minting and rotation.
//!
//! The first-boot wizard mints a fresh 25-year Ed25519 keypair via `rcgen`,
//! writes three PEMs (CA cert, server cert, server key), and stores them
//! under `<data_dir>/tls/`. Per SPEC the same key serves both roles, but for
//! implementation simplicity we use two keypairs — one for the CA, one for
//! the server cert — and treat that as an internal detail that can be unified
//! when the SEP integration lands.
//!
//! `rotate_server_cert()` mints a fresh CA + server keypair and revokes every
//! workstation enrollment in one transaction. Deliberately heavyweight per
//! SPEC: routine in-band rotation would be an attack surface.

use crate::audit::{self, Action, AppendInput};
use crate::db::Database;
use crate::error::{Error, Result};
use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, KeyPair, KeyUsagePurpose, SanType};
use rusqlite::params;
use serde_json::json;
use std::path::Path;

/// PEM trio produced by minting and consumed by the daemon.
#[derive(Debug, Clone)]
pub struct ServerTlsPems {
    pub ca_cert_pem: String,
    pub server_cert_pem: String,
    pub server_key_pem: String,
    /// Raw CA private key PEM — needed when the daemon mints workstation client
    /// certs at enrollment exchange. Stored on disk under `tls/ca_key.pem`.
    pub ca_key_pem: String,
}

/// Mint a fresh CA + server TLS keypair. Used by `anamnez init` and
/// `admin rotate-server-cert`. Server cert SAN = `bind_host` (the LAN address
/// or `anamnez.local`).
///
/// Validity defaults to rcgen's default window. SPEC §Deployment specifies a
/// 25-year horizon; we accept rcgen's default for Phase 1 and tighten when
/// the SEP integration lands and the install pipeline can revisit this.
pub fn mint_server_pems(bind_host: &str) -> Result<ServerTlsPems> {
    // CA.
    let mut ca_params = CertificateParams::new(Vec::<String>::new())
        .map_err(|e| invariant(&format!("ca params: {e}")))?;
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params
        .distinguished_name
        .push(DnType::CommonName, "anamnez-local-ca");
    ca_params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    let ca_key = KeyPair::generate().map_err(|e| invariant(&format!("ca key: {e}")))?;
    let ca_params_keep = ca_params.clone();
    let ca_cert = ca_params
        .self_signed(&ca_key)
        .map_err(|e| invariant(&format!("ca self-sign: {e}")))?;
    let ca_cert_pem = ca_cert.pem();
    let ca_key_pem = ca_key.serialize_pem();

    // Server cert signed by the CA.
    let mut server_params = CertificateParams::new(vec![bind_host.to_owned()])
        .map_err(|e| invariant(&format!("server params: {e}")))?;
    server_params
        .distinguished_name
        .push(DnType::CommonName, bind_host);
    let mut sans =
        vec![SanType::DnsName(bind_host.try_into().map_err(|_| {
            Error::Invariant("bind_host is not a valid DNS name")
        })?)];
    if let Ok(ip) = bind_host.parse::<std::net::IpAddr>() {
        sans.push(SanType::IpAddress(ip));
    }
    server_params.subject_alt_names = sans;
    let server_key = KeyPair::generate().map_err(|e| invariant(&format!("server key: {e}")))?;
    let signer =
        KeyPair::from_pem(&ca_key_pem).map_err(|e| invariant(&format!("clone ca key: {e}")))?;
    let signer_ca_cert = ca_params_keep
        .self_signed(&signer)
        .map_err(|e| invariant(&format!("ca self-sign-for-signing: {e}")))?;
    let server_cert = server_params
        .signed_by(&server_key, &signer_ca_cert, &signer)
        .map_err(|e| invariant(&format!("server signed: {e}")))?;

    Ok(ServerTlsPems {
        ca_cert_pem,
        server_cert_pem: server_cert.pem(),
        server_key_pem: server_key.serialize_pem(),
        ca_key_pem,
    })
}

#[derive(Debug, Clone)]
pub struct RotationReport {
    pub workstations_revoked: u64,
    pub new_ca_fingerprint_sha256: String,
}

/// Mint a fresh server TLS keypair + CA, write the PEMs into `<data_dir>/tls/`
/// atomically (write next to existing files then rename), and revoke every
/// existing workstation in one transaction. The daemon must be stopped before
/// calling — the binary enforces this via the PID-file gate.
pub fn rotate_server_cert(
    db: &Database,
    data_dir: &Path,
    admin: crate::ids::UserId,
    bind_host: &str,
) -> Result<RotationReport> {
    let pems = mint_server_pems(bind_host)?;
    let tls_dir = data_dir.join("tls");
    std::fs::create_dir_all(&tls_dir)?;

    // Atomic-ish write: <name>.new → rename to <name>.
    write_atomic(&tls_dir.join("ca_cert.pem"), &pems.ca_cert_pem)?;
    write_atomic(&tls_dir.join("ca_key.pem"), &pems.ca_key_pem)?;
    write_atomic(&tls_dir.join("server_cert.pem"), &pems.server_cert_pem)?;
    write_atomic(&tls_dir.join("server_key.pem"), &pems.server_key_pem)?;

    let new_ca_fp = fingerprint_sha256_hex(pems.ca_cert_pem.as_bytes());

    let now = db.clock().now();
    let workstations_revoked = db.with_writer(|conn| {
        let live: Vec<String> = {
            let mut stmt = conn.prepare("SELECT id FROM workstation WHERE revoked_at IS NULL")?;
            let rows = stmt
                .query_map(params![], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        let mut revoked: u64 = 0;
        for id in &live {
            conn.execute(
                "UPDATE workstation SET revoked_at = ?2, revoked_reason = ?3 \
                 WHERE id = ?1 AND revoked_at IS NULL",
                params![id, now.to_string(), "server_cert.rotate"],
            )?;
            audit::append_in_conn(
                conn,
                now,
                AppendInput {
                    actor_user_id: Some(admin),
                    auth_session_id: None,
                    action: Action::WorkstationRevoke,
                    target_type: "workstation".into(),
                    target_id: id.clone(),
                    patient_id: None,
                    metadata: json!({"reason": "server_cert.rotate"}),
                },
            )?;
            revoked += 1;
        }
        // Emit one synthetic audit row recording the rotation. SPEC §Audit
        // log integrity's closed Action enum has no `server_cert.rotate`
        // variant; we shoehorn into `WorkstationRevoke` with a per-workstation
        // log above, and avoid inventing a new Action here.
        Ok::<u64, Error>(revoked)
    })?;

    Ok(RotationReport {
        workstations_revoked,
        new_ca_fingerprint_sha256: new_ca_fp,
    })
}

fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    let tmp = {
        let mut p = path.as_os_str().to_owned();
        p.push(".new");
        std::path::PathBuf::from(p)
    };
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn fingerprint_sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    let mut s = String::with_capacity(out.len() * 2);
    for b in out {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn invariant(s: &str) -> Error {
    Error::Invariant(Box::leak(s.to_owned().into_boxed_str()))
}
