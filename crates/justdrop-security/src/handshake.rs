//! Noise_XX handshake for mutual authentication and key agreement.
//!
//! Implements the initiator and responder sides of the Noise_XX_25519_ChaChaPoly_BLAKE2s
//! handshake pattern, producing an encrypted transport session.

use crate::keys::IdentityKeys;
use crate::session::NoiseSession;
use justdrop_core::error::SecurityError;
use snow::{Builder, HandshakeState};
use tracing::{debug, info};

/// Noise protocol pattern string.
const NOISE_PARAMS: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

/// Maximum handshake message size (Noise spec: 65535 bytes, but handshake messages are small).
const MAX_HANDSHAKE_MSG: usize = 65535;

/// Initiator side of the Noise_XX handshake (the one connecting).
pub struct NoiseInitiator {
    state: HandshakeState,
}

impl NoiseInitiator {
    /// Create a new handshake initiator.
    pub fn new(identity: &IdentityKeys) -> Result<Self, SecurityError> {
        let params: snow::params::NoiseParams = NOISE_PARAMS
            .parse()
            .map_err(|e| SecurityError::HandshakeFailed(format!("invalid params: {e}")))?;

        let builder = Builder::new(params);
        let state = builder
            .local_private_key(identity.private_key())
            .map_err(|e| SecurityError::HandshakeFailed(format!("builder key: {e}")))?
            .build_initiator()
            .map_err(|e| SecurityError::HandshakeFailed(format!("builder failed: {e}")))?;

        Ok(Self { state })
    }

    /// Execute the 3-message Noise_XX handshake.
    ///
    /// Returns the encrypted session and the remote peer's static public key.
    ///
    /// Message flow:
    ///   → e                    (initiator sends ephemeral)
    ///   ← e, ee, s, es        (responder sends ephemeral + static)
    ///   → s, se [+ payload]   (initiator sends static + optional payload)
    pub async fn handshake<S, R>(
        mut self,
        mut send: S,
        mut recv: R,
    ) -> Result<(NoiseSession, Vec<u8>), SecurityError>
    where
        S: FnMut(&[u8]) -> Result<(), SecurityError> + Send,
        R: FnMut() -> Result<Vec<u8>, SecurityError> + Send,
    {
        let mut buf = vec![0u8; MAX_HANDSHAKE_MSG];

        // Message 1: → e
        let len = self
            .state
            .write_message(&[], &mut buf)
            .map_err(|e| SecurityError::HandshakeFailed(format!("msg1 write: {e}")))?;
        debug!(len, "handshake msg1 sent (→ e)");
        send(&buf[..len])?;

        // Message 2: ← e, ee, s, es
        let msg2 = recv()?;
        let len = self
            .state
            .read_message(&msg2, &mut buf)
            .map_err(|e| SecurityError::HandshakeFailed(format!("msg2 read: {e}")))?;
        debug!(len, "handshake msg2 received (← e, ee, s, es)");

        // Message 3: → s, se (can include initial payload)
        let len = self
            .state
            .write_message(&[], &mut buf)
            .map_err(|e| SecurityError::HandshakeFailed(format!("msg3 write: {e}")))?;
        debug!(len, "handshake msg3 sent (→ s, se)");
        send(&buf[..len])?;

        // Extract remote static key
        let remote_static = self
            .state
            .get_remote_static()
            .ok_or(SecurityError::PeerVerificationFailed)?
            .to_vec();

        // Transition to transport mode
        let transport = self
            .state
            .into_transport_mode()
            .map_err(|e| SecurityError::HandshakeFailed(format!("transport mode: {e}")))?;

        info!("noise handshake complete (initiator)");
        Ok((NoiseSession::new(transport), remote_static))
    }
}

/// Responder side of the Noise_XX handshake (the one listening).
pub struct NoiseResponder {
    state: HandshakeState,
}

impl NoiseResponder {
    /// Create a new handshake responder.
    pub fn new(identity: &IdentityKeys) -> Result<Self, SecurityError> {
        let params: snow::params::NoiseParams = NOISE_PARAMS
            .parse()
            .map_err(|e| SecurityError::HandshakeFailed(format!("invalid params: {e}")))?;

        let builder = Builder::new(params);
        let state = builder
            .local_private_key(identity.private_key())
            .map_err(|e| SecurityError::HandshakeFailed(format!("builder key: {e}")))?
            .build_responder()
            .map_err(|e| SecurityError::HandshakeFailed(format!("builder failed: {e}")))?;

        Ok(Self { state })
    }

