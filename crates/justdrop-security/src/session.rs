//! Encrypted transport session wrapping a Noise `TransportState`.
//!
//! Provides encrypt/decrypt operations for framed messages after
//! the Noise_XX handshake completes.

use justdrop_core::error::SecurityError;
use snow::TransportState;
use tracing::trace;

/// Maximum ciphertext expansion: 16 bytes for Poly1305 tag.
const TAG_LEN: usize = 16;

/// An established encrypted session after Noise handshake.
pub struct NoiseSession {
    transport: TransportState,
}

impl NoiseSession {
    /// Wrap a completed Noise `TransportState`.
    pub fn new(transport: TransportState) -> Self {
        Self { transport }
    }

    /// Encrypt a plaintext message, returning the ciphertext.
    ///
    /// The output is `plaintext.len() + 16` bytes (16-byte Poly1305 tag).
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, SecurityError> {
        let mut buf = vec![0u8; plaintext.len() + TAG_LEN];
        let len = self
            .transport
            .write_message(plaintext, &mut buf)
            .map_err(|e| SecurityError::EncryptionFailed(format!("{e}")))?;
        buf.truncate(len);
        trace!(
            plaintext_len = plaintext.len(),
            ciphertext_len = len,
            "encrypted message"
        );
        Ok(buf)
    }

    /// Decrypt a ciphertext message, returning the plaintext.
    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, SecurityError> {
        let mut buf = vec![0u8; ciphertext.len()];
        let len = self
            .transport
            .read_message(ciphertext, &mut buf)
            .map_err(|e| SecurityError::DecryptionFailed(format!("{e}")))?;
        buf.truncate(len);
        trace!(
            ciphertext_len = ciphertext.len(),
            plaintext_len = len,
            "decrypted message"
        );
        Ok(buf)
    }

    /// Check if this is the initiator's session.
    pub fn is_initiator(&self) -> bool {
        self.transport.is_initiator()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handshake::{NoiseInitiator, NoiseResponder};
    use crate::keys::IdentityKeys;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    /// Helper to perform a handshake and return both sessions.
    async fn create_session_pair() -> (NoiseSession, NoiseSession) {
        let tmp = std::env::temp_dir().join("justdrop_session_test");
        let _ = std::fs::remove_dir_all(&tmp);

        let dir_i = tmp.join("initiator");
        let dir_r = tmp.join("responder");
        std::fs::create_dir_all(&dir_i).unwrap();
        std::fs::create_dir_all(&dir_r).unwrap();

        let keys_i = IdentityKeys::load_or_generate(&dir_i).unwrap();
        let keys_r = IdentityKeys::load_or_generate(&dir_r).unwrap();

        let i_to_r: Arc<Mutex<VecDeque<Vec<u8>>>> = Arc::new(Mutex::new(VecDeque::new()));
        let r_to_i: Arc<Mutex<VecDeque<Vec<u8>>>> = Arc::new(Mutex::new(VecDeque::new()));

        let initiator = NoiseInitiator::new(&keys_i).unwrap();
        let responder = NoiseResponder::new(&keys_r).unwrap();

        let i_to_r_c = i_to_r.clone();
        let r_to_i_c = r_to_i.clone();

        let i_handle = tokio::spawn(async move {
            let send = |data: &[u8]| -> Result<(), SecurityError> {
                i_to_r_c.lock().unwrap().push_back(data.to_vec());
                Ok(())
            };
            let recv = || -> Result<Vec<u8>, SecurityError> {
                loop {
                    if let Some(msg) = r_to_i_c.lock().unwrap().pop_front() {
                        return Ok(msg);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            };
            initiator.handshake(send, recv).await
        });

        let i_to_r_c2 = i_to_r.clone();
        let r_to_i_c2 = r_to_i.clone();

        let r_handle = tokio::spawn(async move {
            let send = |data: &[u8]| -> Result<(), SecurityError> {
                r_to_i_c2.lock().unwrap().push_back(data.to_vec());
                Ok(())
            };
            let recv = || -> Result<Vec<u8>, SecurityError> {
                loop {
                    if let Some(msg) = i_to_r_c2.lock().unwrap().pop_front() {
                        return Ok(msg);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            };
            responder.handshake(send, recv).await
        });

        let (i_res, r_res) = tokio::join!(i_handle, r_handle);
        let (i_session, _) = i_res.unwrap().unwrap();
        let (r_session, _) = r_res.unwrap().unwrap();

        let _ = std::fs::remove_dir_all(&tmp);
        (i_session, r_session)
    }

    #[tokio::test]
    async fn encrypt_decrypt_roundtrip() {
        let (mut sender, mut receiver) = create_session_pair().await;

        let plaintext = b"Hello, JustDrop!";
        let ciphertext = sender.encrypt(plaintext).unwrap();

        // Ciphertext should be larger than plaintext (tag added)
        assert!(ciphertext.len() > plaintext.len());

        let decrypted = receiver.decrypt(&ciphertext).unwrap();
        assert_eq!(&decrypted, plaintext);
    }

    #[tokio::test]
    async fn bidirectional_communication() {
        let (mut alice, mut bob) = create_session_pair().await;

        // Alice → Bob
        let ct1 = alice.encrypt(b"msg from alice").unwrap();
        let pt1 = bob.decrypt(&ct1).unwrap();
        assert_eq!(pt1, b"msg from alice");

        // Bob → Alice
        let ct2 = bob.encrypt(b"msg from bob").unwrap();
        let pt2 = alice.decrypt(&ct2).unwrap();
        assert_eq!(pt2, b"msg from bob");
    }

    #[tokio::test]
    async fn tampered_ciphertext_fails() {
        let (mut sender, mut receiver) = create_session_pair().await;

        let mut ciphertext = sender.encrypt(b"secret data").unwrap();
        // Flip a bit
        if let Some(byte) = ciphertext.last_mut() {
            *byte ^= 0xFF;
        }

        assert!(receiver.decrypt(&ciphertext).is_err());
    }
}
