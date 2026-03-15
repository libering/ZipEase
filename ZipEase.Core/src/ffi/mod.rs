// FFI (Foreign Function Interface) module
// Exports C-compatible functions for C# P/Invoke

pub mod lock;
pub mod error;
pub mod extract;
pub mod list;

// Re-export FFI functions
pub use lock::{zip_ease_lock_directory, zip_ease_unlock_directory};
pub use error::{zip_ease_get_last_error, zip_ease_free_error_string};
pub use extract::{zip_ease_extract, zip_ease_extract_with_password};
pub use list::{zip_ease_list_archive_contents, zip_ease_free_archive_entries, zip_ease_list_archive_contents_with_password};
