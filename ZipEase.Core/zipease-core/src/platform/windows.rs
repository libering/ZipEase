use zipease_shared::LockError;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, OPEN_EXISTING,
};

#[derive(Debug)]
pub struct WindowsDirectoryLock {
    handle: HANDLE,
}

impl WindowsDirectoryLock {
    pub fn lock<P: AsRef<Path>>(path: P) -> Result<Self, LockError> {
        let path_ref = path.as_ref();
        if path_ref.as_os_str().is_empty() {
            return Err(LockError::InvalidPath("Path is empty".to_string()));
        }
        let path_wide: Vec<u16> = path_ref
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let handle = unsafe {
            CreateFileW(
                PCWSTR(path_wide.as_ptr()),
                0x80000000,
                FILE_SHARE_READ,
                None,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                HANDLE::default(),
            )
        };
        match handle {
            Ok(h) if h != INVALID_HANDLE_VALUE => Ok(Self { handle: h }),
            _ => {
                let error_code = unsafe { GetLastError() };
                let path_str = path_ref.display().to_string();
                let lock_error = match error_code.0 {
                    2 | 3 => LockError::PathNotFound(path_str),
                    5 => LockError::AccessDenied(path_str),
                    32 => LockError::SharingViolation(path_str),
                    123 => LockError::InvalidPath(path_str),
                    _ => LockError::Unknown(format!(
                        "Failed to lock directory '{}': Windows error code {}",
                        path_str, error_code.0
                    )),
                };
                Err(lock_error)
            }
        }
    }

    pub fn as_raw_handle(&self) -> isize {
        self.handle.0 as isize
    }

    pub fn is_valid(&self) -> bool {
        self.handle != INVALID_HANDLE_VALUE && !self.handle.is_invalid()
    }
}

impl Drop for WindowsDirectoryLock {
    fn drop(&mut self) {
        if self.is_valid() {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}
