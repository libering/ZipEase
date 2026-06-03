use crate::extract::smart::smart_list_entries;
use crate::extract::ArchiveEntryInfo;
use crate::extract::bomb_detector::{self, BombThresholds};
use zipease_shared::{set_last_error, parse_wide_string, to_utf16_ptr, decode_filename};

/// FFI-safe archive entry returned to C# callers.
#[repr(C)]
pub struct ArchiveEntryFFI {
    /// Heap-allocated UTF-16 null-terminated string (freed by zip_ease_free_archive_entries).
    pub file_name_ptr: *mut u16,
    /// Uncompressed file size in bytes (-1 if unknown).
    pub file_size: i64,
    /// 1 if this entry is a directory, 0 if it is a file.
    pub is_directory: i32,
}

// SAFETY: The raw pointers are heap-allocated and ownership is transferred across the FFI
// boundary. The C# caller must invoke zip_ease_free_archive_entries to release them.
unsafe impl Send for ArchiveEntryFFI {}

/// Convert a `Vec<ArchiveEntryInfo>` into an FFI-safe array and write the pointer/count.
fn write_ffi_entries(
    raw_entries: Vec<ArchiveEntryInfo>,
    out_entries_ptr: *mut *mut ArchiveEntryFFI,
    out_count: *mut i32,
) -> i32 {
    let ffi_entries: Vec<ArchiveEntryFFI> = raw_entries
        .iter()
        .map(|info| {
            let decoded = decode_filename(info.name.as_bytes());
            ArchiveEntryFFI {
                file_name_ptr: to_utf16_ptr(&decoded),
                file_size: info.size,
                is_directory: if info.is_directory { 1 } else { 0 },
            }
        })
        .collect();

    let count = ffi_entries.len() as i32;
    let boxed = ffi_entries.into_boxed_slice();
    let ptr = Box::into_raw(boxed) as *mut ArchiveEntryFFI;

    unsafe {
        *out_entries_ptr = ptr;
        *out_count = count;
    }
    0
}

