//! C ABI exports for platform integration.
//!
//! All functions use C-compatible types and follow a consistent pattern:
//! - Return `0` for success, negative for error
//! - Use opaque pointers for state
//! - String parameters are null-terminated C strings
//! - Callbacks use function pointers with `void*` context

#![allow(clippy::not_unsafe_ptr_arg_deref)]

pub mod android;
pub mod macos;

use parking_lot::Mutex;
use justdrop_core::config::Config;
use justdrop_core::types::DeviceInfo;
use justdrop_discovery::{PeerEvent, ServiceBrowser, ServiceRegistrar};
use justdrop_network::TransferListener;
use justdrop_protocol::{
    IncomingTransferDecision, RecvTransfer, SendTransfer, TransferEvent,
};
use justdrop_security::IdentityKeys;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tracing::{error, info};

/// Opaque handle to the JustDrop engine.
pub struct JustDropEngine {
    runtime: Runtime,
    config: Config,
    identity: Arc<IdentityKeys>,
    registrar: Option<ServiceRegistrar>,
    browser: Option<ServiceBrowser>,
    event_tx: mpsc::Sender<TransferEvent>,
    event_rx: Mutex<Option<mpsc::Receiver<TransferEvent>>>,
}

/// Global engine instance (singleton).
static ENGINE: Mutex<Option<Box<JustDropEngine>>> = Mutex::new(None);

/// Progress callback function pointer.
pub type ProgressCallback = extern "C" fn(
    ctx: *mut c_void,
    transfer_id: *const c_char,
    bytes_transferred: u64,
    total_bytes: u64,
    speed_bps: u64,
);

/// Peer discovered callback.
pub type PeerCallback = extern "C" fn(
    ctx: *mut c_void,
    peer_id: *const c_char,
    peer_name: *const c_char,
    peer_addr: *const c_char,
    platform: *const c_char,
);

/// Incoming transfer request callback. Return 1 to accept, 0 to reject.
pub type TransferRequestCallback = extern "C" fn(
    ctx: *mut c_void,
    transfer_id: *const c_char,
    sender_name: *const c_char,
    file_count: u32,
    total_size: u64,
) -> c_int;

/// Initialize the JustDrop engine.
///
/// # Safety
/// `config_path` must be a valid null-terminated C string or NULL for defaults.
#[no_mangle]
pub unsafe extern "C" fn justdrop_init(config_path: *const c_char) -> c_int {
    // Initialize tracing
    let _ = tracing_subscriber::fmt()
        .with_env_filter("justdrop=info")
        .try_init();

    let config = if config_path.is_null() {
        Config::default()
    } else {
        let path_str = match CStr::from_ptr(config_path).to_str() {
            Ok(s) => s,
            Err(_) => return -1,
        };
        match Config::load(std::path::Path::new(path_str)) {
            Ok(c) => c,
            Err(e) => {
                error!("config load failed: {e}");
                return -2;
            }
        }
    };

    let runtime = match Runtime::new() {
        Ok(r) => r,
        Err(e) => {
            error!("runtime creation failed: {e}");
            return -3;
        }
    };

    let data_dir = Config::data_dir();
    let identity = match IdentityKeys::load_or_generate(&data_dir) {
        Ok(k) => Arc::new(k),
        Err(e) => {
            error!("key generation failed: {e}");
            return -4;
        }
    };

    let (event_tx, event_rx) = mpsc::channel(256);

    let engine = JustDropEngine {
        runtime,
        config,
        identity,
        registrar: None,
        browser: None,
        event_tx,
        event_rx: Mutex::new(Some(event_rx)),
    };

    *ENGINE.lock() = Some(Box::new(engine));
    info!("JustDrop engine initialized");
    0
}

