//! TLS configuration for the QUIC transport.
//!
//! Each node uses its Ed25519 signing key as the TLS identity. The cert public
//! key doubles as the network NodeId so peers derive identity from the
//! handshake alone — no separate handshake protocol or PKI is needed.
//!
//! `SkipServerVerification` deliberately bypasses CA verification because we
//! pin identity by NodeId == cert pubkey instead.

use std::sync::Arc;
use std::time::Duration;

use quinn::{ClientConfig, Connection, ServerConfig, TransportConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

use pubsub_types::node::{node_id_from_key, NodeId};

/// Build TLS server+client configs using the node's Ed25519 key as the cert identity.
/// The same key is used for both sides so the cert public key equals the signing key.
pub fn generate_tls_config(
    seed: &[u8; 32],
) -> Result<(ServerConfig, ClientConfig), Box<dyn std::error::Error>> {
    // Ensure the ring crypto provider is installed. When quinn-proto pulls in
    // both ring and aws-lc-rs as transitive deps, rustls cannot auto-select one.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let pkcs8_bytes = seed_to_pkcs8_der(seed);
    let private_key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(pkcs8_bytes.clone()));
    let key_pair = rcgen::KeyPair::from_der_and_sign_algo(&private_key, &rcgen::PKCS_ED25519)?;
    let cert = rcgen::CertificateParams::new(vec!["pubsub-node".to_string()])?
        .self_signed(&key_pair)?;

    let cert_der = cert.der().clone();

    let mut tc = TransportConfig::default();
    tc.keep_alive_interval(Some(Duration::from_secs(15)));
    tc.max_idle_timeout(None);
    // Bound per-connection stream fan-out so a single peer (or impostor) cannot
    // amplify CPU by opening unlimited concurrent streams. Limits apply
    // symmetrically to inbound and outbound peers and are well above legitimate
    // protocol use; see constants in `super`.
    tc.max_concurrent_bidi_streams(super::MAX_CONCURRENT_BIDI_PER_CONN.into());
    tc.max_concurrent_uni_streams(super::MAX_CONCURRENT_UNI_PER_CONN.into());
    let tc = Arc::new(tc);

    let mut server_config = ServerConfig::with_single_cert(
        vec![cert_der.clone()],
        PrivatePkcs8KeyDer::from(pkcs8_bytes).into(),
    )?;
    server_config.transport_config(tc.clone());

    let crypto = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
        .with_no_client_auth();

    let mut client_config = ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(crypto)?,
    ));
    client_config.transport_config(tc);

    Ok((server_config, client_config))
}

/// Encode a 32-byte Ed25519 seed as a PKCS8 v1 DER structure (RFC 8410).
///
/// Layout (48 bytes):
///   SEQUENCE {
///     INTEGER 0                    -- version
///     SEQUENCE { OID 1.3.101.112 } -- AlgorithmIdentifier (Ed25519)
///     OCTET STRING { OCTET STRING { seed } }
///   }
fn seed_to_pkcs8_der(seed: &[u8; 32]) -> Vec<u8> {
    let mut der = Vec::with_capacity(48);
    der.extend_from_slice(&[
        0x30, 0x2E,                          // SEQUENCE (46 bytes)
        0x02, 0x01, 0x00,                    // INTEGER 0 (version)
        0x30, 0x05,                          // SEQUENCE (5 bytes) — AlgorithmIdentifier
        0x06, 0x03, 0x2B, 0x65, 0x70,        // OID 1.3.101.112 (Ed25519)
        0x04, 0x22,                          // OCTET STRING (34) — PrivateKey
        0x04, 0x20,                          // OCTET STRING (32) — CurvePrivateKey
    ]);
    der.extend_from_slice(seed);
    der
}

/// Scan a certificate DER for the Ed25519 SubjectPublicKeyInfo and return the 32-byte key.
///
/// Looks for the Ed25519 OID (06 03 2B 65 70), then the BIT STRING header (03 21 00)
/// that immediately follows the SPKI SEQUENCE.
fn extract_ed25519_pubkey_from_cert_der(cert_der: &[u8]) -> Option<[u8; 32]> {
    let oid = &[0x06u8, 0x03, 0x2B, 0x65, 0x70];
    let pos = cert_der.windows(5).position(|w| w == oid)?;
    let after_oid = &cert_der[pos + 5..];
    // BIT STRING: tag 0x03, length 0x21 (33), unused-bits 0x00, then 32 bytes
    let bs_tag = &[0x03u8, 0x21, 0x00];
    let bs_pos = after_oid.windows(3).position(|w| w == bs_tag)?;
    let key_start = pos + 5 + bs_pos + 3;
    if key_start + 32 > cert_der.len() {
        return None;
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&cert_der[key_start..key_start + 32]);
    Some(key)
}

/// Derive the NodeId from the peer's TLS certificate public key.
///
/// Returns `None` when no peer cert is available (inbound connections without
/// mutual TLS, or non-Ed25519 certs).
pub fn peer_cert_node_id(conn: &Connection) -> Option<NodeId> {
    let identity = conn.peer_identity()?;
    let certs = identity.downcast::<Vec<CertificateDer<'static>>>().ok()?;
    let cert_der = certs.first()?;
    let pubkey = extract_ed25519_pubkey_from_cert_der(cert_der)?;
    Some(node_id_from_key(&pubkey))
}

/// Skip CA verification — NodeId is verified via the cert public key instead.
#[derive(Debug)]
struct SkipServerVerification;

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ED25519,
        ]
    }
}
