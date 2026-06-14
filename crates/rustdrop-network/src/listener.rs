//! TCP listener with socket2 tuning for high-throughput transfers.
//!
//! Binds a TCP listener with optimized socket options and spawns
//! per-connection handlers for incoming transfer requests.

use rustdrop_core::config::NetworkConfig;
use rustdrop_core::error::NetworkError;
use socket2::{Domain, Protocol, Socket, Type};
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info, warn};

/// A tuned TCP listener for receiving incoming connections.
pub struct TransferListener {
    listener: TcpListener,
    local_addr: SocketAddr,
}

impl TransferListener {
    /// Bind and start listening on the configured port with socket2 tuning.
    pub async fn bind(config: &NetworkConfig) -> Result<Self, NetworkError> {
        let addr: SocketAddr = format!("0.0.0.0:{}", config.listen_port)
            .parse()
            .map_err(|e| NetworkError::BindFailed {
                port: config.listen_port,
                source: std::io::Error::new(std::io::ErrorKind::InvalidInput, e),
            })?;

        // Create socket with socket2 for fine-grained tuning
        let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)).map_err(
            |e| NetworkError::BindFailed {
                port: config.listen_port,
                source: e,
            },
        )?;

        // Socket options for performance
        socket.set_reuse_address(true).map_err(|e| {
            NetworkError::BindFailed {
                port: config.listen_port,
                source: e,
            }
        })?;

        socket.set_nodelay(config.tcp_nodelay).map_err(|e| {
            NetworkError::BindFailed {
                port: config.listen_port,
                source: e,
            }
        })?;

        socket
            .set_send_buffer_size(config.send_buffer_size)
            .map_err(|e| NetworkError::BindFailed {
                port: config.listen_port,
                source: e,
            })?;

        socket
            .set_recv_buffer_size(config.recv_buffer_size)
            .map_err(|e| NetworkError::BindFailed {
                port: config.listen_port,
                source: e,
            })?;

        // Bind and listen
        socket
            .bind(&addr.into())
            .map_err(|e| NetworkError::BindFailed {
                port: config.listen_port,
                source: e,
            })?;

        socket.listen(128).map_err(|e| NetworkError::BindFailed {
            port: config.listen_port,
            source: e,
        })?;

        socket.set_nonblocking(true).map_err(|e| {
            NetworkError::BindFailed {
                port: config.listen_port,
                source: e,
            }
        })?;

        let std_listener: std::net::TcpListener = socket.into();
        let listener = TcpListener::from_std(std_listener).map_err(|e| {
            NetworkError::BindFailed {
                port: config.listen_port,
                source: e,
            }
        })?;

        let local_addr = listener.local_addr().map_err(|e| NetworkError::BindFailed {
            port: config.listen_port,
            source: e,
        })?;

        info!(addr = %local_addr, "TCP listener bound");

        Ok(Self {
            listener,
            local_addr,
        })
    }

    /// Accept the next incoming connection.
    pub async fn accept(&self) -> Result<(TcpStream, SocketAddr), NetworkError> {
        let (stream, addr) = self.listener.accept().await.map_err(NetworkError::Io)?;
        debug!(peer = %addr, "accepted incoming connection");
        Ok((stream, addr))
    }

    /// Get the local address this listener is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Get the listening port.
    pub fn port(&self) -> u16 {
        self.local_addr.port()
    }
}

/// Apply performance tuning to an accepted TCP stream.
pub fn tune_stream(stream: &TcpStream, config: &NetworkConfig) -> Result<(), NetworkError> {
    stream
        .set_nodelay(config.tcp_nodelay)
        .map_err(NetworkError::Io)?;
    debug!("tuned accepted TCP stream");
    Ok(())
}
