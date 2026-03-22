use crate::extract::smart::smart_list_entries;
use crate::extract::ArchiveEntryInfo;
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

        if password_ptr.is_null() {
            let raw_entries = match smart_list_entries(&archive_path) {
                Ok(v) => v,
                Err(e) => {
                    let code = e.to_error_code();
                    set_last_error(e);
                    return code;
                }
            };
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

        write_ffi_entries(raw_entries, out_entries_ptr, out_count)
    })
    .unwrap_or(-1)
}

/// Free the array previously returned by `zip_ease_list_archive_contents`.
#[no_mangle]
pub extern "C" fn zip_ease_free_archive_entries(entries_ptr: *mut ArchiveEntryFFI, count: i32) {
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
}
