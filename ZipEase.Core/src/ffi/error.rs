// FFI error functions (Task 8)
// Provides C-compatible APIs for error reporting

use crate::error::get_last_error;
use std::ffi::CString;
use std::os::raw::c_char;

/// Retrieves the last error message as a C string
///
/// Returns a pointer to a null-terminated UTF-8 string.
/// The caller is responsible for freeing this memory by calling
/// zip_ease_free_error_string().
/// If no error has occurred, returns a null pointer.
#[no_mangle]
pub extern "C" fn zip_ease_get_last_error() -> *const c_char {
    std::panic::catch_unwind(|| {
        match get_last_error() {
            Some(error) => {
                let msg = error.message();
                match CString::new(msg) {
                    Ok(c_str) => c_str.into_raw(),
                    Err(_) => std::ptr::null(),
                }
            }
            None => std::ptr::null(),
        }
    })
    .unwrap_or(std::ptr::null())
}

/// Frees a string allocated by Rust and passed to C#
///
/// This must be called for every non-null pointer returned by
/// zip_ease_get_last_error().
#[no_mangle]
pub extern "C" fn zip_ease_free_error_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }

    let _ = std::panic::catch_unwind(|| unsafe {
        let _ = CString::from_raw(ptr);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{set_last_error, clear_last_error, LockError};
    use std::ffi::CStr;

    #[test]
    fn test_ffi_error_roundtrip() {
        clear_last_error();
        
        // 1. No error initially
        assert!(zip_ease_get_last_error().is_null());
        
        // 2. Set an error
        let msg = "test error message";
        set_last_error(LockError::Unknown(msg.into()));
        
        // 3. Get error via FFI
        let ptr = zip_ease_get_last_error();
        assert!(!ptr.is_null());
        
        let c_str = unsafe { CStr::from_ptr(ptr) };
        let retrieved_msg = c_str.to_str().unwrap();
        assert!(retrieved_msg.contains(msg));
        
        // 4. Free string
        zip_ease_free_error_string(ptr as *mut c_char);
    }

    #[test]
    fn test_ffi_error_null_handling() {
        clear_last_error();
        assert!(zip_ease_get_last_error().is_null());
        
        // Should not crash
        zip_ease_free_error_string(std::ptr::null_mut());
    }
}