    /// Execute the 3-message Noise_XX handshake (responder side).
    ///
    /// Returns the encrypted session and the remote peer's static public key.
    pub async fn handshake<S, R>(
        mut self,
        mut send: S,
        mut recv: R,
    ) -> Result<(NoiseSession, Vec<u8>), SecurityError>
    where
        S: FnMut(&[u8]) -> Result<(), SecurityError> + Send,
        R: FnMut() -> Result<Vec<u8>, SecurityError> + Send,
    {
        let mut buf = vec![0u8; MAX_HANDSHAKE_MSG];

        // Message 1: ← e (receive initiator's ephemeral)
        let msg1 = recv()?;
        let len = self
            .state
            .read_message(&msg1, &mut buf)
            .map_err(|e| SecurityError::HandshakeFailed(format!("msg1 read: {e}")))?;
        debug!(len, "handshake msg1 received (← e)");

        // Message 2: → e, ee, s, es
        let len = self
            .state
            .write_message(&[], &mut buf)
            .map_err(|e| SecurityError::HandshakeFailed(format!("msg2 write: {e}")))?;
        debug!(len, "handshake msg2 sent (→ e, ee, s, es)");
        send(&buf[..len])?;

        // Message 3: ← s, se
        let msg3 = recv()?;
        let len = self
            .state
            .read_message(&msg3, &mut buf)
            .map_err(|e| SecurityError::HandshakeFailed(format!("msg3 read: {e}")))?;
        debug!(len, "handshake msg3 received (← s, se)");

        // Extract remote static key
        let remote_static = self
            .state
            .get_remote_static()
            .ok_or(SecurityError::PeerVerificationFailed)?
            .to_vec();

        // Transition to transport mode
        let transport = self
            .state
            .into_transport_mode()
            .map_err(|e| SecurityError::HandshakeFailed(format!("transport mode: {e}")))?;

        info!("noise handshake complete (responder)");
        Ok((NoiseSession::new(transport), remote_static))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    /// In-memory channel for testing handshakes without real I/O.
    struct TestChannel {
        queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    }

    impl TestChannel {
        fn new() -> (Self, Self) {
            let q1 = Arc::new(Mutex::new(VecDeque::new()));
            let q2 = Arc::new(Mutex::new(VecDeque::new()));
            (Self { queue: q1.clone() }, Self { queue: q2.clone() })
        }
    }

    #[tokio::test]
    async fn noise_xx_handshake_roundtrip() {
        let tmp = std::env::temp_dir().join("justdrop_hs_test");
        let _ = std::fs::remove_dir_all(&tmp);

        let dir_i = tmp.join("initiator");
        let dir_r = tmp.join("responder");
        std::fs::create_dir_all(&dir_i).unwrap();
        std::fs::create_dir_all(&dir_r).unwrap();

        let keys_i = IdentityKeys::load_or_generate(&dir_i).unwrap();
        let keys_r = IdentityKeys::load_or_generate(&dir_r).unwrap();

        // Shared queues: initiator→responder and responder→initiator
        let i_to_r: Arc<Mutex<VecDeque<Vec<u8>>>> = Arc::new(Mutex::new(VecDeque::new()));
        let r_to_i: Arc<Mutex<VecDeque<Vec<u8>>>> = Arc::new(Mutex::new(VecDeque::new()));

        let initiator = NoiseInitiator::new(&keys_i).unwrap();
        let responder = NoiseResponder::new(&keys_r).unwrap();

        let i_to_r_clone = i_to_r.clone();
        let r_to_i_clone = r_to_i.clone();

        // Run initiator
        let i_handle = tokio::spawn(async move {
            let send = |data: &[u8]| -> Result<(), SecurityError> {
                i_to_r_clone.lock().unwrap().push_back(data.to_vec());
                Ok(())
            };
            let recv = || -> Result<Vec<u8>, SecurityError> {
                loop {
                    if let Some(msg) = r_to_i_clone.lock().unwrap().pop_front() {
                        return Ok(msg);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            };
            initiator.handshake(send, recv).await
        });

        let i_to_r_clone2 = i_to_r.clone();
        let r_to_i_clone2 = r_to_i.clone();

        // Run responder
        let r_handle = tokio::spawn(async move {
            let send = |data: &[u8]| -> Result<(), SecurityError> {
                r_to_i_clone2.lock().unwrap().push_back(data.to_vec());
                Ok(())
            };
            let recv = || -> Result<Vec<u8>, SecurityError> {
                loop {
                    if let Some(msg) = i_to_r_clone2.lock().unwrap().pop_front() {
                        return Ok(msg);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            };
            responder.handshake(send, recv).await
        });

        let (i_result, r_result) = tokio::join!(i_handle, r_handle);
        let (_i_session, i_remote_key) = i_result.unwrap().unwrap();
        let (_r_session, r_remote_key) = r_result.unwrap().unwrap();

        // Verify mutual authentication
        assert_eq!(i_remote_key, keys_r.public_key());
        assert_eq!(r_remote_key, keys_i.public_key());

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
