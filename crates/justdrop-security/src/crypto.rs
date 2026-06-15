//! Cryptographic session management using ring.
//!
//! Replaces the snow-based Noise protocol with direct ring primitives:
//! - Ed25519 for device identity signing/verification
//! - X25519 ephemeral DH for session key establishment
//! - HKDF-SHA256 for key derivation
//! - ChaCha20-Poly1305 AEAD for symmetric encryption
//!
//! Every transfer session gets a unique session key derived from
//! an ephemeral X25519 Diffie-Hellman exchange.

use ring::aead::{self, Nonce, NONCE_LEN};
use ring::agreement::{self, EphemeralPrivateKey, UnparsedPublicKey};
use ring::hkdf;
use ring::rand::SystemRandom;
use ring::signature::{self};
use tracing::debug;

/// Errors from the crypto layer.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("key generation failed")]
    KeyGeneration,
    #[error("key exchange failed")]
    KeyExchange,
    #[error("signature verification failed")]
    VerificationFailed,
    #[error("encryption failed")]
    EncryptionFailed,
    #[error("decryption failed")]
    DecryptionFailed,
    #[error("invalid key material")]
    InvalidKey,
}

/// Result of an X25519 key exchange — produces an ephemeral keypair
/// whose public half is sent to the peer over BLE GATT.
pub struct KeyExchangeInitiator {
    private_key: EphemeralPrivateKey,
    pub public_key_bytes: Vec<u8>,
}

impl KeyExchangeInitiator {
    /// Generate a new ephemeral X25519 keypair.
    pub fn new() -> Result<Self, CryptoError> {
        let rng = SystemRandom::new();
        let private_key = EphemeralPrivateKey::generate(&agreement::X25519, &rng)
            .map_err(|_| CryptoError::KeyGeneration)?;

        let public_key_bytes = private_key
            .compute_public_key()
            .map_err(|_| CryptoError::KeyGeneration)?
            .as_ref()
            .to_vec();

        Ok(Self {
            private_key,
            public_key_bytes,
        })
    }

    /// Complete the key exchange with the peer's public key.
    /// Returns a `SessionKeys` containing derived encryption/decryption keys.
    pub fn complete(
        self,
        peer_public_key: &[u8],
        is_initiator: bool,
    ) -> Result<SessionKeys, CryptoError> {
        let peer_key = UnparsedPublicKey::new(&agreement::X25519, peer_public_key);

        let shared_secret =
            agreement::agree_ephemeral(self.private_key, &peer_key, |secret| secret.to_vec())
                .map_err(|_| CryptoError::KeyExchange)?;

        debug!("X25519 key exchange completed");
        SessionKeys::derive(&shared_secret, is_initiator)
    }
}

/// Derived session keys for bidirectional encrypted communication.
///
/// Two separate keys are derived from the shared secret:
/// one for each direction, preventing nonce reuse across directions.
pub struct SessionKeys {
    /// Key for encrypting outgoing data.
    pub encrypt_key: Vec<u8>,
    /// Key for decrypting incoming data.
    pub decrypt_key: Vec<u8>,
}

impl SessionKeys {
    /// Derive directional keys from the DH shared secret using HKDF.
    fn derive(shared_secret: &[u8], is_initiator: bool) -> Result<Self, CryptoError> {
        let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, b"justdrop-session-v1");
        let prk = salt.extract(shared_secret);

        let mut initiator_key = vec![0u8; 32];
        let mut responder_key = vec![0u8; 32];

        prk.expand(&[b"initiator-key"], HkdfLen(32))
            .and_then(|okm| okm.fill(&mut initiator_key))
            .map_err(|_| CryptoError::KeyExchange)?;

        prk.expand(&[b"responder-key"], HkdfLen(32))
            .and_then(|okm| okm.fill(&mut responder_key))
            .map_err(|_| CryptoError::KeyExchange)?;

