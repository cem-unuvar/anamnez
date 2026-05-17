//! Custom rustls `ClientCertVerifier` — accepts certs signed by the local CA and
//! enforces the in-memory deny-set keyed by `device_id` (the cert CN).

use anamnez_core::error::{Error, Result};
use anamnez_core::ids::WorkstationId;
use parking_lot::RwLock;
use rustls::client::danger::HandshakeSignatureValid;
use rustls::pki_types::CertificateDer;
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::server::WebPkiClientVerifier;
use rustls::DigitallySignedStruct;
use rustls::DistinguishedName;
use rustls::RootCertStore;
use rustls::SignatureScheme;
use std::collections::HashSet;
use std::sync::Arc;
use x509_parser::prelude::{FromDer, X509Certificate};

/// Build the rustls `ClientCertVerifier` that the daemon uses for mTLS.
pub fn build_verifier(
    ca_pem: &str,
    revoked: Arc<RwLock<HashSet<WorkstationId>>>,
) -> Result<Arc<dyn ClientCertVerifier>> {
    let mut roots = RootCertStore::empty();
    let mut cursor = std::io::Cursor::new(ca_pem.as_bytes());
    for cert in rustls_pemfile::certs(&mut cursor) {
        let cert = cert.map_err(Error::from)?;
        roots
            .add(cert)
            .map_err(|_| Error::Invariant("ca_cert.pem: invalid root"))?;
    }
    let inner = WebPkiClientVerifier::builder(Arc::new(roots))
        .build()
        .map_err(|_| Error::Invariant("WebPkiClientVerifier::build"))?;
    Ok(Arc::new(AnamnezClientVerifier { inner, revoked }))
}

/// Extract the `WorkstationId` from a peer client cert. Returns the parsed UUID
/// if the cert's CN looks like one.
pub fn workstation_id_from_cert(cert: &CertificateDer<'_>) -> Result<WorkstationId> {
    let (_, parsed) = X509Certificate::from_der(cert.as_ref())
        .map_err(|_| Error::Invariant("client cert: failed to parse"))?;
    // CN appears in the subject; iterate to find it.
    for rdn in parsed.subject().iter() {
        for atv in rdn.iter() {
            if atv.attr_type().to_id_string() == "2.5.4.3" {
                // 2.5.4.3 = commonName
                let cn = atv
                    .as_str()
                    .map_err(|_| Error::Invariant("client cert: CN not utf-8"))?;
                let uuid = uuid::Uuid::parse_str(cn)
                    .map_err(|_| Error::Invariant("client cert: CN not a UUID"))?;
                return Ok(WorkstationId(uuid));
            }
        }
    }
    Err(Error::Invariant("client cert: CN missing"))
}

#[derive(Debug)]
struct AnamnezClientVerifier {
    inner: Arc<dyn ClientCertVerifier>,
    revoked: Arc<RwLock<HashSet<WorkstationId>>>,
}

impl ClientCertVerifier for AnamnezClientVerifier {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        self.inner.root_hint_subjects()
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<ClientCertVerified, rustls::Error> {
        // PKIX check first.
        self.inner
            .verify_client_cert(end_entity, intermediates, now)?;
        // CN → WorkstationId.
        let device_id = workstation_id_from_cert(end_entity)
            .map_err(|_| rustls::Error::General("client cert: CN not a workstation UUID".into()))?;
        if self.revoked.read().contains(&device_id) {
            return Err(rustls::Error::General("client cert: device revoked".into()));
        }
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }

    fn offer_client_auth(&self) -> bool {
        true
    }
}
