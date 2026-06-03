// FFI functions receive raw pointers from C# P/Invoke and must dereference them,
// but cannot be marked `unsafe` as that changes the extern "C" calling convention.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

pub mod platform;
pub mod lock;
pub mod ffi;

// Re-export lock/error FFI
pub use ffi::lock::{zip_ease_lock_directory, zip_ease_unlock_directory};
pub use ffi::error::{zip_ease_get_last_error, zip_ease_free_error_string};

/// Automatically initialise the Rust file logger when the DLL is loaded.
/// `#[ctor]` runs before any user code — no manual call needed from C#.
/// In release builds the log level can be raised to Warn/Error to reduce noise.
#[ctor::ctor]
fn dll_init() {
    zipease_extract::init_logging();
}

// Lock-detector FFI — forwarding wrapper so the symbol is emitted in this cdylib.
// (pub use alone does not force #[no_mangle] symbols from rlib deps into a cdylib.)
#[no_mangle]
pub extern "C" fn zip_ease_who_locks(path_ptr: *const u16) -> *mut u16 {
    std::panic::catch_unwind(|| {
        zipease_extract::ffi::lock_detector::zip_ease_who_locks(path_ptr)
    })
    .unwrap_or(std::ptr::null_mut())
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
pub use zipease_extract::ffi::search::{
    zip_ease_search_entries,
    zip_ease_free_search_results,
};

// Re-export compress FFI (from zipease-compress)
pub use zipease_compress::ffi::compress::zip_ease_compress;