        let (encrypt_key, decrypt_key) = if is_initiator {
            (initiator_key, responder_key)
        } else {
            (responder_key, initiator_key)
        };

        Ok(Self {
            encrypt_key,
            decrypt_key,
        })
    }

    /// Create an encryptor from the encrypt key.
    pub fn encryptor(&self) -> Result<SessionCipher, CryptoError> {
        SessionCipher::new(&self.encrypt_key)
    }

    /// Create a decryptor from the decrypt key.
    pub fn decryptor(&self) -> Result<SessionCipher, CryptoError> {
        SessionCipher::new(&self.decrypt_key)
    }
}

/// ChaCha20-Poly1305 AEAD cipher with auto-incrementing nonce.
pub struct SessionCipher {
    key: aead::LessSafeKey,
    nonce_counter: u64,
}

impl SessionCipher {
    fn new(key_bytes: &[u8]) -> Result<Self, CryptoError> {
        let unbound_key = aead::UnboundKey::new(&aead::CHACHA20_POLY1305, key_bytes)
            .map_err(|_| CryptoError::InvalidKey)?;
        Ok(Self {
            key: aead::LessSafeKey::new(unbound_key),
            nonce_counter: 0,
        })
    }

    /// Encrypt data in-place, appending the authentication tag.
    /// Returns the nonce used (for the receiver to use the same nonce).
    pub fn encrypt(&mut self, plaintext: &mut Vec<u8>) -> Result<[u8; NONCE_LEN], CryptoError> {
        let nonce_bytes = self.next_nonce();
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);

        self.key
            .seal_in_place_append_tag(nonce, aead::Aad::empty(), plaintext)
            .map_err(|_| CryptoError::EncryptionFailed)?;

        Ok(nonce_bytes)
    }

    /// Decrypt data in-place using the provided nonce.
    pub fn decrypt(
        &self,
        nonce_bytes: &[u8; NONCE_LEN],
        ciphertext: &mut Vec<u8>,
    ) -> Result<(), CryptoError> {
        let nonce = Nonce::assume_unique_for_key(*nonce_bytes);

        let plaintext_len = self
            .key
            .open_in_place(nonce, aead::Aad::empty(), ciphertext)
            .map_err(|_| CryptoError::DecryptionFailed)?
            .len();

        ciphertext.truncate(plaintext_len);
        Ok(())
    }

    fn next_nonce(&mut self) -> [u8; NONCE_LEN] {
        let counter = self.nonce_counter;
        self.nonce_counter += 1;
        let mut nonce = [0u8; NONCE_LEN];
        nonce[..8].copy_from_slice(&counter.to_le_bytes());
        nonce
    }
}

/// Verify an Ed25519 signature from a peer's public key.
pub fn verify_signature(
    public_key: &[u8],
    message: &[u8],
    signature_bytes: &[u8],
) -> Result<(), CryptoError> {
    let peer_key = signature::UnparsedPublicKey::new(&signature::ED25519, public_key);
    peer_key
        .verify(message, signature_bytes)
        .map_err(|_| CryptoError::VerificationFailed)
}

/// HKDF output length helper.
struct HkdfLen(usize);

