//! Self-signed TLS certificate generation from Ed25519 device identity.
//!
//! Quinn requires rustls, which requires X.509 certificates.
//! We generate ephemeral self-signed certs from the device's Ed25519 key.
//! Peer authentication is done via certificate fingerprint pinning
//! exchanged during the BLE handshake, NOT via a CA chain.

use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::sync::Arc;
use tracing::debug;

use crate::TransportError;

/// Generate a rustls `ServerConfig` using a self-signed certificate
/// derived from the device's Ed25519 PKCS#8 key material.
///
/// Note: Quinn QUIC uses rustls under the hood. Since we don't have a CA,
/// we use self-signed certs and verify peers via fingerprint pinning
/// established during the BLE handshake phase.
pub fn server_config(
    pkcs8_der: &[u8],
    cert_der: &[u8],
) -> Result<quinn::ServerConfig, TransportError> {
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(pkcs8_der.to_vec()));
    let cert = CertificateDer::from(cert_der.to_vec());

    let mut tls_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .map_err(|e| TransportError::Tls(format!("server config: {e}")))?;

    tls_config.alpn_protocols = vec![b"justdrop/1".to_vec()];

    let server_config = quinn::ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(tls_config)
            .map_err(|e| TransportError::Tls(format!("quic server config: {e}")))?,
    ));

    debug!("QUIC server config created");
    Ok(server_config)
}

/// Generate a rustls `ClientConfig` that skips certificate verification.
///
/// Security is NOT delegated to TLS CA verification. Instead, we verify
/// the peer's identity via the Ed25519 public key fingerprint exchanged
/// during the BLE handshake. The TLS layer provides transport encryption
/// only; authentication is handled at the application layer.
pub fn client_config() -> Result<quinn::ClientConfig, TransportError> {
    let mut tls_config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
        .with_no_client_auth();

    tls_config.alpn_protocols = vec![b"justdrop/1".to_vec()];

    let client_config = quinn::ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(tls_config)
            .map_err(|e| TransportError::Tls(format!("quic client config: {e}")))?,
    ));

    debug!("QUIC client config created");
    Ok(client_config)
}

/// Certificate verifier that accepts any certificate.
///
/// This is intentional: we do NOT trust the TLS certificate chain.
/// Peer identity is verified via Ed25519 public key fingerprinting
/// at the application protocol layer (BLE handshake).
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
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA256,
        ]
    }
}
