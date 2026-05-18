//! Build reqwest clients with our CA + workstation client cert.

use reqwest::tls::{Certificate, Identity};

pub fn client(ca_pem: &str, client_cert_pem: &str, client_key_pem: &str) -> reqwest::Client {
    let ca = Certificate::from_pem(ca_pem.as_bytes()).expect("ca cert parse");
    let identity_pem = format!("{client_cert_pem}\n{client_key_pem}");
    let identity = Identity::from_pem(identity_pem.as_bytes()).expect("client identity parse");
    reqwest::Client::builder()
        .add_root_certificate(ca)
        .identity(identity)
        .danger_accept_invalid_hostnames(true)
        .build()
        .expect("reqwest client")
}

/// Reqwest client that trusts the CA but presents no client cert — i.e., the
/// pre-enrollment workstation. Only `/v1/enroll/exchange` is reachable; everything
/// else is rejected by the `require_device_id` middleware.
pub fn client_no_identity(ca_pem: &str) -> reqwest::Client {
    let ca = Certificate::from_pem(ca_pem.as_bytes()).expect("ca cert parse");
    reqwest::Client::builder()
        .add_root_certificate(ca)
        .danger_accept_invalid_hostnames(true)
        .build()
        .expect("reqwest client")
}
