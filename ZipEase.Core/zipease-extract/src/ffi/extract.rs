use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use crate::extract;
use zipease_shared::{set_last_error, parse_wide_string};

/// Convert a LockError to a negative FFI error code.
///
/// FFI contract: 0 = success, negative = failure.
/// `LockError::to_error_code()` may return positive Windows error codes (e.g. 3 for
/// PathNotFound), so we negate any positive value to ensure C# `result < 0` works.
///
/// Special sentinel preserved as-is:
///   - `PasswordRequired` (0x2004): C# checks `result == unchecked((int)0x2004)`.
#[inline]
fn to_ffi_error(e: zipease_shared::LockError) -> i32 {
    let code = e.to_error_code();
    set_last_error(e);
    if code == 0x2004 { return code; }
    if code > 0 { -code } else { code }
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
        let output_dir   = unsafe { parse_wide_string(output_dir_ptr) };
        match extract::extract(&archive_path, &output_dir) {
            Ok(_)  => 0,
            Err(e) => to_ffi_error(e),
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
        let output_dir   = unsafe { parse_wide_string(output_dir_ptr) };

        let file_count = AtomicUsize::new(0);
        let progress_fn = |current: usize, total: usize, file_name: &str| {
            if let Some(callback) = progress_callback {
                let pct = if total > 0 { ((current as f64 / total as f64) * 100.0) as i32 } else { 0 };
                let wide: Vec<u16> = OsString::from(file_name).encode_wide().chain(std::iter::once(0)).collect();
                callback(pct, wide.as_ptr());
                file_count.store(current, Ordering::Relaxed);
            }
        };

        match extract::extract_with_progress(&archive_path, &output_dir, progress_fn) {
            Ok(_)  => file_count.load(Ordering::Relaxed) as i32,
            Err(e) => to_ffi_error(e),
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
        let output_dir   = unsafe { parse_wide_string(output_dir_ptr) };

        let file_count = AtomicUsize::new(0);

        if password_ptr.is_null() {
            let progress_fn = |current: usize, total: usize, file_name: &str| {
                if let Some(callback) = progress_callback {
                    let pct = if total > 0 { ((current as f64 / total as f64) * 100.0) as i32 } else { 0 };
                    let wide: Vec<u16> = OsString::from(file_name).encode_wide().chain(std::iter::once(0)).collect();
                    callback(pct, wide.as_ptr());
                    file_count.store(current, Ordering::Relaxed);
                }
            };
            return match extract::extract_with_progress(&archive_path, &output_dir, progress_fn) {
                Ok(_)  => file_count.load(Ordering::Relaxed) as i32,
                Err(e) => to_ffi_error(e),
            };
        }

        let password = unsafe { parse_wide_string(password_ptr) }.to_string_lossy().into_owned();
        let ext = archive_path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()).unwrap_or_default();

        let progress_fn2 = |current: usize, total: usize, file_name: &str| {
            if let Some(callback) = progress_callback {
                let pct = if total > 0 { ((current as f64 / total as f64) * 100.0) as i32 } else { 0 };
                let wide: Vec<u16> = OsString::from(file_name).encode_wide().chain(std::iter::once(0)).collect();
                callback(pct, wide.as_ptr());
                file_count.store(current, Ordering::Relaxed);
            }
        };

        let result = match ext.as_str() {
            "zip" => {
                use crate::extract::zip::ZipBackend;
                ZipBackend.extract_with_password_progress(&archive_path, &output_dir, &password, progress_fn2)
            }
            "7z" => {
                use crate::extract::sevenz::SevenZBackend;
                SevenZBackend.extract_with_password_progress(&archive_path, &output_dir, &password, progress_fn2)
            }
            _ => extract::extract_with_progress(&archive_path, &output_dir, progress_fn2),
        };

        match result {
            Ok(_)  => file_count.load(Ordering::Relaxed) as i32,
            Err(e) => to_ffi_error(e),
        }
    }).unwrap_or(-1)
}

/// Extract a ZIP archive ignoring CRC errors (force/recovery mode).
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
        let output_dir   = unsafe { parse_wide_string(output_dir_ptr) };

        let file_count = AtomicUsize::new(0);
        let progress_fn = |current: usize, total: usize, file_name: &str| {
            if let Some(callback) = progress_callback {
                let pct = if total > 0 { ((current as f64 / total as f64) * 100.0) as i32 } else { 0 };
                let wide: Vec<u16> = OsString::from(file_name).encode_wide().chain(std::iter::once(0)).collect();
                callback(pct, wide.as_ptr());
                file_count.store(current, Ordering::Relaxed);
            }
        };

        use crate::extract::zip::ZipBackend;
        match ZipBackend.extract_force_progress(&archive_path, &output_dir, progress_fn) {
            Ok(_)  => file_count.load(Ordering::Relaxed) as i32,
            Err(e) => to_ffi_error(e),
        }
    }).unwrap_or(-1)
}

/// Extract a single entry by index from a ZIP archive.
///
/// # Safety
/// `out_name_ptr` receives a Rust-allocated UTF-16 string; caller MUST free with `zip_ease_free_string`.
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
        let output_dir   = unsafe { parse_wide_string(output_dir_ptr) };

        use crate::extract::zip::ZipBackend;
        match ZipBackend.extract_entry(&archive_path, entry_index, &output_dir) {
            Ok(name) => {
                let mut wide: Vec<u16> = OsString::from(&name).encode_wide().chain(std::iter::once(0)).collect();
                wide.shrink_to_fit();
                let ptr = wide.as_mut_ptr();
                std::mem::forget(wide);
                unsafe { *out_name_ptr = ptr; }
                0
            }
            Err(e) => to_ffi_error(e),
        }
    }).unwrap_or(-1)
}