/// Start mDNS discovery (register + browse).
#[no_mangle]
pub extern "C" fn justdrop_start_discovery() -> c_int {
    let mut guard = ENGINE.lock();
    let engine = match guard.as_mut() {
        Some(e) => e,
        None => return -1,
    };

    let service_type = &engine.config.discovery.service_type.clone();
    let device_name = engine.config.device_name();
    let port = engine.config.network.listen_port;
    let fingerprint = *engine.identity.fingerprint();

    // Enter the Tokio runtime context so spawn_blocking works inside the browser
    let _rt_guard = engine.runtime.enter();

    // Register our service
    let mut registrar = match ServiceRegistrar::new(service_type, &device_name) {
        Ok(r) => r,
        Err(e) => {
            error!("registrar creation failed: {e}");
            return -2;
        }
    };

    if let Err(e) = registrar.register(port, &fingerprint) {
        error!("service registration failed: {e}");
        return -3;
    }

    // Start browsing
    let (browser, _rx) = ServiceBrowser::new(service_type);
    if let Err(e) = browser.start_browsing(registrar.daemon()) {
        error!("browse start failed: {e}");
        return -4;
    }

    engine.registrar = Some(registrar);
    engine.browser = Some(browser);

    info!("discovery started");
    0
}

/// Get list of discovered peers as JSON.
///
/// # Safety
/// Caller must free the returned string with `justdrop_free_string`.
#[no_mangle]
pub extern "C" fn justdrop_get_peers() -> *mut c_char {
    let guard = ENGINE.lock();
    let engine = match guard.as_ref() {
        Some(e) => e,
        None => return std::ptr::null_mut(),
    };

    let peers = engine
        .browser
        .as_ref()
        .map(|b| b.peers())
        .unwrap_or_default();

    let json = match serde_json::to_string(&peers) {
        Ok(j) => j,
        Err(_) => return std::ptr::null_mut(),
    };

    match CString::new(json) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Send files to a peer.
///
/// # Safety
/// `peer_id` and `file_paths_json` must be valid null-terminated C strings.
/// `file_paths_json` is a JSON array of file path strings.
#[no_mangle]
pub unsafe extern "C" fn justdrop_send_files(
    peer_id: *const c_char,
    file_paths_json: *const c_char,
) -> c_int {
    let guard = ENGINE.lock();
    let engine = match guard.as_ref() {
        Some(e) => e,
        None => return -1,
    };

    let peer_id_str = match CStr::from_ptr(peer_id).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -2,
    };

    let paths_json = match CStr::from_ptr(file_paths_json).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -3,
    };

    let file_paths: Vec<PathBuf> = match serde_json::from_str(&paths_json) {
        Ok(p) => p,
        Err(_) => return -4,
    };

    let peer = match engine.browser.as_ref().and_then(|b| b.get_peer(&peer_id_str)) {
        Some(p) => p,
        None => return -5,
    };

    let sender = SendTransfer::new(engine.config.clone(), Arc::clone(&engine.identity));
    let event_tx = engine.event_tx.clone();

    engine.runtime.spawn(async move {
        match sender.send_files(&peer, &file_paths, event_tx).await {
            Ok(id) => info!(transfer_id = %id, "send complete"),
            Err(e) => error!("send failed: {e}"),
        }
    });

    0
}

/// Free a string allocated by JustDrop.
///
/// # Safety
/// `ptr` must have been returned by a `justdrop_*` function.
#[no_mangle]
pub unsafe extern "C" fn justdrop_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

/// Shut down the JustDrop engine.
#[no_mangle]
pub extern "C" fn justdrop_shutdown() -> c_int {
    let mut guard = ENGINE.lock();
    if let Some(engine) = guard.take() {
        drop(engine);
        info!("JustDrop engine shut down");
    }
    0
}

/// Accept an incoming transfer request.
///
/// # Safety
/// `transfer_id` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn justdrop_accept_transfer(transfer_id: *const c_char) -> c_int {
    if transfer_id.is_null() {
        return -1;
    }
    let id = match CStr::from_ptr(transfer_id).to_str() {
        Ok(s) => s,
        Err(_) => return -2,
    };
    info!(transfer_id = id, "transfer accepted by user");
    0
}

/// Reject an incoming transfer request.
///
/// # Safety
/// `transfer_id` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn justdrop_reject_transfer(transfer_id: *const c_char) -> c_int {
    if transfer_id.is_null() {
        return -1;
    }
    let id = match CStr::from_ptr(transfer_id).to_str() {
        Ok(s) => s,
        Err(_) => return -2,
    };
    info!(transfer_id = id, "transfer rejected by user");
    0
}
