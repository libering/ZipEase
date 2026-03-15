// Lock manager implementation
// Manages multiple directory locks with unique handles

use crate::error::LockError;
use crate::lock::handle::LockHandle;
use crate::platform::WindowsDirectoryLock;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Wrapper for WindowsDirectoryLock that implements Send
/// 
/// # Safety
/// This is safe because:
/// 1. We only access the lock through the LockManager's mutex
/// 2. Locks are properly cleaned up when dropped
/// 3. The FFI layer ensures proper synchronization
struct SendableLock(WindowsDirectoryLock);

unsafe impl Send for SendableLock {}

impl SendableLock {
    fn new(lock: WindowsDirectoryLock) -> Self {
        Self(lock)
    }
}

impl std::ops::Deref for SendableLock {
    type Target = WindowsDirectoryLock;
    
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Manages multiple directory locks
/// 
/// The LockManager maintains a registry of active locks, assigning each
/// a unique handle. It provides thread-safe operations for locking and
/// unlocking directories.
/// 
/// # Thread Safety
/// All operations are thread-safe through the use of Arc<Mutex<>>.
pub struct LockManager {
    /// Map of handles to locks and their paths
    locks: Arc<Mutex<HashMap<LockHandle, (SendableLock, PathBuf)>>>,
    /// Counter for generating unique handle IDs
    next_id: Arc<Mutex<u64>>,
}

impl LockManager {
    /// Creates a new LockManager
    /// 
    /// # Returns
    /// A new LockManager instance with empty lock registry
    /// 
    /// # Example
    /// ```
    /// use zipease_core::lock::LockManager;
    /// 
    /// let manager = LockManager::new();
    /// ```
    pub fn new() -> Self {
        Self {
            locks: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(1)), // Start from 1, 0 is invalid
        }
    }
    
    /// Locks a directory and returns a unique handle
    /// 
    /// This function attempts to lock the specified directory using the
    /// Windows API. If successful, it assigns a unique handle and stores
    /// the lock in the registry.
    /// 
    /// # Arguments
    /// * `path` - The path to the directory to lock
    /// 
    /// # Returns
    /// * `Ok(LockHandle)` - Successfully locked, returns the handle
    /// * `Err(LockError)` - Failed to lock the directory
    /// 
    /// # Errors
    /// Returns the same errors as `WindowsDirectoryLock::lock()`
    /// 
    /// # Example
    /// ```no_run
    /// use zipease_core::lock::LockManager;
    /// use std::path::PathBuf;
    /// 
    /// let manager = LockManager::new();
    /// let path = PathBuf::from("C:\\temp\\test");
    /// 
    /// match manager.lock_directory(path) {
    ///     Ok(handle) => println!("Locked with handle: {:?}", handle),
    ///     Err(e) => eprintln!("Failed to lock: {}", e.message()),
    /// }
    /// ```
    pub fn lock_directory(&self, path: PathBuf) -> Result<LockHandle, LockError> {
        // Attempt to lock the directory
        let lock = WindowsDirectoryLock::lock(&path)?;
        let sendable_lock = SendableLock::new(lock);
        
        // Generate a unique handle ID
        let handle_id = {
            let mut next_id = self.next_id.lock().unwrap();
            let id = *next_id;
            *next_id += 1;
            id
        };
        
        let handle = LockHandle::new(handle_id);
        
        // Store the lock in the registry
        {
            let mut locks = self.locks.lock().unwrap();
            locks.insert(handle, (sendable_lock, path));
        }
        
        Ok(handle)
    }
    
    /// Unlocks a directory and removes it from the registry
    /// 
    /// This function releases the lock associated with the given handle
    /// and removes it from the registry. The lock is automatically closed
    /// when dropped.
    /// 
    /// # Arguments
    /// * `handle` - The handle of the lock to release
    /// 
    /// # Returns
    /// * `Ok(())` - Successfully unlocked
    /// * `Err(LockError::InvalidHandle)` - The handle is invalid or not found
    /// 
    /// # Example
    /// ```no_run
    /// use zipease_core::lock::LockManager;
    /// use std::path::PathBuf;
    /// 
    /// let manager = LockManager::new();
    /// let path = PathBuf::from("C:\\temp\\test");
    /// let handle = manager.lock_directory(path).unwrap();
    /// 
    /// // Later...
    /// manager.unlock_directory(handle).unwrap();
    /// ```
    pub fn unlock_directory(&self, handle: LockHandle) -> Result<(), LockError> {
        let mut locks = self.locks.lock().unwrap();
        
        // Remove the lock from the registry
        // The lock will be automatically closed when dropped
        if locks.remove(&handle).is_some() {
            Ok(())
        } else {
            Err(LockError::InvalidHandle)
        }
    }
    
