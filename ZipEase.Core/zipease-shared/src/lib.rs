pub mod error;
pub mod ffi_helpers;

pub use error::{LockError, set_last_error, get_last_error, clear_last_error};
pub use ffi_helpers::{parse_wide_string, to_utf16_ptr, decode_filename};
