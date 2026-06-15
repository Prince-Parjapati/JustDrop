//! Android-specific FFI helpers for JNI interop.
//!
//! These functions are intended to be called from Kotlin/Java via JNI.
//! The actual JNI bridge code lives in the Android project.

/// Android-specific initialization (e.g., setting up file paths from Android context).
///
/// # Safety
/// `downloads_dir` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn justdrop_android_set_downloads_dir(
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
pub unsafe extern "C" fn justdrop_android_set_data_dir(
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

#[cfg(target_os = "android")]
use jni::{JNIEnv, objects::{JClass, JString}, sys::{jint, jstring}};

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_justdrop_app_JustBridge_init<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    config_path: JString<'local>,
) -> jint {
    let config_path_c_str = if !config_path.is_null() {
        match env.get_string(&config_path) {
            Ok(java_str) => Some(java_str),
            Err(_) => return -1,
        }
    } else {
        None
    };

    let ptr = config_path_c_str.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null());
    unsafe { crate::justdrop_init(ptr) as jint }
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_justdrop_app_JustBridge_shutdown<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jint {
    crate::justdrop_shutdown() as jint
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_justdrop_app_JustBridge_startDiscovery<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jint {
    crate::justdrop_start_discovery() as jint
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_justdrop_app_JustBridge_getPeers<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    let ptr = crate::justdrop_get_peers();
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let c_str = unsafe { std::ffi::CStr::from_ptr(ptr) };
    let string = c_str.to_string_lossy().into_owned();
    unsafe { crate::justdrop_free_string(ptr) };
    
    match env.new_string(string) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_justdrop_app_JustBridge_sendFiles<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    peer_id: JString<'local>,
    file_paths_json: JString<'local>,
) -> jint {
    let peer_id_str = match env.get_string(&peer_id) {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let file_paths_str = match env.get_string(&file_paths_json) {
        Ok(s) => s,
        Err(_) => return -1,
    };
    unsafe { crate::justdrop_send_files(peer_id_str.as_ptr(), file_paths_str.as_ptr()) as jint }
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_justdrop_app_JustBridge_acceptTransfer<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    transfer_id: JString<'local>,
) -> jint {
    let transfer_id_str = match env.get_string(&transfer_id) {
        Ok(s) => s,
        Err(_) => return -1,
    };
    unsafe { crate::justdrop_accept_transfer(transfer_id_str.as_ptr()) as jint }
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_justdrop_app_JustBridge_rejectTransfer<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    transfer_id: JString<'local>,
) -> jint {
    let transfer_id_str = match env.get_string(&transfer_id) {
        Ok(s) => s,
        Err(_) => return -1,
    };
    unsafe { crate::justdrop_reject_transfer(transfer_id_str.as_ptr()) as jint }
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_justdrop_app_JustBridge_setDownloadsDir<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    path: JString<'local>,
) -> jint {
    let path_str = match env.get_string(&path) {
        Ok(s) => s,
        Err(_) => return -1,
    };
    unsafe { justdrop_android_set_downloads_dir(path_str.as_ptr()) as jint }
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_justdrop_app_JustBridge_setDataDir<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    path: JString<'local>,
) -> jint {
    let path_str = match env.get_string(&path) {
        Ok(s) => s,
        Err(_) => return -1,
    };
    unsafe { justdrop_android_set_data_dir(path_str.as_ptr()) as jint }
}
