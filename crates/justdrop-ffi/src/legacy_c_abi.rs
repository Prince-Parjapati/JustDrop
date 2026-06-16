//! Live C-ABI functions for Swift/Kotlin interoperability.
//!
//! These functions bridge the platform layers (Swift menu bar app, Android UI)
//! to the Rust engine. A global singleton holds the tokio runtime, mDNS
//! discovery, and the transfer listener.

use justdrop_core::config::Config;
use justdrop_discovery::{ServiceBrowser, ServiceRegistrar};
use justdrop_network::TransferListener;
use justdrop_protocol::{IncomingTransferDecision, RecvTransfer, TransferEvent};
use justdrop_security::IdentityKeys;
use parking_lot::Mutex;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};

/// Global singleton holding the entire running state.
struct GlobalState {
    runtime: tokio::runtime::Runtime,
    config: Config,
    identity: Arc<IdentityKeys>,
    browser: Arc<ServiceBrowser>,
    // ServiceRegistrar must stay alive to keep the mDNS registration active.
    _registrar: ServiceRegistrar,
}

static GLOBAL: Mutex<Option<GlobalState>> = Mutex::new(None);

#[no_mangle]
/// # Safety
/// `data_dir_ptr` must be a valid null-terminated C string or null.
pub unsafe extern "C" fn justdrop_init(data_dir_ptr: *const c_char) -> c_int {
    // Idempotent — don't double-init
    if GLOBAL.lock().is_some() {
        return 0;
    }

    let data_dir = if data_dir_ptr.is_null() {
        Config::data_dir()
    } else {
        let c_str = unsafe { CStr::from_ptr(data_dir_ptr) };
        PathBuf::from(c_str.to_string_lossy().as_ref())
    };

    // Initialize tracing (ignore errors if already init)
    let _ = tracing_subscriber::fmt()
        .with_env_filter("justdrop=info")
        .try_init();

    // Create data directory
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        error!(error = %e, "failed to create data dir");
        return -1;
    }

    // Load config
    let config = Config::default();

    // Load or generate identity
    let identity = match IdentityKeys::load_or_generate(&data_dir) {
        Ok(k) => Arc::new(k),
        Err(e) => {
            error!(error = %e, "failed to load identity");
            return -2;
        }
    };

    // Create tokio runtime
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            error!(error = %e, "failed to create tokio runtime");
            return -3;
        }
    };

    let service_type = config.discovery.service_type.clone();
    let device_name = config.device_name();
    let port = config.network.listen_port;
    let fingerprint = *identity.fingerprint();

    // Start mDNS registrar
    let mut registrar = match ServiceRegistrar::new(&service_type, &device_name) {
        Ok(r) => r,
        Err(e) => {
            error!(error = %e, "failed to create mDNS registrar");
            return -4;
        }
    };

    if let Err(e) = registrar.register(port, &fingerprint) {
        error!(error = %e, "failed to register mDNS service");
        return -5;
    }

    // Start mDNS browser
    let (browser, _peer_rx) = ServiceBrowser::new(&service_type);
    if let Err(e) = runtime.block_on(async { browser.start_browsing(registrar.daemon()) }) {
        error!(error = %e, "failed to start mDNS browsing");
        return -6;
    }

    let browser = Arc::new(browser);

    // Start TCP listener in background
    let config_clone = config.clone();
    let identity_clone = Arc::clone(&identity);
    runtime.spawn(async move {
        let listener = match TransferListener::bind(&config_clone.network).await {
            Ok(l) => l,
            Err(e) => {
                error!(error = %e, "failed to bind TCP listener");
                return;
            }
        };

        info!(addr = %listener.local_addr(), "TCP listener ready");

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!(peer = %addr, "incoming connection");
                    let (decision_tx, decision_rx) = mpsc::channel(1);
                    let (event_tx, _event_rx) = mpsc::channel::<TransferEvent>(64);

                    // Auto-accept
                    let _ = decision_tx.send(IncomingTransferDecision::Accept).await;

                    let recv = RecvTransfer::new(config_clone.clone(), Arc::clone(&identity_clone));
                    tokio::spawn(async move {
                        if let Err(e) = recv.handle_incoming(stream, decision_rx, event_tx).await {
                            error!(error = %e, "incoming transfer failed");
                        }
                    });
                }
                Err(e) => {
                    error!(error = %e, "accept failed");
                }
            }
        }
    });

    info!(
        device = %device_name,
        fingerprint = %justdrop_discovery::registrar::hex_encode(&fingerprint[..8]),
        "JustDrop engine initialized"
    );

    *GLOBAL.lock() = Some(GlobalState {
        runtime,
        config,
        identity,
        browser,
        _registrar: registrar,
    });

    0
}

