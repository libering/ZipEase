// Platform layer tests
// Tests for Windows API wrapper

use zipease_core::platform::WindowsDirectoryLock;
use std::path::Path;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_lock_and_prevent_deletion() {
    let temp_dir = tempdir().unwrap();
    let path = temp_dir.path();
    
    // Lock the directory
    let _lock = WindowsDirectoryLock::lock(path).unwrap();
    
    // Try to delete - should fail while locked
    let result = fs::remove_dir(path);
    assert!(result.is_err(), "Directory should not be deletable while locked");
}

#[test]
fn test_lock_release_allows_deletion() {
    let temp_dir = tempdir().unwrap();
    let path = temp_dir.path().to_path_buf();
    
    {
        let _lock = WindowsDirectoryLock::lock(&path).unwrap();
        // Lock is held here
    }
    // Lock released here
    
    // Now deletion should succeed
    let result = fs::remove_dir(&path);
    assert!(result.is_ok(), "Directory should be deletable after lock is released");
}

#[test]
fn test_lock_nonexistent_path_returns_error() {
    let path = Path::new("C:\\this_path_definitely_does_not_exist_12345");
    
    let result = WindowsDirectoryLock::lock(path);
    assert!(result.is_err());
    
    let err = result.unwrap_err();
    // Should be PathNotFound error (code 3)
    assert_eq!(err.to_error_code(), 3);
    assert!(err.message().contains("Path not found"));
}

#[test]
fn test_lock_empty_path_returns_error() {
    let path = Path::new("");
    
    let result = WindowsDirectoryLock::lock(path);
    assert!(result.is_err());
    
    let err = result.unwrap_err();
    // Should be InvalidPath error (code 123)
    assert_eq!(err.to_error_code(), 123);
    assert!(err.message().contains("Invalid path"));
}

#[test]
fn test_lock_valid_handle() {
    let temp_dir = tempdir().unwrap();
    let path = temp_dir.path();
    
    let lock = WindowsDirectoryLock::lock(path).unwrap();
    
    // Handle should be valid
    assert!(lock.is_valid());
    
    // Raw handle should not be -1 (INVALID_HANDLE_VALUE)
    assert_ne!(lock.as_raw_handle(), -1);
}

#[test]
fn test_multiple_read_locks_allowed() {
    let temp_dir = tempdir().unwrap();
    let path = temp_dir.path();
    
    // First lock
    let lock1 = WindowsDirectoryLock::lock(path);
    assert!(lock1.is_ok());
    
    // Second lock should also succeed (FILE_SHARE_READ allows multiple readers)
    let lock2 = WindowsDirectoryLock::lock(path);
    assert!(lock2.is_ok());
    
    // Both locks should be valid
    assert!(lock1.unwrap().is_valid());
    assert!(lock2.unwrap().is_valid());
}

#[test]
fn test_lock_with_unicode_path() {
    let temp_dir = tempdir().unwrap();
    let unicode_subdir = temp_dir.path().join("測試目錄");
    
    // Create directory with Unicode name
    fs::create_dir(&unicode_subdir).unwrap();
    
    // Should be able to lock it
    let lock = WindowsDirectoryLock::lock(&unicode_subdir);
    assert!(lock.is_ok());
    assert!(lock.unwrap().is_valid());
}

#[test]
fn test_drop_is_called_automatically() {
    let temp_dir = tempdir().unwrap();
    let path = temp_dir.path().to_path_buf();
    
    // Create a scope where lock is created and dropped
    {
        let _lock = WindowsDirectoryLock::lock(&path).unwrap();
        // Lock exists here
    }
    // Lock should be dropped and handle closed here
    
    // Verify we can perform operations that require the lock to be released
    // (This is implicit - if drop wasn't called, we'd have a handle leak)
}
