pub mod platform;
pub mod lock;
pub mod ffi;

// Re-export lock/error FFI
pub use ffi::lock::{zip_ease_lock_directory, zip_ease_unlock_directory};
pub use ffi::error::{zip_ease_get_last_error, zip_ease_free_error_string};

// Lock-detector FFI — forwarding wrapper so the symbol is emitted in this cdylib.
// (pub use alone does not force #[no_mangle] symbols from rlib deps into a cdylib.)
#[no_mangle]
pub extern "C" fn zip_ease_who_locks(path_ptr: *const u16) -> *mut u16 {
    zipease_extract::ffi::lock_detector::zip_ease_who_locks(path_ptr)
}

// Re-export extract FFI (from zipease-extract)
pub use zipease_extract::ffi::extract::{
    zip_ease_extract,
    zip_ease_extract_with_progress,
    zip_ease_extract_with_password,
};
pub use zipease_extract::ffi::list::{
    zip_ease_list_archive_contents,
    zip_ease_list_archive_contents_with_password,
    zip_ease_free_archive_entries,
};

// Re-export compress FFI (from zipease-compress)
pub use zipease_compress::ffi::compress::zip_ease_compress;

// Legacy smoke-test stub
#[no_mangle]
pub extern "C" fn zip_ease_add(left: u64, right: u64) -> u64 {
    left + right
}
