//! # RustDrop Discovery
//!
//! mDNS-based peer discovery for the local network. Handles both service
//! registration (advertising this device) and browsing (finding peers).

pub mod browser;
pub mod registrar;

pub use browser::{PeerEvent, ServiceBrowser};
pub use registrar::ServiceRegistrar;
