use zipease_shared::{set_last_error, clear_last_error, LockError};
use crate::lock::LockHandle;
use crate::lock::manager::LOCK_MANAGER;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

#[no_mangle]
pub extern "C" fn zip_ease_lock_directory(path: *const u16) -> isize {
    std::panic::catch_unwind(|| {
        if path.is_null() {
            set_last_error(LockError::InvalidPath("Null pointer".into()));
            return LockHandle::INVALID;
        }
        let len = unsafe {
            let mut len = 0usize;
            let mut p = path;
            while *p != 0 { len += 1; p = p.add(1); }
            len
        };
        let os_string = unsafe {
            let slice = std::slice::from_raw_parts(path, len);
            OsString::from_wide(slice)
        };
        let path_buf = PathBuf::from(os_string);
        if path_buf.as_os_str().is_empty() {
            set_last_error(LockError::InvalidPath("Empty path".into()));
            return LockHandle::INVALID;
        }
        match LOCK_MANAGER.lock_directory(path_buf) {
            Ok(handle) => {
                clear_last_error();
                handle.as_raw()
            }
            Err(err) => {
                set_last_error(err);
                LockHandle::INVALID
            }
        }
    })
    .unwrap_or_else(|_| {
        set_last_error(LockError::Unknown("panic in zip_ease_lock_directory".into()));
        LockHandle::INVALID
    })
}

#[no_mangle]
pub extern "C" fn zip_ease_unlock_directory(handle: isize) -> i32 {
    std::panic::catch_unwind(|| {
        let lock_handle = match LockHandle::from_raw(handle) {
            Some(h) => h,
            None => {
                set_last_error(LockError::InvalidHandle);
                return LockError::InvalidHandle.to_error_code();
            }
        };
        match LOCK_MANAGER.unlock_directory(lock_handle) {
            Ok(_) => 0,
            Err(err) => {
                let code = err.to_error_code();
                set_last_error(err);
                code
            }
        }
    })
    .unwrap_or_else(|_| {
        set_last_error(LockError::Unknown("panic in zip_ease_unlock_directory".into()));
        -1
    })
}
