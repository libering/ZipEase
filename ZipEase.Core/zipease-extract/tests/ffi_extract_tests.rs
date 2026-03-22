//! Unit tests for FFI progress callback (ui-integration task 1.4).
//!
//! Feature: ui-integration, task 1.4
//! Validates: Requirements 9.2, 9.3

use zipease_extract::ffi::extract::{zip_ease_extract_with_progress, zip_ease_extract};

// ─── Null pointer handling ────────────────────────────────────────────────────

#[test]
fn test_null_archive_path_returns_neg1() {
    let result = zip_ease_extract_with_progress(
        std::ptr::null(),
        std::ptr::null(),
        None,
    );
    assert_eq!(result, -1, "null archive path must return -1");
}

#[test]
fn test_null_output_dir_returns_neg1() {
    let wide: Vec<u16> = "C:\\fake\\path.zip\0".encode_utf16().collect();
    let result = zip_ease_extract_with_progress(
        wide.as_ptr(),
        std::ptr::null(),
        None,
    );
    assert_eq!(result, -1, "null output dir must return -1");
}

#[test]
fn test_null_both_paths_returns_neg1() {
    let result = zip_ease_extract(std::ptr::null(), std::ptr::null());
    assert_eq!(result, -1, "null both paths must return -1");
}

// ─── Panic recovery ───────────────────────────────────────────────────────────

#[test]
fn test_no_panic_on_nonexistent_archive() {
    // A non-existent path should return a negative error code, not panic.
    let archive: Vec<u16> = "C:\\does_not_exist_xyz_abc.zip\0".encode_utf16().collect();
    let output: Vec<u16> = "C:\\temp\0".encode_utf16().collect();
    let result = zip_ease_extract_with_progress(archive.as_ptr(), output.as_ptr(), None);
    assert!(result < 0, "non-existent archive must return negative error code, got {}", result);
}

// ─── Callback not invoked for missing archive ─────────────────────────────────

#[test]
fn test_callback_not_invoked_on_error() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

    extern "C" fn counting_callback(_pct: i32, _file: *const u16) {
        CALL_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    CALL_COUNT.store(0, Ordering::Relaxed);

    let archive: Vec<u16> = "C:\\does_not_exist_xyz_abc.zip\0".encode_utf16().collect();
    let output: Vec<u16> = "C:\\temp\0".encode_utf16().collect();
    let _ = zip_ease_extract_with_progress(archive.as_ptr(), output.as_ptr(), Some(counting_callback));

    assert_eq!(
        CALL_COUNT.load(Ordering::Relaxed),
        0,
        "callback must not be invoked when archive does not exist"
    );
}

// ─── Error code is negative ───────────────────────────────────────────────────

#[test]
fn test_error_code_is_negative_for_bad_path() {
    let archive: Vec<u16> = "C:\\no_such_file_12345.zip\0".encode_utf16().collect();
    let output: Vec<u16> = "C:\\temp\0".encode_utf16().collect();
    let result = zip_ease_extract_with_progress(archive.as_ptr(), output.as_ptr(), None);
    assert!(result < 0, "error code must be negative, got {}", result);
}
