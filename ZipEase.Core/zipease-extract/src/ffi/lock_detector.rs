// FFI shim for lock detection.
// Business logic lives in `crate::lock_detector::who_locks`.
// This file is the sole `extern "C"` export point for lock detection.

/// Query which processes hold a lock on the file at `path_ptr`.
///
/// Returns a Rust-allocated null-terminated UTF-16 string of comma-separated
/// process names, or null if no lock is detected or any error occurs.
/// The caller must free the returned pointer via `zip_ease_free_string`.
///
/// # Safety
/// `path_ptr` must be a valid null-terminated UTF-16 string, or null.
/// The returned pointer must be freed by the C# caller via `zip_ease_free_string`.
pub extern "C" fn zip_ease_who_locks(path_ptr: *const u16) -> *mut u16 {
    // Requirements 4.2, 5.2: catch_unwind prevents any Rust panic from crossing the FFI boundary.
    match std::panic::catch_unwind(|| {
        // Requirement 5.4: delegate entirely to the lock_detector module.
        crate::lock_detector::who_locks(path_ptr)
    }) {
        Ok(ptr) => ptr,
        Err(_) => std::ptr::null_mut(),
    }
}
