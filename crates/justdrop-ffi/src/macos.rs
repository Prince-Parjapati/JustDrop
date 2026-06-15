//! macOS-specific FFI helpers.
//!
//! Legacy C-ABI functions retained for backward compatibility during
//! UniFFI migration. New code should use the `JustDropEngine` UniFFI interface.

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
