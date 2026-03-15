// Lock manager integration tests

use zipease_core::lock::{LockManager, LockHandle};
use std::path::PathBuf;
use tempfile::tempdir;
use std::fs;

#[test]
fn test_lock_and_unlock_flow() {
    let manager = LockManager::new();
    let temp_dir = tempdir().unwrap();
    let path = temp_dir.path().to_path_buf();
    
    // Lock
    let handle = manager.lock_directory(path.clone()).unwrap();
    assert_eq!(manager.lock_count(), 1);
    
    // Unlock
    manager.unlock_directory(handle).unwrap();
    assert_eq!(manager.lock_count(), 0);
}

#[test]
fn test_multiple_independent_locks() {
    let manager = LockManager::new();
    
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();
    let dir3 = tempdir().unwrap();
    
    let handle1 = manager.lock_directory(dir1.path().to_path_buf()).unwrap();
    let handle2 = manager.lock_directory(dir2.path().to_path_buf()).unwrap();
    let handle3 = manager.lock_directory(dir3.path().to_path_buf()).unwrap();
    
    // All handles should be unique
    assert_ne!(handle1, handle2);
    assert_ne!(handle2, handle3);
    assert_ne!(handle1, handle3);
    
    assert_eq!(manager.lock_count(), 3);
    
    // Unlock middle one
    manager.unlock_directory(handle2).unwrap();
    assert_eq!(manager.lock_count(), 2);
    
    // Others should still work
    manager.unlock_directory(handle1).unwrap();
    manager.unlock_directory(handle3).unwrap();
    assert_eq!(manager.lock_count(), 0);
}

#[test]
fn test_unlock_invalid_handle_returns_error() {
    let manager = LockManager::new();
    let invalid_handle = LockHandle::new(99999);
    
    let result = manager.unlock_directory(invalid_handle);
    assert!(result.is_err());
    
    let err = result.unwrap_err();
    assert_eq!(err.to_error_code(), 6); // ERROR_INVALID_HANDLE
}

#[test]
fn test_lock_prevents_deletion() {
    let manager = LockManager::new();
    let temp_dir = tempdir().unwrap();
    let path = temp_dir.path().to_path_buf();
    
    let _handle = manager.lock_directory(path.clone()).unwrap();
    
    // Try to delete - should fail
    let result = fs::remove_dir(&path);
    assert!(result.is_err());
}

#[test]
fn test_unlock_allows_deletion() {
    let manager = LockManager::new();
    let temp_dir = tempdir().unwrap();
    let path = temp_dir.path().to_path_buf();
    
    let handle = manager.lock_directory(path.clone()).unwrap();
    manager.unlock_directory(handle).unwrap();
    
    // Now deletion should work
    let result = fs::remove_dir(&path);
    assert!(result.is_ok());
}

#[test]
fn test_lock_nonexistent_directory_fails() {
    let manager = LockManager::new();
    let path = PathBuf::from("C:\\nonexistent_test_dir_12345");
    
    let result = manager.lock_directory(path);
    assert!(result.is_err());
    assert_eq!(manager.lock_count(), 0);
}

#[test]
fn test_double_unlock_fails() {
    let manager = LockManager::new();
    let temp_dir = tempdir().unwrap();
    let path = temp_dir.path().to_path_buf();
    
    let handle = manager.lock_directory(path).unwrap();
    
    // First unlock succeeds
    assert!(manager.unlock_directory(handle).is_ok());
    
    // Second unlock fails
    assert!(manager.unlock_directory(handle).is_err());
}

#[test]
fn test_lock_count_accuracy() {
    let manager = LockManager::new();
    assert_eq!(manager.lock_count(), 0);
    
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();
    
    let h1 = manager.lock_directory(dir1.path().to_path_buf()).unwrap();
    assert_eq!(manager.lock_count(), 1);
    
    let h2 = manager.lock_directory(dir2.path().to_path_buf()).unwrap();
    assert_eq!(manager.lock_count(), 2);
    
    manager.unlock_directory(h1).unwrap();
    assert_eq!(manager.lock_count(), 1);
    
    manager.unlock_directory(h2).unwrap();
    assert_eq!(manager.lock_count(), 0);
}