impl hkdf::KeyType for HkdfLen {
    fn len(&self) -> usize {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    #[test]
    fn key_exchange_produces_matching_keys() {
        let initiator = KeyExchangeInitiator::new().unwrap();
        let responder = KeyExchangeInitiator::new().unwrap();

        let init_pub = initiator.public_key_bytes.clone();
        let resp_pub = responder.public_key_bytes.clone();

        let init_keys = initiator.complete(&resp_pub, true).unwrap();
        let resp_keys = responder.complete(&init_pub, false).unwrap();

        // Initiator's encrypt key == Responder's decrypt key
        assert_eq!(init_keys.encrypt_key, resp_keys.decrypt_key);
        // Responder's encrypt key == Initiator's decrypt key
        assert_eq!(resp_keys.encrypt_key, init_keys.decrypt_key);
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let initiator = KeyExchangeInitiator::new().unwrap();
        let responder = KeyExchangeInitiator::new().unwrap();

        let init_pub = initiator.public_key_bytes.clone();
        let resp_pub = responder.public_key_bytes.clone();

        let init_keys = initiator.complete(&resp_pub, true).unwrap();
        let resp_keys = responder.complete(&init_pub, false).unwrap();

        let mut encryptor = init_keys.encryptor().unwrap();
        let decryptor = resp_keys.decryptor().unwrap();

        let original = b"hello justdrop secure transfer".to_vec();
        let mut data = original.clone();
        let nonce = encryptor.encrypt(&mut data).unwrap();

        // Data is now encrypted + has auth tag appended
        assert_ne!(data[..original.len()], original[..]);

        decryptor.decrypt(&nonce, &mut data).unwrap();
        assert_eq!(data, original);
    }

    #[test]
    fn tampered_data_fails_decryption() {
        let initiator = KeyExchangeInitiator::new().unwrap();
        let responder = KeyExchangeInitiator::new().unwrap();

        let init_pub = initiator.public_key_bytes.clone();
        let resp_pub = responder.public_key_bytes.clone();

        let init_keys = initiator.complete(&resp_pub, true).unwrap();
        let resp_keys = responder.complete(&init_pub, false).unwrap();

        let mut encryptor = init_keys.encryptor().unwrap();
        let decryptor = resp_keys.decryptor().unwrap();

        let mut data = b"secret data".to_vec();
        let nonce = encryptor.encrypt(&mut data).unwrap();

        // Tamper with ciphertext
        data[0] ^= 0xFF;

        let result = decryptor.decrypt(&nonce, &mut data);
        assert!(result.is_err());
    }

    #[test]
    fn wrong_nonce_fails() {
        let initiator = KeyExchangeInitiator::new().unwrap();
        let responder = KeyExchangeInitiator::new().unwrap();

        let init_pub = initiator.public_key_bytes.clone();
        let resp_pub = responder.public_key_bytes.clone();

        let init_keys = initiator.complete(&resp_pub, true).unwrap();
        let resp_keys = responder.complete(&init_pub, false).unwrap();

        let mut encryptor = init_keys.encryptor().unwrap();
        let decryptor = resp_keys.decryptor().unwrap();

        let mut data = b"test".to_vec();
        let _nonce = encryptor.encrypt(&mut data).unwrap();

        let wrong_nonce = [0xFF; NONCE_LEN];
        let result = decryptor.decrypt(&wrong_nonce, &mut data);
        assert!(result.is_err());
    }

    #[test]
    fn ed25519_signature_verification() {
        let rng = SystemRandom::new();
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let keypair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();

        let message = b"justdrop handshake payload";
        let sig = keypair.sign(message);

        verify_signature(keypair.public_key().as_ref(), message, sig.as_ref()).unwrap();
    }

    #[test]
    fn wrong_signature_rejected() {
        let rng = SystemRandom::new();
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let keypair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();

        let message = b"original message";
        let sig = keypair.sign(message);

        let result = verify_signature(
            keypair.public_key().as_ref(),
            b"different message",
            sig.as_ref(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn multiple_encryptions_use_different_nonces() {
        let initiator = KeyExchangeInitiator::new().unwrap();
        let responder = KeyExchangeInitiator::new().unwrap();

        let init_keys = initiator
            .complete(&responder.public_key_bytes, true)
            .unwrap();

        let mut encryptor = init_keys.encryptor().unwrap();

        let mut data1 = b"msg1".to_vec();
        let nonce1 = encryptor.encrypt(&mut data1).unwrap();

        let mut data2 = b"msg2".to_vec();
        let nonce2 = encryptor.encrypt(&mut data2).unwrap();

        assert_ne!(nonce1, nonce2);
    }
}
