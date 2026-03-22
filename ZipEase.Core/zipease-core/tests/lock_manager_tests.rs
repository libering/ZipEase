use zipease_core::lock::{LockManager, LockHandle};
use std::path::PathBuf;
use tempfile::tempdir;
use std::fs;

#[test]
fn test_lock_and_unlock_flow() {
    let manager = LockManager::new();
    let temp_dir = tempdir().unwrap();
    let path = temp_dir.path().to_path_buf();

    let handle = manager.lock_directory(path.clone()).unwrap();
    assert_eq!(manager.lock_count(), 1);

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

    assert_ne!(handle1, handle2);
    assert_ne!(handle2, handle3);
    assert_ne!(handle1, handle3);
    assert_eq!(manager.lock_count(), 3);

    manager.unlock_directory(handle2).unwrap();
    assert_eq!(manager.lock_count(), 2);

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
    assert_eq!(err.to_error_code(), 6);
}

#[test]
fn test_lock_prevents_deletion() {
    let manager = LockManager::new();
    let temp_dir = tempdir().unwrap();
    let path = temp_dir.path().to_path_buf();

    let _handle = manager.lock_directory(path.clone()).unwrap();

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

    assert!(manager.unlock_directory(handle).is_ok());
    assert!(manager.unlock_directory(handle).is_err());
}
