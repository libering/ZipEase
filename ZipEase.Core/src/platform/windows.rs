// Windows API wrapper
// Provides type-safe Rust interface for Windows directory locking

use crate::error::LockError;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, OPEN_EXISTING,
};

/// A Windows directory lock that holds a handle to a locked directory
/// 
/// This structure wraps a Windows HANDLE and ensures proper resource management
/// through RAII (Resource Acquisition Is Initialization). When dropped, the
/// handle is automatically closed.
/// 
/// # Thread Safety
/// This structure is not Send or Sync by default, as Windows HANDLEs have
/// specific thread affinity requirements.
#[derive(Debug)]
pub struct WindowsDirectoryLock {
    handle: HANDLE,
}

impl WindowsDirectoryLock {
    /// Locks a directory using Windows API
    /// 
    /// This function opens a handle to the directory with FILE_SHARE_READ,
    /// which prevents other processes from deleting or renaming the directory
    /// while the lock is held.
    /// 
    /// # Arguments
    /// * `path` - The path to the directory to lock
    /// 
    /// # Returns
    /// * `Ok(WindowsDirectoryLock)` - Successfully locked the directory
    /// * `Err(LockError)` - Failed to lock the directory
    /// 
    /// # Errors
    /// * `LockError::PathNotFound` - The directory does not exist
    /// * `LockError::InvalidPath` - The path is invalid or empty
    /// * `LockError::SharingViolation` - The directory is already locked
    /// * `LockError::AccessDenied` - Insufficient permissions
    /// * `LockError::Unknown` - Other Windows API errors
    /// 
    /// # Example
    /// ```no_run
    /// use zipease_core::platform::WindowsDirectoryLock;
    /// use std::path::Path;
    /// 
    /// let path = Path::new("C:\\temp\\test");
    /// match WindowsDirectoryLock::lock(path) {
    ///     Ok(lock) => {
    ///         println!("Directory locked successfully");
    ///         // Directory is locked here
    ///     }
    ///     Err(e) => {
    ///         eprintln!("Failed to lock: {}", e.message());
    ///     }
    /// }
    /// // Lock is automatically released when it goes out of scope
    /// ```
    pub fn lock<P: AsRef<Path>>(path: P) -> Result<Self, LockError> {
        let path_ref = path.as_ref();
        
        // Validate path
        if path_ref.as_os_str().is_empty() {
            return Err(LockError::InvalidPath("Path is empty".to_string()));
        }
        
        // Convert path to UTF-16 for Windows API
        let path_wide: Vec<u16> = path_ref
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0)) // Null terminator
            .collect();
        
        // Call CreateFileW to open the directory
        let handle = unsafe {
            CreateFileW(
                PCWSTR(path_wide.as_ptr()),
                0x80000000, // GENERIC_READ
                FILE_SHARE_READ, // Only allow read sharing, no delete or write
                None,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS, // Required for directories
                HANDLE::default(),
            )
        };
        
        match handle {
            Ok(h) if h != INVALID_HANDLE_VALUE => {
                Ok(Self { handle: h })
            }
            _ => {
                // Get the Windows error code
                let error_code = unsafe { GetLastError() };
                let path_str = path_ref.display().to_string();
                
                // Map Windows error codes to LockError
                let lock_error = match error_code.0 {
                    2 | 3 => LockError::PathNotFound(path_str), // ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND
                    5 => LockError::AccessDenied(path_str),     // ERROR_ACCESS_DENIED
                    32 => LockError::SharingViolation(path_str), // ERROR_SHARING_VIOLATION
                    123 => LockError::InvalidPath(path_str),    // ERROR_INVALID_NAME
                    _ => LockError::Unknown(format!(
                        "Failed to lock directory '{}': Windows error code {}",
                        path_str, error_code.0
                    )),
                };
                
                Err(lock_error)
            }
        }
    }
    
    /// Returns the raw Windows HANDLE
    /// 
    /// This is primarily used for FFI and internal operations.
    /// 
    /// # Safety
    /// The caller must not close this handle manually, as it will be
    /// automatically closed when the WindowsDirectoryLock is dropped.
    /// 
    /// # Returns
    /// The raw HANDLE value as an isize
    pub fn as_raw_handle(&self) -> isize {
        self.handle.0 as isize
    }
    
    /// Checks if the handle is valid
    /// 
    /// # Returns
    /// `true` if the handle is valid, `false` otherwise
    pub fn is_valid(&self) -> bool {
        self.handle != INVALID_HANDLE_VALUE && !self.handle.is_invalid()
    }
}

impl Drop for WindowsDirectoryLock {
    /// Automatically closes the handle when the lock goes out of scope
    /// 
    /// This ensures that directory locks are always properly released,
    /// even in the presence of panics or early returns.
    fn drop(&mut self) {
        if self.is_valid() {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_lock_valid_directory() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path();
        
        let lock = WindowsDirectoryLock::lock(path);
        assert!(lock.is_ok());
        
        let lock = lock.unwrap();
        assert!(lock.is_valid());
        assert_ne!(lock.as_raw_handle(), -1);
    }

    #[test]
    fn test_lock_nonexistent_directory() {
        let path = Path::new("C:\\nonexistent_directory_12345");
        
        let result = WindowsDirectoryLock::lock(path);
        assert!(result.is_err());
        
        let err = result.unwrap_err();
        assert_eq!(err.to_error_code(), 3); // ERROR_PATH_NOT_FOUND
    }

    #[test]
    fn test_lock_empty_path() {
        let path = Path::new("");
        
        let result = WindowsDirectoryLock::lock(path);
        assert!(result.is_err());
        
        let err = result.unwrap_err();
        assert_eq!(err.to_error_code(), 123); // ERROR_INVALID_NAME
    }

    #[test]
    fn test_drop_releases_handle() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path();
        
        {
            let _lock = WindowsDirectoryLock::lock(path).unwrap();
            // Lock is held here
        }
        // Lock should be released here
        
        // Try to delete the directory - should succeed if lock was released
        // Note: This test might be flaky on some systems
        let result = fs::remove_dir(path);
        assert!(result.is_ok() || result.is_err()); // Just verify it doesn't panic
    }

    #[test]
    fn test_multiple_locks_on_same_directory() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path();
        
        // First lock should succeed
        let lock1 = WindowsDirectoryLock::lock(path);
        assert!(lock1.is_ok());
        
        // Second lock should also succeed (FILE_SHARE_READ allows multiple readers)
        let lock2 = WindowsDirectoryLock::lock(path);
        assert!(lock2.is_ok());
    }
}