/// List all entries in an archive file.
#[no_mangle]
pub extern "C" fn zip_ease_list_archive_contents(
    archive_path_ptr: *const u16,
    out_entries_ptr: *mut *mut ArchiveEntryFFI,
    out_count: *mut i32,
) -> i32 {
    std::panic::catch_unwind(|| {
        crate::init_logging();
        if archive_path_ptr.is_null() || out_entries_ptr.is_null() || out_count.is_null() {
            return -1;
        }

        let archive_path = unsafe { parse_wide_string(archive_path_ptr) };

        let raw_entries = match smart_list_entries(&archive_path) {
            Ok(v) => v,
            Err(e) => {
                let code = e.to_error_code();
                set_last_error(e);
                return code;
            }
        };

        // ── Zip Bomb detection ────────────────────────────────────────────────
        let archive_file_size = std::fs::metadata(&archive_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let archive_ext = archive_path.extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();
        if let Err(e) = bomb_detector::check_entries(
            &raw_entries, archive_file_size, &archive_ext, 1, &BombThresholds::default()
        ) {
            let code = e.to_error_code();
            set_last_error(e);
            return code;
        }
        // ─────────────────────────────────────────────────────────────────────

        write_ffi_entries(raw_entries, out_entries_ptr, out_count)
    })
    .unwrap_or(-1)
}

/// List all entries in a password-protected archive.
#[no_mangle]
pub extern "C" fn zip_ease_list_archive_contents_with_password(
    archive_path_ptr: *const u16,
    password_ptr: *const u16,
    out_entries_ptr: *mut *mut ArchiveEntryFFI,
    out_count: *mut i32,
) -> i32 {
    std::panic::catch_unwind(|| {
        if archive_path_ptr.is_null() || out_entries_ptr.is_null() || out_count.is_null() {
            return -1;
        }

        let archive_path = unsafe { parse_wide_string(archive_path_ptr) };

        // Helper: run bomb detection and return error code on detection
        let run_bomb_check = |entries: &[ArchiveEntryInfo]| -> Option<i32> {
            let archive_file_size = std::fs::metadata(&archive_path)
                .map(|m| m.len())
                .unwrap_or(0);
            let archive_ext = archive_path.extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();
            if let Err(e) = bomb_detector::check_entries(
                entries, archive_file_size, &archive_ext, 1, &BombThresholds::default()
            ) {
                let code = e.to_error_code();
                set_last_error(e);
                return Some(code);
            }
            None
        };

        if password_ptr.is_null() {
            let raw_entries = match smart_list_entries(&archive_path) {
                Ok(v) => v,
                Err(e) => {
                    let code = e.to_error_code();
                    set_last_error(e);
                    return code;
                }
            };
            crate::zlog(&format!("[list] with_password(null): {} entries for {:?}",
                raw_entries.len(), archive_path));
            if let Some(code) = run_bomb_check(&raw_entries) {
                return code;
            }
            return write_ffi_entries(raw_entries, out_entries_ptr, out_count);
        }

        let password_path = unsafe { parse_wide_string(password_ptr) };
        let password = password_path.to_string_lossy().into_owned();

        let ext = archive_path.extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        let raw_entries = match ext.as_str() {
            "zip" => {
                use crate::extract::zip::ZipBackend;
                match ZipBackend.list_entries_info_with_password(&archive_path, &password) {
                    Ok(v) => v,
                    Err(e) => {
                        let code = e.to_error_code();
                        set_last_error(e);
                        return code;
                    }
                }
            }
            "7z" => {
                use crate::extract::sevenz::SevenZBackend;
                match SevenZBackend.list_entries_info_with_password(&archive_path, &password) {
                    Ok(v) => v,
                    Err(e) => {
                        let code = e.to_error_code();
                        set_last_error(e);
                        return code;
                    }
                }
            }
            _ => {
                match smart_list_entries(&archive_path) {
                    Ok(v) => v,
                    Err(e) => {
                        let code = e.to_error_code();
                        set_last_error(e);
                        return code;
                    }
                }
            }
        };

        if let Some(code) = run_bomb_check(&raw_entries) {
            return code;
        }

        write_ffi_entries(raw_entries, out_entries_ptr, out_count)
    })
    .unwrap_or(-1)
}

/// Free the array previously returned by `zip_ease_list_archive_contents`.
#[no_mangle]
pub extern "C" fn zip_ease_free_archive_entries(entries_ptr: *mut ArchiveEntryFFI, count: i32) {
    let _ = std::panic::catch_unwind(|| {
        if entries_ptr.is_null() || count <= 0 {
            return;
        }
        unsafe {
            let slice = std::slice::from_raw_parts_mut(entries_ptr, count as usize);
            for entry in slice.iter_mut() {
                if !entry.file_name_ptr.is_null() {
                    let mut len = 0usize;
                    while *entry.file_name_ptr.add(len) != 0 {
                        len += 1;
                    }
                    let _ = Box::from_raw(std::slice::from_raw_parts_mut(
                        entry.file_name_ptr,
                        len + 1,
                    ));
                    entry.file_name_ptr = std::ptr::null_mut();
                }
            }
            let _ = Box::from_raw(std::slice::from_raw_parts_mut(entries_ptr, count as usize));
        }
    });
}

/// List archive contents with custom Zip Bomb detection thresholds.
///
/// Allows C# callers to pass user-configured thresholds from AppSettings.
/// GB values are converted to bytes internally.
#[no_mangle]
pub extern "C" fn zip_ease_list_archive_contents_with_thresholds(
    archive_path_ptr: *const u16,
    password_ptr: *const u16,
    max_compression_ratio: f64,
    max_total_gb: f64,
    max_single_entry_gb: f64,
    max_nesting_depth: u32,
    out_entries_ptr: *mut *mut ArchiveEntryFFI,
    out_count: *mut i32,
) -> i32 {
    std::panic::catch_unwind(|| {
        if archive_path_ptr.is_null() || out_entries_ptr.is_null() || out_count.is_null() {
            return -1;
        }

        let archive_path = unsafe { parse_wide_string(archive_path_ptr) };

        // Build custom thresholds from the scalar parameters (GB → bytes)
        let thresholds = BombThresholds {
            max_compression_ratio,
            max_total_uncompressed_bytes: (max_total_gb * 1_073_741_824.0) as u64,
            max_single_entry_bytes: (max_single_entry_gb * 1_073_741_824.0) as u64,
            max_nesting_depth,
            exempt_formats: vec!["iso".to_string()],
        };

        // Determine password
        let password: Option<String> = if password_ptr.is_null() {
            None
        } else {
            let pw_path = unsafe { parse_wide_string(password_ptr) };
            Some(pw_path.to_string_lossy().into_owned())
        };

        let ext = archive_path.extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        let raw_entries = match (ext.as_str(), &password) {
            ("zip", Some(pw)) => {
                use crate::extract::zip::ZipBackend;
                match ZipBackend.list_entries_info_with_password(&archive_path, pw) {
                    Ok(v) => v,
                    Err(e) => {
                        let code = e.to_error_code();
                        set_last_error(e);
                        return code;
                    }
                }
            }
            ("7z", Some(pw)) => {
                use crate::extract::sevenz::SevenZBackend;
                match SevenZBackend.list_entries_info_with_password(&archive_path, pw) {
                    Ok(v) => v,
                    Err(e) => {
                        let code = e.to_error_code();
                        set_last_error(e);
                        return code;
                    }
                }
            }
            _ => {
                match smart_list_entries(&archive_path) {
                    Ok(v) => v,
                    Err(e) => {
                        let code = e.to_error_code();
                        set_last_error(e);
                        return code;
                    }
                }
            }
        };

        // Run bomb detection with custom thresholds
        let archive_file_size = std::fs::metadata(&archive_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let archive_ext = archive_path.extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();
        if let Err(e) = bomb_detector::check_entries(
            &raw_entries, archive_file_size, &archive_ext, 1, &thresholds
        ) {
            let code = e.to_error_code();
            set_last_error(e);
            return code;
        }

        write_ffi_entries(raw_entries, out_entries_ptr, out_count)
    })
    .unwrap_or(-1)
}
