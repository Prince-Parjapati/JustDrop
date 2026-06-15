//! macOS-specific FFI helpers for Swift interop.

/// macOS-specific initialization.
///
/// # Safety
/// `bundle_id` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn justdrop_macos_set_bundle_id(
    bundle_id: *const std::os::raw::c_char,
) -> std::os::raw::c_int {
    if bundle_id.is_null() {
        return -1;
    }

    let id = match std::ffi::CStr::from_ptr(bundle_id).to_str() {
        Ok(s) => s,
        Err(_) => return -2,
    };

    tracing::info!(bundle_id = id, "macOS bundle ID set");
    0
}

/// Get the engine's public key fingerprint as a hex string.
///
/// # Safety
/// Caller must free the returned string with `justdrop_free_string`.
#[no_mangle]
pub extern "C" fn justdrop_macos_get_fingerprint() -> *mut std::os::raw::c_char {
    let guard = super::ENGINE.lock();
    let engine = match guard.as_ref() {
        Some(e) => e,
        None => return std::ptr::null_mut(),
    };

    let fp = engine.identity.fingerprint_hex();
    match std::ffi::CString::new(fp) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}
