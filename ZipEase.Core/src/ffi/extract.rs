use std::path::PathBuf;
use std::ffi::OsString;
use std::os::windows::ffi::{OsStringExt, OsStrExt};
use crate::extract;
use crate::error::storage::set_last_error;

/// Safely parses a null-terminated UTF-16 string pointer into a PathBuf.
/// 
/// # Safety
/// 
/// The caller must ensure that `ptr` points to a valid null-terminated UTF-16 string.
/// The pointer must remain valid for the duration of this function call.
unsafe fn parse_wide_string(ptr: *const u16) -> PathBuf {
    if ptr.is_null() {
        return PathBuf::new();
    }
    
    let mut len = 0;
    while *ptr.offset(len) != 0 {
        len += 1;
    }
    let slice = std::slice::from_raw_parts(ptr, len as usize);
    PathBuf::from(OsString::from_wide(slice))
}

#[no_mangle]
pub extern "C" fn zip_ease_extract(
    archive_path_ptr: *const u16,
    output_dir_ptr: *const u16,
) -> i32 {
    std::panic::catch_unwind(|| {
        if archive_path_ptr.is_null() || output_dir_ptr.is_null() {
            return -1;
        }

        let archive_path = unsafe { parse_wide_string(archive_path_ptr) };
        let output_dir = unsafe { parse_wide_string(output_dir_ptr) };

        match extract::extract(&archive_path, &output_dir) {
            Ok(_) => 0,
            Err(e) => {
                let code = e.to_error_code();
                set_last_error(e);
                code
            }
        }
    }).unwrap_or(-1)
}

/// Callback type for progress reporting: fn(percentage: i32, current_file: *const u16)
type ProgressCallback = extern "C" fn(i32, *const u16);

#[no_mangle]
pub extern "C" fn zip_ease_extract_with_progress(
    archive_path_ptr: *const u16,
    output_dir_ptr: *const u16,
    progress_callback: Option<ProgressCallback>,
) -> i32 {
    use std::sync::atomic::{AtomicUsize, Ordering};
    
    std::panic::catch_unwind(|| {
        if archive_path_ptr.is_null() || output_dir_ptr.is_null() {
            return -1;
        }

        let archive_path = unsafe { parse_wide_string(archive_path_ptr) };
        let output_dir = unsafe { parse_wide_string(output_dir_ptr) };

        // Track file count for return value
        let file_count = AtomicUsize::new(0);
        
        // Create progress reporter closure
        let progress_fn = |current: usize, total: usize, file_name: &str| {
            if let Some(callback) = progress_callback {
                let percentage = if total > 0 {
                    ((current as f64 / total as f64) * 100.0) as i32
                } else {
                    0
                };
                
                // Convert filename to UTF-16
                let wide_name: Vec<u16> = OsString::from(file_name)
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                
                callback(percentage, wide_name.as_ptr());
                file_count.store(current, Ordering::Relaxed);
            }
        };

        match extract::extract_with_progress(&archive_path, &output_dir, progress_fn) {
            Ok(_) => file_count.load(Ordering::Relaxed) as i32,
            Err(e) => {
                let code = e.to_error_code();
                set_last_error(e);
                code
            }
        }
    }).unwrap_or(-1)
}

/// Extract a password-protected archive with progress reporting.
/// If `password_ptr` is null, delegates to the no-password path.
/// Returns 0x2004 if the password is wrong.
#[no_mangle]
pub extern "C" fn zip_ease_extract_with_password(
    archive_path_ptr: *const u16,
    output_dir_ptr: *const u16,
    password_ptr: *const u16,
    progress_callback: Option<ProgressCallback>,
) -> i32 {
    use std::sync::atomic::{AtomicUsize, Ordering};

    std::panic::catch_unwind(|| {
        if archive_path_ptr.is_null() || output_dir_ptr.is_null() {
            return -1;
        }

        let archive_path = unsafe { parse_wide_string(archive_path_ptr) };
        let output_dir = unsafe { parse_wide_string(output_dir_ptr) };

        // If no password, delegate to the no-password function
        if password_ptr.is_null() {
            let file_count = AtomicUsize::new(0);
            let progress_fn = |current: usize, total: usize, file_name: &str| {
                if let Some(callback) = progress_callback {
                    let percentage = if total > 0 {
                        ((current as f64 / total as f64) * 100.0) as i32
                    } else {
                        0
                    };
                    let wide_name: Vec<u16> = OsString::from(file_name)
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect();
                    callback(percentage, wide_name.as_ptr());
                    file_count.store(current, Ordering::Relaxed);
                }
            };
            return match extract::extract_with_progress(&archive_path, &output_dir, progress_fn) {
                Ok(_) => file_count.load(Ordering::Relaxed) as i32,
                Err(e) => {
                    let code = e.to_error_code();
                    set_last_error(e);
                    code
                }
            };
        }

        let password_path = unsafe { parse_wide_string(password_ptr) };
        let password = password_path.to_string_lossy().into_owned();

        let ext = archive_path.extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        let file_count = AtomicUsize::new(0);
        let make_progress = |current: usize, total: usize, file_name: &str| {
            if let Some(callback) = progress_callback {
                let percentage = if total > 0 {
                    ((current as f64 / total as f64) * 100.0) as i32
                } else {
                    0
                };
                let wide_name: Vec<u16> = OsString::from(file_name)
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                callback(percentage, wide_name.as_ptr());
                file_count.store(current, Ordering::Relaxed);
            }
        };

        let result = match ext.as_str() {
            "zip" => {
                use crate::extract::zip::ZipBackend;
                ZipBackend.extract_with_password_progress(&archive_path, &output_dir, &password, make_progress)
            }
            "7z" => {
                use crate::extract::sevenz::SevenZBackend;
                SevenZBackend.extract_with_password_progress(&archive_path, &output_dir, &password, make_progress)
            }
            _ => {
                extract::extract_with_progress(&archive_path, &output_dir, make_progress)
            }
        };

        match result {
            Ok(_) => file_count.load(Ordering::Relaxed) as i32,
            Err(e) => {
                let code = e.to_error_code();
                set_last_error(e);
                code
            }
        }
    })
    .unwrap_or(-1)
}

