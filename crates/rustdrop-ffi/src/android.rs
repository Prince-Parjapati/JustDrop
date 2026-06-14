//! Android-specific FFI helpers for JNI interop.
//!
//! These functions are intended to be called from Kotlin/Java via JNI.
//! The actual JNI bridge code lives in the Android project.

/// Android-specific initialization (e.g., setting up file paths from Android context).
///
/// # Safety
/// `downloads_dir` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn rustdrop_android_set_downloads_dir(
    downloads_dir: *const std::os::raw::c_char,
) -> std::os::raw::c_int {
    if downloads_dir.is_null() {
        return -1;
    }

    let dir = match std::ffi::CStr::from_ptr(downloads_dir).to_str() {
        Ok(s) => s,
        Err(_) => return -2,
    };

    tracing::info!(dir = dir, "android downloads dir set");
    // Store for use in config
    // In a real implementation, this would update the global config
    0
}

/// Set the Android-specific data directory for key storage.
///
/// # Safety
/// `data_dir` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn rustdrop_android_set_data_dir(
    data_dir: *const std::os::raw::c_char,
) -> std::os::raw::c_int {
    if data_dir.is_null() {
        return -1;
    }

    let dir = match std::ffi::CStr::from_ptr(data_dir).to_str() {
        Ok(s) => s,
        Err(_) => return -2,
    };

    tracing::info!(dir = dir, "android data dir set");
    0
}
