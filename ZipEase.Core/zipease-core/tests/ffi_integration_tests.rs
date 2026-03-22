use std::ffi::CStr;
use std::os::raw::c_char;
use tempfile::tempdir;
use zipease_core::{
    zip_ease_lock_directory, zip_ease_unlock_directory,
    zip_ease_get_last_error, zip_ease_free_error_string,
};
use zipease_shared::clear_last_error;

#[test]
fn test_ffi_complete_flow() {
    clear_last_error();
    let temp = tempdir().unwrap();
    let path_str = temp.path().to_str().unwrap();
    let mut path_wide: Vec<u16> = path_str.encode_utf16().collect();
    path_wide.push(0);

    let handle = zip_ease_lock_directory(path_wide.as_ptr());
    assert!(handle > 0, "Failed to lock directory via FFI");

    let error_ptr = zip_ease_get_last_error();
    if !error_ptr.is_null() {
        let c_str = unsafe { CStr::from_ptr(error_ptr) };
        let msg = c_str.to_string_lossy();
        zip_ease_free_error_string(error_ptr as *mut c_char);
        panic!("Error should be null on success, but got: {}", msg);
    }

    let delete_res = std::fs::remove_dir(temp.path());
    assert!(delete_res.is_err(), "Directory should be locked");

    let result = zip_ease_unlock_directory(handle);
    assert_eq!(result, 0, "Failed to unlock directory via FFI");

    let delete_res_after = std::fs::remove_dir(temp.path());
    assert!(delete_res_after.is_ok(), "Directory should be unlocked");
}

#[test]
fn test_ffi_error_handling() {
    clear_last_error();
    let result = zip_ease_unlock_directory(-999);
    assert_ne!(result, 0);

    let error_ptr = zip_ease_get_last_error();
    assert!(!error_ptr.is_null());

    let c_str = unsafe { CStr::from_ptr(error_ptr) };
    let msg = c_str.to_str().unwrap();
    assert!(msg.contains("Invalid handle"), "Error message mismatch: {}", msg);

    zip_ease_free_error_string(error_ptr as *mut c_char);
}

#[test]
fn test_ffi_null_pointer_handling() {
    clear_last_error();
    let handle = zip_ease_lock_directory(std::ptr::null());
    assert_eq!(handle, -1);

    let error_ptr = zip_ease_get_last_error();
    assert!(!error_ptr.is_null());

    let c_str = unsafe { CStr::from_ptr(error_ptr) };
    assert!(c_str.to_str().unwrap().contains("Null pointer"));

    zip_ease_free_error_string(error_ptr as *mut c_char);
}
