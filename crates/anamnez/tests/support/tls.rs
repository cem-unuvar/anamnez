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
