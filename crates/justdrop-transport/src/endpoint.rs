//! QUIC endpoint management.
//!
//! Wraps Quinn's `Endpoint` with JustDrop-specific configuration
//! for both server (listening) and client (connecting) roles.

use crate::tls;
use crate::{StreamRecv, StreamSend, TransportError};
use quinn::{Connection, Endpoint};
use std::net::SocketAddr;
use tracing::{debug, info};

/// QUIC connection wrapper implementing the JustDrop transport interface.
pub struct QuicConnection {
    conn: Connection,
}

impl QuicConnection {
    /// Wrap an established Quinn connection.
    pub fn new(conn: Connection) -> Self {
        Self { conn }
    }

    /// Open a new bidirectional stream.
    pub async fn open_stream(&self) -> Result<(StreamSend, StreamRecv), TransportError> {
        let (send, recv) = self
            .conn
            .open_bi()
            .await
            .map_err(|e| TransportError::Connection(format!("open_bi: {e}")))?;
        Ok((StreamSend::new(send), StreamRecv::new(recv)))
    }

    /// Accept an incoming bidirectional stream.
    pub async fn accept_stream(&self) -> Result<(StreamSend, StreamRecv), TransportError> {
        let (send, recv) = self
            .conn
            .accept_bi()
            .await
            .map_err(|e| TransportError::Connection(format!("accept_bi: {e}")))?;
        Ok((StreamSend::new(send), StreamRecv::new(recv)))
    }

    /// Remote address.
    pub fn remote_addr(&self) -> SocketAddr {
        self.conn.remote_address()
    }

    /// Close gracefully.
    pub fn close(&self, reason: &str) {
        self.conn.close(0u32.into(), reason.as_bytes());
    }
}

/// Create a QUIC server endpoint.
///
/// Binds to the given address and listens for incoming connections.
pub async fn create_server(
    bind_addr: SocketAddr,
    pkcs8_der: &[u8],
    cert_der: &[u8],
) -> Result<Endpoint, TransportError> {
    let server_config = tls::server_config(pkcs8_der, cert_der)?;

    let endpoint = Endpoint::server(server_config, bind_addr)
        .map_err(|e| TransportError::Bind(format!("server bind {bind_addr}: {e}")))?;

    info!(addr = %bind_addr, "QUIC server listening");
    Ok(endpoint)
}

/// Create a QUIC client endpoint bound to a random port.
pub fn create_client() -> Result<Endpoint, TransportError> {
    let bind_addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
    let mut endpoint = Endpoint::client(bind_addr)
        .map_err(|e| TransportError::Bind(format!("client bind: {e}")))?;

    let client_config = tls::client_config()?;
    endpoint.set_default_client_config(client_config);

    debug!("QUIC client endpoint created");
    Ok(endpoint)
}

/// Connect to a QUIC server.
pub async fn connect(
    endpoint: &Endpoint,
    addr: SocketAddr,
) -> Result<QuicConnection, TransportError> {
    let conn = endpoint
        .connect(addr, "justdrop")
        .map_err(|e| TransportError::Connection(format!("connect config: {e}")))?
        .await
        .map_err(|e| TransportError::Connection(format!("connect {addr}: {e}")))?;

    info!(remote = %addr, "QUIC connection established");
    Ok(QuicConnection::new(conn))
}
