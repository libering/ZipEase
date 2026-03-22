use crate::notify::toast::{notify_failure, notify_success, ptr_to_string};

/// FFI export: dispatch a success toast notification.
/// Wraps the entire body in `catch_unwind` — a Rust panic must never cross the FFI boundary
/// (Requirement 5.4, 5.5).
#[no_mangle]
pub extern "C" fn zip_ease_notify_success(
    archive_name: *const u16,
    output_folder: *const u16,
    file_count: i32,
) {
    let _ = std::panic::catch_unwind(|| {
        let name = ptr_to_string(archive_name);
        let folder = ptr_to_string(output_folder);
        notify_success(&name, &folder, file_count);
    });
}

/// FFI export: dispatch a failure toast notification.
/// Wraps the entire body in `catch_unwind` — a Rust panic must never cross the FFI boundary
/// (Requirement 5.4, 5.5).
#[no_mangle]
pub extern "C" fn zip_ease_notify_failure(
    archive_name: *const u16,
    error_msg: *const u16,
) {
    let _ = std::panic::catch_unwind(|| {
        let name = ptr_to_string(archive_name);
        let msg = ptr_to_string(error_msg);
        notify_failure(&name, &msg);
    });
}