#[no_mangle]
pub extern "C" fn justdrop_shutdown() -> c_int {
    let mut guard = GLOBAL.lock();
    if guard.take().is_some() {
        info!("JustDrop engine shut down");
    }
    0
}

#[no_mangle]
pub extern "C" fn justdrop_start_discovery() -> c_int {
    // Discovery is already started by justdrop_init. This is a no-op kept for API compat.
    0
}

#[no_mangle]
pub extern "C" fn justdrop_get_peers() -> *mut c_char {
    let guard = GLOBAL.lock();
    let state = match guard.as_ref() {
        Some(s) => s,
        None => {
            let json = CString::new("[]").unwrap();
            return json.into_raw();
        }
    };

    let peers = state.browser.peers();
    let json_peers: Vec<serde_json::Value> = peers
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "name": p.name,
                "addr": p.addr.to_string(),
                "platform": format!("{}", p.platform),
                "fingerprint": justdrop_discovery::registrar::hex_encode(&p.fingerprint),
            })
        })
        .collect();

    let json_str = serde_json::to_string(&json_peers).unwrap_or_else(|_| "[]".to_string());
    CString::new(json_str).unwrap_or_default().into_raw()
}

#[no_mangle]
/// # Safety
/// `peer_id` and `paths_json` must be valid null-terminated C strings or null.
pub unsafe extern "C" fn justdrop_send_files(
    peer_id: *const c_char,
    paths_json: *const c_char,
) -> c_int {
    if peer_id.is_null() || paths_json.is_null() {
        return -1;
    }

    let peer_id_str = unsafe { CStr::from_ptr(peer_id) }
        .to_string_lossy()
        .to_string();
    let paths_str = unsafe { CStr::from_ptr(paths_json) }
        .to_string_lossy()
        .to_string();

    let paths: Vec<String> = match serde_json::from_str(&paths_str) {
        Ok(p) => p,
        Err(_) => return -2,
    };

    let guard = GLOBAL.lock();
    let state = match guard.as_ref() {
        Some(s) => s,
        None => return -3,
    };

    let peer = match state.browser.get_peer(&peer_id_str) {
        Some(p) => p,
        None => {
            error!(peer_id = %peer_id_str, "peer not found for send");
            return -4;
        }
    };

    let file_paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let sender =
        justdrop_protocol::SendTransfer::new(state.config.clone(), Arc::clone(&state.identity));
    let (event_tx, _event_rx) = mpsc::channel(64);

    state.runtime.spawn(async move {
        match sender.send_files(&peer, &file_paths, event_tx).await {
            Ok(tid) => info!(transfer_id = %tid, "transfer completed"),
            Err(e) => error!(error = %e, "transfer failed"),
        }
    });

    0
}

#[no_mangle]
pub extern "C" fn justdrop_set_trust(_peer_id: *const c_char, _level: c_int) -> c_int {
    // Trust management requires the full Engine (not used in current C-ABI path).
    // Stubbed for now — trust changes are persisted via the UniFFI JustDropEngine path.
    0
}

#[no_mangle]
pub extern "C" fn justdrop_cancel_transfer(_transfer_id: *const c_char) -> c_int {
    // Cancel requires session tracking — not yet wired in the C-ABI path.
    0
}

/// Free a string previously returned by justdrop_get_peers or similar.
///
/// # Safety
/// `ptr` must be a pointer previously returned by a justdrop C-ABI function,
/// or null.
#[no_mangle]
pub unsafe extern "C" fn justdrop_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr);
    }
}
