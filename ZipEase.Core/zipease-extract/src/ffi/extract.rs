use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use crate::extract;
use zipease_shared::{set_last_error, parse_wide_string};

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

/// Extract a ZIP archive ignoring CRC errors (force/recovery mode).
///
/// # Safety
/// - `archive_path_ptr` and `output_dir_ptr` must be valid UTF-16 null-terminated strings.
/// - `progress_callback` may be null; if provided it is called on the calling thread.
///
/// # Returns
/// - `0` on success
/// - negative error code on failure (call `zip_ease_get_last_error` for details)
#[no_mangle]
pub extern "C" fn zip_ease_extract_force(
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

        use crate::extract::zip::ZipBackend;
        match ZipBackend.extract_force_progress(&archive_path, &output_dir, progress_fn) {
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

/// Extract a single entry by index from a ZIP archive.
///
/// # Safety
/// - `archive_path_ptr`, `output_dir_ptr` must be valid UTF-16 null-terminated strings.
/// - `out_name_ptr` receives a Rust-allocated UTF-16 string; caller MUST free it with
///   `zip_ease_free_string`.
///
/// # Returns
/// - `0` on success, `out_name_ptr` is set to the extracted filename
/// - negative error code on failure
#[no_mangle]
pub extern "C" fn zip_ease_extract_entry(
    archive_path_ptr: *const u16,
    entry_index: u32,
    output_dir_ptr: *const u16,
    out_name_ptr: *mut *mut u16,
) -> i32 {
    std::panic::catch_unwind(|| {
        if archive_path_ptr.is_null() || output_dir_ptr.is_null() || out_name_ptr.is_null() {
            return -1;
        }

        let archive_path = unsafe { parse_wide_string(archive_path_ptr) };
        let output_dir = unsafe { parse_wide_string(output_dir_ptr) };

        use crate::extract::zip::ZipBackend;
        match ZipBackend.extract_entry(&archive_path, entry_index, &output_dir) {
            Ok(name) => {
                // Allocate UTF-16 string for the caller; freed via zip_ease_free_string
                let mut wide: Vec<u16> = OsString::from(&name)
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                wide.shrink_to_fit();
                let ptr = wide.as_mut_ptr();
                std::mem::forget(wide);
                unsafe { *out_name_ptr = ptr; }
                0
            }
            Err(e) => {
                let code = e.to_error_code();
                set_last_error(e);
                code
            }
        }
    })
    .unwrap_or(-1)
}

/// Free a UTF-16 string allocated by Rust FFI functions (e.g. `zip_ease_extract_entry`).
///
/// # Safety
/// `ptr` must have been returned by a Rust FFI function that documents this free requirement.
/// Passing any other pointer is undefined behaviour.
#[no_mangle]
pub unsafe extern "C" fn zip_ease_free_string(ptr: *mut u16) {
    if ptr.is_null() { return; }
    // Reconstruct the Vec to let Rust drop it properly.
    // We stored it as a null-terminated Vec<u16>; find the length first.
    let mut len = 0usize;
    while *ptr.add(len) != 0 { len += 1; }
    drop(Vec::from_raw_parts(ptr, len + 1, len + 1));
}
