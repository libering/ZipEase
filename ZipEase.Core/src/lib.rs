// ZipEase Core Library
// Provides directory locking functionality for ZipEase

// Module declarations
pub mod error;
pub mod lock;
pub mod platform;
pub mod ffi;
pub mod extract;

// Re-export FFI functions for external use
pub use ffi::{
    zip_ease_lock_directory,
    zip_ease_unlock_directory,
    zip_ease_get_last_error,
    zip_ease_free_error_string,
    zip_ease_list_archive_contents,
    zip_ease_free_archive_entries,
};

// Legacy test function (will be removed later)
#[no_mangle]
pub extern "C" fn zip_ease_add(left: u64, right: u64) -> u64 {
    left + right
}