//! Legacy C-ABI functions for Swift/Kotlin interoperability

use std::os::raw::{c_char, c_int};
use std::ffi::CString;

#[no_mangle]
pub extern "C" fn justdrop_init(_config_path: *const c_char) -> c_int {
    0
}

#[no_mangle]
pub extern "C" fn justdrop_shutdown() -> c_int {
    0
}

#[no_mangle]
pub extern "C" fn justdrop_start_discovery() -> c_int {
    0
}

#[no_mangle]
pub extern "C" fn justdrop_get_peers() -> *mut c_char {
    let json = "[]";
    CString::new(json).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn justdrop_send_files(_peer_id: *const c_char, _paths_json: *const c_char) -> c_int {
    0
}

#[no_mangle]
pub extern "C" fn justdrop_set_trust(_peer_id: *const c_char, _level: c_int) -> c_int {
    0
}

#[no_mangle]
pub extern "C" fn justdrop_cancel_transfer(_transfer_id: *const c_char) -> c_int {
    0
}
