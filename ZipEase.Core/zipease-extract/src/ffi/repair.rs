use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use std::ptr;

use crate::repair::RepairEngine;
use zipease_shared::parse_wide_string;

/// Progress callback type: fn(current_step: i32, total_steps: i32, entry_name: *const u16)
type RepairProgressCallback = extern "C" fn(i32, i32, *const u16);

/// Diagnose an archive. Returns JSON-encoded DamageReport as a UTF-16 null-terminated string.
/// Caller MUST free the returned pointer with `zip_ease_free_diagnosis`.
/// Returns null on error or panic.
///
/// # Safety
/// `archive_path_ptr` must be a valid null-terminated UTF-16 string pointer, or null.
#[no_mangle]
pub extern "C" fn zip_ease_diagnose_archive(
    archive_path_ptr: *const u16,
) -> *mut u16 {
    std::panic::catch_unwind(|| {
        if archive_path_ptr.is_null() {
            return ptr::null_mut();
        }

        let path = unsafe { parse_wide_string(archive_path_ptr) };

        let report = match RepairEngine::diagnose(&path) {
            Ok(r) => r,
            Err(_) => return ptr::null_mut(),
        };

        let json = match serde_json::to_string(&report) {
            Ok(j) => j,
            Err(_) => return ptr::null_mut(),
        };

        // Encode JSON string as UTF-16 with null terminator
        let mut wide: Vec<u16> = OsString::from(&json)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        wide.shrink_to_fit();
        let ptr = wide.as_mut_ptr();
        std::mem::forget(wide);
        ptr
    })
    .unwrap_or(ptr::null_mut())
}

/// Repair an archive. Writes repaired copy to output_path (or auto-generates path if null).
/// Returns: 0 = full success, 0x2007 = partial success, 0x2006 = not repairable, -1 = error/panic.
///
/// # Safety
/// - `archive_path_ptr` must be a valid null-terminated UTF-16 string pointer, or null.
/// - `output_path_ptr` may be null (auto-generates `_repaired` path) or a valid UTF-16 string.
/// - `progress_callback` may be None (no progress reporting).
#[no_mangle]
pub extern "C" fn zip_ease_repair_archive(
    archive_path_ptr: *const u16,
    output_path_ptr: *const u16,
    progress_callback: Option<RepairProgressCallback>,
) -> i32 {
    std::panic::catch_unwind(|| {
        if archive_path_ptr.is_null() {
            return -1;
        }

        let archive_path = unsafe { parse_wide_string(archive_path_ptr) };

        // Build progress closure that forwards to the C callback
        let progress_fn = move |current: u32, total: u32, entry_name: &str| {
            if let Some(callback) = progress_callback {
                let wide_name: Vec<u16> = OsString::from(entry_name)
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                callback(current as i32, total as i32, wide_name.as_ptr());
            }
        };

        let result = if output_path_ptr.is_null() {
            // Auto-generate output path
            RepairEngine::repair_auto(&archive_path, progress_fn)
        } else {
            let output_path = unsafe { parse_wide_string(output_path_ptr) };
            RepairEngine::repair(&archive_path, &output_path, progress_fn)
        };

        match result {
            Ok(_) => 0,
            Err(ref e) => e.to_ffi_code(),
        }
    })
    .unwrap_or(-1)
}

/// Free the UTF-16 JSON string returned by `zip_ease_diagnose_archive`.
///
/// # Safety
/// `ptr` must have been returned by `zip_ease_diagnose_archive`, or be null.
#[no_mangle]
pub unsafe extern "C" fn zip_ease_free_diagnosis(ptr: *mut u16) {
    let _ = std::panic::catch_unwind(|| {
        if ptr.is_null() {
            return;
        }
        // Walk to find the null terminator to reconstruct the Vec
        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        // Reconstruct Vec including the null terminator, then drop it
        drop(Vec::from_raw_parts(ptr, len + 1, len + 1));
    });
}