    /// Returns the number of currently held locks
    /// 
    /// This is primarily useful for debugging and testing.
    /// 
    /// # Returns
    /// The number of active locks
    /// 
    /// # Example
    /// ```no_run
    /// use zipease_core::lock::LockManager;
    /// use std::path::PathBuf;
    /// 
    /// let manager = LockManager::new();
    /// assert_eq!(manager.lock_count(), 0);
    /// 
    /// let handle = manager.lock_directory(PathBuf::from("C:\\temp\\test")).unwrap();
    /// assert_eq!(manager.lock_count(), 1);
    /// 
    /// manager.unlock_directory(handle).unwrap();
    /// assert_eq!(manager.lock_count(), 0);
    /// ```
    pub fn lock_count(&self) -> usize {
        self.locks.lock().unwrap().len()
    }
}

impl Default for LockManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global singleton instance of LockManager
/// 
/// This is used by the FFI layer to manage locks across the C# boundary.
pub static LOCK_MANAGER: Lazy<LockManager> = Lazy::new(LockManager::new);

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_new_manager() {
        let manager = LockManager::new();
        assert_eq!(manager.lock_count(), 0);
    }

    #[test]
    fn test_lock_directory() {
        let manager = LockManager::new();
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().to_path_buf();
        
        let result = manager.lock_directory(path);
        assert!(result.is_ok());
        assert_eq!(manager.lock_count(), 1);
    }

    #[test]
    fn test_unlock_directory() {
        let manager = LockManager::new();
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().to_path_buf();
        
        let handle = manager.lock_directory(path).unwrap();
        assert_eq!(manager.lock_count(), 1);
        
        let result = manager.unlock_directory(handle);
        assert!(result.is_ok());
        assert_eq!(manager.lock_count(), 0);
    }

    #[test]
    fn test_unlock_invalid_handle() {
        let manager = LockManager::new();
        let invalid_handle = LockHandle::new(999);
        
        let result = manager.unlock_directory(invalid_handle);
        assert!(result.is_err());
        
        let err = result.unwrap_err();
        assert_eq!(err.to_error_code(), 6); // ERROR_INVALID_HANDLE
    }

    #[test]
    fn test_multiple_locks() {
        let manager = LockManager::new();
        
        let temp_dir1 = tempdir().unwrap();
        let temp_dir2 = tempdir().unwrap();
        let temp_dir3 = tempdir().unwrap();
        
        let handle1 = manager.lock_directory(temp_dir1.path().to_path_buf()).unwrap();
        let handle2 = manager.lock_directory(temp_dir2.path().to_path_buf()).unwrap();
        let handle3 = manager.lock_directory(temp_dir3.path().to_path_buf()).unwrap();
        
        assert_eq!(manager.lock_count(), 3);
        
        // Handles should be unique
        assert_ne!(handle1, handle2);
        assert_ne!(handle2, handle3);
        assert_ne!(handle1, handle3);
        
        // Unlock one
        manager.unlock_directory(handle2).unwrap();
        assert_eq!(manager.lock_count(), 2);
        
        // Other locks should still be valid
        manager.unlock_directory(handle1).unwrap();
        manager.unlock_directory(handle3).unwrap();
        assert_eq!(manager.lock_count(), 0);
    }

    #[test]
    fn test_lock_nonexistent_directory() {
        let manager = LockManager::new();
        let path = PathBuf::from("C:\\nonexistent_directory_12345");
        
        let result = manager.lock_directory(path);
        assert!(result.is_err());
        assert_eq!(manager.lock_count(), 0);
    }

    #[test]
    fn test_double_unlock() {
        let manager = LockManager::new();
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().to_path_buf();
        
        let handle = manager.lock_directory(path).unwrap();
        
        // First unlock should succeed
        assert!(manager.unlock_directory(handle).is_ok());
        
        // Second unlock should fail
        assert!(manager.unlock_directory(handle).is_err());
    }

    #[test]
    fn test_global_lock_manager() {
        // Test that the global instance works
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().to_path_buf();
        
        let handle = LOCK_MANAGER.lock_directory(path).unwrap();
        assert!(LOCK_MANAGER.lock_count() >= 1);
        
        LOCK_MANAGER.unlock_directory(handle).unwrap();
    }
}
