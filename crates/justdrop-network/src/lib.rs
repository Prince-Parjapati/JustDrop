//! # JustDrop Network
//!
//! TCP transport layer with socket2 tuning, encrypted framing via Noise session,
//! and zero-copy sendfile optimization for high-throughput transfers.

pub mod connector;
pub mod listener;
pub mod sendfile;
pub mod transport;

pub use connector::{connect, connect_with_retry};
pub use listener::TransferListener;
pub use transport::SecureTransport;
