//! Encrypted transport layer wrapping TCP + Noise session.
//!
//! Provides framed read/write of length-prefixed encrypted messages
//! over a TCP stream using the established Noise session.

use bytes::{Buf, BufMut, BytesMut};
use rustdrop_core::error::{NetworkError, SecurityError};
use rustdrop_core::types::MAX_MESSAGE_SIZE;
use rustdrop_security::NoiseSession;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tracing::{debug, trace, warn};

/// Frame header size: 4 bytes for length.
const FRAME_HEADER_SIZE: usize = 4;

/// Encrypted transport over TCP with Noise session.
///
/// Thread-safe: uses Arc<Mutex<>> internally for split read/write access.
pub struct SecureTransport {
    stream: TcpStream,
    session: Arc<Mutex<NoiseSession>>,
    read_buf: BytesMut,
}

impl SecureTransport {
    /// Create a new secure transport from a TCP stream and completed Noise session.
    pub fn new(stream: TcpStream, session: NoiseSession) -> Self {
        Self {
            stream,
            session: Arc::new(Mutex::new(session)),
            read_buf: BytesMut::with_capacity(64 * 1024),
        }
    }

    /// Send a plaintext message (will be encrypted and framed).
    pub async fn send(&mut self, plaintext: &[u8]) -> Result<(), NetworkError> {
        let ciphertext = {
            let mut session = self.session.lock().await;
            session
                .encrypt(plaintext)
                .map_err(|e| NetworkError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
        };

        let frame_len = ciphertext.len() as u32;
        let mut frame = Vec::with_capacity(FRAME_HEADER_SIZE + ciphertext.len());
        frame.extend_from_slice(&frame_len.to_be_bytes());
        frame.extend_from_slice(&ciphertext);

        self.stream
            .write_all(&frame)
            .await
            .map_err(NetworkError::Io)?;

        trace!(plaintext_len = plaintext.len(), frame_len = frame.len(), "sent encrypted frame");
        Ok(())
    }

    /// Receive and decrypt the next framed message.
    ///
    /// Returns `None` if the connection was cleanly closed.
    pub async fn recv(&mut self) -> Result<Option<Vec<u8>>, NetworkError> {
        // Read frame header (4 bytes length)
        while self.read_buf.len() < FRAME_HEADER_SIZE {
            let n = self
                .stream
                .read_buf(&mut self.read_buf)
                .await
                .map_err(NetworkError::Io)?;
            if n == 0 {
                if self.read_buf.is_empty() {
                    return Ok(None); // Clean close
                }
                return Err(NetworkError::ConnectionReset);
            }
        }

        let frame_len = u32::from_be_bytes([
            self.read_buf[0],
            self.read_buf[1],
            self.read_buf[2],
            self.read_buf[3],
        ]) as usize;

        if frame_len > MAX_MESSAGE_SIZE {
            return Err(NetworkError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("frame too large: {frame_len} > {MAX_MESSAGE_SIZE}"),
            )));
        }

        // Read full frame body
        let total_needed = FRAME_HEADER_SIZE + frame_len;
        while self.read_buf.len() < total_needed {
            let n = self
                .stream
                .read_buf(&mut self.read_buf)
                .await
                .map_err(NetworkError::Io)?;
            if n == 0 {
                return Err(NetworkError::ConnectionReset);
            }
        }

        // Extract the frame
        self.read_buf.advance(FRAME_HEADER_SIZE);
        let ciphertext = self.read_buf.split_to(frame_len);

        // Decrypt
        let plaintext = {
            let mut session = self.session.lock().await;
            session
                .decrypt(&ciphertext)
                .map_err(|e| NetworkError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
        };

        trace!(ciphertext_len = frame_len, plaintext_len = plaintext.len(), "received encrypted frame");
        Ok(Some(plaintext))
    }

    /// Send raw bytes without encryption (for handshake messages).
    pub async fn send_raw(&mut self, data: &[u8]) -> Result<(), NetworkError> {
        let len = data.len() as u32;
        self.stream
            .write_all(&len.to_be_bytes())
            .await
            .map_err(NetworkError::Io)?;
        self.stream
            .write_all(data)
            .await
            .map_err(NetworkError::Io)?;
        Ok(())
    }

