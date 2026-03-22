use zipease_shared::{LockError, set_last_error, get_last_error, clear_last_error};
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn test_error_code_mapping_correctness() {
    assert_eq!(LockError::PathNotFound("test".into()).to_error_code(), 3);
    assert_eq!(LockError::InvalidPath("test".into()).to_error_code(), 123);
    assert_eq!(LockError::SharingViolation("test".into()).to_error_code(), 32);
    assert_eq!(LockError::AccessDenied("test".into()).to_error_code(), 5);
    assert_eq!(LockError::InvalidHandle.to_error_code(), 6);
    assert_eq!(LockError::Unknown("test".into()).to_error_code(), -1);
}

#[test]
fn test_error_messages_contain_context() {
    let path = "C:\\test\\directory";

    let err = LockError::PathNotFound(path.into());
    assert!(err.message().contains("Path not found"));
    assert!(err.message().contains(path));

    let err = LockError::InvalidPath(path.into());
    assert!(err.message().contains("Invalid path"));
    assert!(err.message().contains(path));

    let err = LockError::SharingViolation(path.into());
    assert!(err.message().contains("locked"));
    assert!(err.message().contains(path));

    let err = LockError::AccessDenied(path.into());
    assert!(err.message().contains("Access denied"));
    assert!(err.message().contains(path));

    let err = LockError::InvalidHandle;
    assert!(err.message().contains("Invalid handle"));

    let custom_msg = "custom error details";
    let err = LockError::Unknown(custom_msg.into());
    assert!(err.message().contains("Unknown error"));
    assert!(err.message().contains(custom_msg));
}

#[test]
fn test_error_storage_basic_operations() {
    clear_last_error();
    assert!(get_last_error().is_none());

    let error = LockError::PathNotFound("test_path".into());
    set_last_error(error.clone());

    let retrieved = get_last_error();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().to_error_code(), error.to_error_code());

    clear_last_error();
    assert!(get_last_error().is_none());
}

#[test]
fn test_error_storage_overwrite() {
    clear_last_error();

    set_last_error(LockError::PathNotFound("path1".into()));
    let error2 = LockError::AccessDenied("path2".into());
    set_last_error(error2.clone());

    let retrieved = get_last_error().unwrap();
    assert_eq!(retrieved.to_error_code(), error2.to_error_code());
    assert!(retrieved.message().contains("path2"));
}

#[test]
fn test_error_storage_multithreaded() {
    clear_last_error();

    let counter = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    for i in 0..10 {
        let counter_clone = Arc::clone(&counter);
        let handle = thread::spawn(move || {
            set_last_error(LockError::PathNotFound(format!("path_{}", i)));
            counter_clone.fetch_add(1, Ordering::SeqCst);
            assert!(get_last_error().is_some());
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(counter.load(Ordering::SeqCst), 10);
    assert!(get_last_error().is_some());
}