/// Free a UTF-16 string allocated by Rust FFI functions.
///
/// # Safety
/// `ptr` must have been returned by a Rust FFI function that documents this requirement.
#[no_mangle]
pub unsafe extern "C" fn zip_ease_free_string(ptr: *mut u16) {
    let _ = std::panic::catch_unwind(|| {
        if ptr.is_null() { return; }
        let mut len = 0usize;
        while *ptr.add(len) != 0 { len += 1; }
        drop(Vec::from_raw_parts(ptr, len + 1, len + 1));
    });
}

/// Extract a single entry by index from any archive format.
/// ZIP uses fast index-based path; other formats extract to temp and copy.
///
/// # Safety
/// `out_name_ptr` must be freed with `zip_ease_free_string`.
#[no_mangle]
pub extern "C" fn zip_ease_extract_entry_any(
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
        let output_dir   = unsafe { parse_wide_string(output_dir_ptr) };

        let ext = archive_path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()).unwrap_or_default();

        // ZIP: fast index-based extraction
        if matches!(ext.as_str(), "zip" | "apk" | "jar" | "ipa") {
            use crate::extract::zip::ZipBackend;
            return match ZipBackend.extract_entry(&archive_path, entry_index, &output_dir) {
                Ok(name) => {
                    let mut wide: Vec<u16> = OsString::from(&name).encode_wide().chain(std::iter::once(0)).collect();
                    wide.shrink_to_fit();
                    let ptr = wide.as_mut_ptr();
                    std::mem::forget(wide);
                    unsafe { *out_name_ptr = ptr; }
                    0
                }
                Err(e) => to_ffi_error(e),
            };
        }

        // Other formats: extract entire archive to temp, then copy target file
        use crate::extract::smart;

        let entries = match smart::smart_list_entries(&archive_path) {
            Ok(e)  => e,
            Err(e) => return to_ffi_error(e),
        };

        let idx = entry_index as usize;
        if idx >= entries.len() {
            return to_ffi_error(zipease_shared::LockError::ExtractionFailed(
                format!("Entry index {} out of range (archive has {} entries)", idx, entries.len())
            ));
        }

        let target_entry = &entries[idx];
        if target_entry.is_directory {
            return to_ffi_error(zipease_shared::LockError::ExtractionFailed(
                "Cannot preview a directory".to_string()
            ));
        }

        let target_name = target_entry.name.clone();

        // Fixed-name temp dir (legacy path — superseded by zip_ease_extract_entry_by_name)
        let temp_dir = std::env::temp_dir().join(format!("zipease_prev_{entry_index}"));
        if let Err(e) = std::fs::create_dir_all(&temp_dir) {
            return to_ffi_error(zipease_shared::LockError::ExtractionFailed(e.to_string()));
        }

        if let Err(e) = crate::extract::extract_direct(&archive_path, &temp_dir, |_, _, _| {}) {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return to_ffi_error(e);
        }

        let src = temp_dir.join(&target_name);
        let file_name = std::path::Path::new(&target_name)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| target_name.clone());
        let dst = output_dir.join(&file_name);

        if let Some(parent) = dst.parent() { let _ = std::fs::create_dir_all(parent); }

        if let Err(e) = std::fs::copy(&src, &dst) {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return to_ffi_error(zipease_shared::LockError::ExtractionFailed(
                format!("Failed to copy extracted file: {e}")
            ));
        }

        let _ = std::fs::remove_dir_all(&temp_dir);

        let mut wide: Vec<u16> = OsString::from(&file_name).encode_wide().chain(std::iter::once(0)).collect();
        wide.shrink_to_fit();
        let ptr = wide.as_mut_ptr();
        std::mem::forget(wide);
        unsafe { *out_name_ptr = ptr; }
        0
    }).unwrap_or(-1)
}

/// Extract a single entry by full path name from any archive format (7z, RAR, TAR, etc.).
/// Uses name-based matching to avoid index offset issues with non-ZIP formats.
///
/// # Parameters
/// - `archive_path_ptr`: UTF-16 null-terminated archive path
/// - `entry_name_ptr`:   UTF-16 null-terminated entry full path (e.g. "folder/sub/file.txt")
/// - `output_dir_ptr`:   UTF-16 null-terminated output directory
/// - `out_name_ptr`:     output parameter; caller MUST free with `zip_ease_free_string`
///
/// # Returns
/// - `0`: success
/// - negative: failure, call `zip_ease_get_last_error` for message
#[no_mangle]
pub extern "C" fn zip_ease_extract_entry_by_name(
    archive_path_ptr: *const u16,
    entry_name_ptr: *const u16,
    output_dir_ptr: *const u16,
    out_name_ptr: *mut *mut u16,
) -> i32 {
    std::panic::catch_unwind(|| {
        if archive_path_ptr.is_null() || entry_name_ptr.is_null()
            || output_dir_ptr.is_null() || out_name_ptr.is_null()
        {
            return -1;
        }
        let archive_path   = unsafe { parse_wide_string(archive_path_ptr) };
        let entry_name     = unsafe { parse_wide_string(entry_name_ptr) };
        let output_dir     = unsafe { parse_wide_string(output_dir_ptr) };
        let entry_name_str = entry_name.to_string_lossy().into_owned();

        match extract::extract_entry_by_name(&archive_path, &entry_name_str, &output_dir) {
            Ok(file_name) => {
                let mut wide: Vec<u16> = OsString::from(&file_name).encode_wide().chain(std::iter::once(0)).collect();
                wide.shrink_to_fit();
                let ptr = wide.as_mut_ptr();
                std::mem::forget(wide);
                unsafe { *out_name_ptr = ptr; }
                0
            }
            Err(e) => to_ffi_error(e),
        }
    }).unwrap_or(-1)
}