    /// Receive raw bytes without decryption (for handshake messages).
    pub async fn recv_raw(&mut self) -> Result<Vec<u8>, NetworkError> {
        let mut len_buf = [0u8; 4];
        self.stream
            .read_exact(&mut len_buf)
            .await
            .map_err(NetworkError::Io)?;
        let len = u32::from_be_bytes(len_buf) as usize;

        if len > MAX_MESSAGE_SIZE {
            return Err(NetworkError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("raw frame too large: {len}"),
            )));
        }

        let mut buf = vec![0u8; len];
        self.stream
            .read_exact(&mut buf)
            .await
            .map_err(NetworkError::Io)?;
        Ok(buf)
    }

    /// Flush the underlying stream.
    pub async fn flush(&mut self) -> Result<(), NetworkError> {
        self.stream.flush().await.map_err(NetworkError::Io)
    }

    /// Shut down the transport.
    pub async fn shutdown(&mut self) -> Result<(), NetworkError> {
        self.stream.shutdown().await.map_err(NetworkError::Io)
    }

    /// Get the underlying TCP stream reference (for sendfile operations).
    pub fn stream(&self) -> &TcpStream {
        &self.stream
    }

    /// Get mutable reference to the underlying TCP stream.
    pub fn stream_mut(&mut self) -> &mut TcpStream {
        &mut self.stream
    }

    /// Get a clone of the session Arc for shared access.
    pub fn session(&self) -> Arc<Mutex<NoiseSession>> {
        Arc::clone(&self.session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn raw_send_recv_roundtrip() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            // Use a dummy session - we test raw operations here
            let mut len_buf = [0u8; 4];
            let mut reader = tokio::io::BufReader::new(stream);
            tokio::io::AsyncReadExt::read_exact(&mut reader, &mut len_buf).await.unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut buf = vec![0u8; len];
            tokio::io::AsyncReadExt::read_exact(&mut reader, &mut buf).await.unwrap();
            buf
        });

        let client_stream = TcpStream::connect(addr).await.unwrap();
        // Create a dummy transport just for raw send
        let session = create_dummy_noise_session();
        let mut transport = SecureTransport::new(client_stream, session);

        transport.send_raw(b"hello raw").await.unwrap();

        let received = server.await.unwrap();
        assert_eq!(received, b"hello raw");
    }

    /// Create a Noise session for testing (initiator side of a self-handshake).
    fn create_dummy_noise_session() -> NoiseSession {
        // We can't easily create a NoiseSession without a handshake,
        // so we'll test the raw path which doesn't use the session.
        // For full integration tests, see rustdrop-protocol.
        
        // Build a quick handshake pair
        let params: snow::params::NoiseParams = "Noise_XX_25519_ChaChaPoly_BLAKE2s".parse().unwrap();
        let builder_i = snow::Builder::new(params.clone());
        let keypair_i = builder_i.generate_keypair().unwrap();
        let builder_r = snow::Builder::new(params.clone());
        let keypair_r = builder_r.generate_keypair().unwrap();

        let mut initiator = snow::Builder::new(params.clone())
            .local_private_key(&keypair_i.private)
            .unwrap()
            .build_initiator()
            .unwrap();
        let mut responder = snow::Builder::new(params)
            .local_private_key(&keypair_r.private)
            .unwrap()
            .build_responder()
            .unwrap();

        let mut buf = vec![0u8; 65535];

        // msg1: → e
        let len = initiator.write_message(&[], &mut buf).unwrap();
        let msg1 = buf[..len].to_vec();

        // msg2: ← e, ee, s, es
        responder.read_message(&msg1, &mut buf).unwrap();
        let len = responder.write_message(&[], &mut buf).unwrap();
        let msg2 = buf[..len].to_vec();

        // msg3: → s, se
        initiator.read_message(&msg2, &mut buf).unwrap();
        let len = initiator.write_message(&[], &mut buf).unwrap();
        let msg3 = buf[..len].to_vec();
        responder.read_message(&msg3, &mut buf).unwrap();

        let transport = initiator.into_transport_mode().unwrap();
        NoiseSession::new(transport)
    }
}
