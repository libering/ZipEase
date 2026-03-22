use crate::trash::trash_file;

/// Move a file to the Windows Recycle Bin.
///
/// # Safety
/// `path_ptr` must be a valid null-terminated UTF-16 string, or null.
///
/// # Returns
/// - `0` on success
/// - `-1` if a Rust panic occurred (host process is safe)
/// - `-2` on any other error (file not found, permission denied, in use, etc.)
#[no_mangle]
pub extern "C" fn zip_ease_trash_file(path_ptr: *const u16) -> i32 {
    std::panic::catch_unwind(|| {
        match trash_file(path_ptr) {
            Ok(()) => 0,
            Err(_) => -2,
        }
    })
    .unwrap_or(-1)
}
