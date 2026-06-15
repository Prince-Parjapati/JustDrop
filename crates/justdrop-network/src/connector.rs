//! TCP connector with timeout, retry, and socket tuning.

use justdrop_core::config::NetworkConfig;
use justdrop_core::error::NetworkError;
use socket2::{Domain, Protocol, Socket, Type};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpStream;
use tracing::{debug, info, warn};

/// Connect to a peer with socket tuning and timeout.
pub async fn connect(
    addr: SocketAddr,
    config: &NetworkConfig,
) -> Result<TcpStream, NetworkError> {
    let timeout = Duration::from_secs(config.connect_timeout_secs);

    debug!(peer = %addr, timeout_secs = config.connect_timeout_secs, "connecting to peer");

    let stream = tokio::time::timeout(timeout, TcpStream::connect(addr))
        .await
        .map_err(|_| NetworkError::Timeout {
            addr: addr.to_string(),
        })?
        .map_err(|e| NetworkError::ConnectionFailed {
            addr: addr.to_string(),
            source: e,
        })?;

    // Apply socket tuning
    stream
        .set_nodelay(config.tcp_nodelay)
        .map_err(NetworkError::Io)?;

    info!(peer = %addr, "connected to peer");
    Ok(stream)
}

/// Connect with retry logic.
pub async fn connect_with_retry(
    addr: SocketAddr,
    config: &NetworkConfig,
    max_retries: u32,
    retry_delay: Duration,
) -> Result<TcpStream, NetworkError> {
    let mut last_err = None;

    for attempt in 0..=max_retries {
        if attempt > 0 {
            warn!(
                attempt = attempt,
                max_retries = max_retries,
                delay_ms = retry_delay.as_millis(),
                "retrying connection"
            );
            tokio::time::sleep(retry_delay).await;
        }

        match connect(addr, config).await {
            Ok(stream) => return Ok(stream),
            Err(e) => {
                warn!(attempt = attempt, error = %e, "connection attempt failed");
                last_err = Some(e);
            }
        }
    }

    Err(last_err.unwrap_or(NetworkError::Timeout {
        addr: addr.to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn connect_to_nonexistent_host_fails() {
        let config = NetworkConfig {
            connect_timeout_secs: 1,
            ..Default::default()
        };
        let addr: SocketAddr = "127.0.0.1:1".parse().unwrap();
        let result = connect(addr, &config).await;
        assert!(result.is_err());
    }
}
