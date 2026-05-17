//! `rcgen`-based PEM cert minting for layer-2 tests.

use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, KeyPair, KeyUsagePurpose, SanType};

#[allow(clippy::struct_field_names)]
pub struct MintedCa {
    pub ca_cert_pem: String,
    pub ca_key_pair: KeyPair,
    pub ca_params: CertificateParams,
}

pub struct MintedLeaf {
    pub cert_pem: String,
    pub key_pem: String,
}

pub fn mint_ca() -> MintedCa {
    let mut params = CertificateParams::new(vec![]).expect("ca params");
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params
        .distinguished_name
        .push(DnType::CommonName, "anamnez-local-ca");
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    let key_pair = KeyPair::generate().expect("ca key");
    let keep = params.clone();
    let cert = params.self_signed(&key_pair).expect("ca self-sign");
    MintedCa {
        ca_cert_pem: cert.pem(),
        ca_key_pair: key_pair,
        ca_params: keep,
    }
}

pub fn mint_server_cert(ca: &MintedCa, dns_name: &str) -> MintedLeaf {
    let mut params = CertificateParams::new(vec![dns_name.to_owned()]).expect("server params");
    params.distinguished_name.push(DnType::CommonName, dns_name);
    params.subject_alt_names = vec![
        SanType::DnsName(dns_name.try_into().expect("dns_name")),
        SanType::IpAddress("127.0.0.1".parse().unwrap()),
    ];
    let key_pair = KeyPair::generate().expect("server key");
    let signer_kp = KeyPair::from_pem(&ca.ca_key_pair.serialize_pem()).expect("clone ca key");
    let ca_cert = ca
        .ca_params
        .clone()
        .self_signed(&signer_kp)
        .expect("ca self-sign for signing");
    let cert = params
        .signed_by(&key_pair, &ca_cert, &signer_kp)
        .expect("server signed");
    MintedLeaf {
        cert_pem: cert.pem(),
        key_pem: key_pair.serialize_pem(),
    }
}

pub fn mint_workstation_cert(ca: &MintedCa, device_id: uuid::Uuid) -> MintedLeaf {
    let cn = device_id.to_string();
    let mut params = CertificateParams::new(Vec::<String>::new()).expect("ws params");
    params.distinguished_name.push(DnType::CommonName, cn);
    let key_pair = KeyPair::generate().expect("ws key");
    let signer_kp = KeyPair::from_pem(&ca.ca_key_pair.serialize_pem()).expect("clone ca key");
    let ca_cert = ca
        .ca_params
        .clone()
        .self_signed(&signer_kp)
        .expect("ca self-sign for signing");
    let cert = params
        .signed_by(&key_pair, &ca_cert, &signer_kp)
        .expect("workstation signed");
    MintedLeaf {
        cert_pem: cert.pem(),
        key_pem: key_pair.serialize_pem(),
    }
}
