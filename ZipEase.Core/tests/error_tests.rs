// Error handling unit tests
// Tests for error types and error storage functionality

mod unit;

use zipease_core::error::{LockError, set_last_error, get_last_error, clear_last_error};
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn test_error_code_mapping_correctness() {
    // Test that all error types map to correct Windows error codes
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
    
    // PathNotFound should include the path
    let err = LockError::PathNotFound(path.into());
    let msg = err.message();
    assert!(msg.contains("Path not found"), "Message: {}", msg);
    assert!(msg.contains(path), "Message: {}", msg);
    
    // InvalidPath should include the path
    let err = LockError::InvalidPath(path.into());
    let msg = err.message();
    assert!(msg.contains("Invalid path"), "Message: {}", msg);
    assert!(msg.contains(path), "Message: {}", msg);
    
    // SharingViolation should include the path
    let err = LockError::SharingViolation(path.into());
    let msg = err.message();
    assert!(msg.contains("locked"), "Message: {}", msg);
    assert!(msg.contains(path), "Message: {}", msg);
    
    // AccessDenied should include the path
    let err = LockError::AccessDenied(path.into());
    let msg = err.message();
    assert!(msg.contains("Access denied"), "Message: {}", msg);
    assert!(msg.contains(path), "Message: {}", msg);
    
    // InvalidHandle should have a clear message
    let err = LockError::InvalidHandle;
    let msg = err.message();
    assert!(msg.contains("Invalid handle"), "Message: {}", msg);
    
    // Unknown should include the custom message
    let custom_msg = "custom error details";
    let err = LockError::Unknown(custom_msg.into());
    let msg = err.message();
    assert!(msg.contains("Unknown error"), "Message: {}", msg);
    assert!(msg.contains(custom_msg), "Message: {}", msg);
}

#[test]
fn test_error_storage_basic_operations() {
    // Clear any previous errors
    clear_last_error();
    assert!(get_last_error().is_none());
    
    // Set an error
    let error = LockError::PathNotFound("test_path".into());
    set_last_error(error.clone());
    
    // Retrieve the error
    let retrieved = get_last_error();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().to_error_code(), error.to_error_code());
    
    // Clear the error
    clear_last_error();
    assert!(get_last_error().is_none());
}

#[test]
fn test_error_storage_overwrite() {
    clear_last_error();
    
    // Set first error
    let error1 = LockError::PathNotFound("path1".into());
    set_last_error(error1);
    
    // Set second error (should overwrite)
    let error2 = LockError::AccessDenied("path2".into());
    set_last_error(error2.clone());
    
    // Should retrieve the second error
    let retrieved = get_last_error().unwrap();
    assert_eq!(retrieved.to_error_code(), error2.to_error_code());
    assert!(retrieved.message().contains("path2"));
}

#[test]
fn test_error_storage_multithreaded() {
    // Test that error storage is thread-safe
    clear_last_error();
    
    let counter = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];
    
    // Spawn multiple threads that set errors
    for i in 0..10 {
        let counter_clone = Arc::clone(&counter);
        let handle = thread::spawn(move || {
            let error = LockError::PathNotFound(format!("path_{}", i));
            set_last_error(error);
            counter_clone.fetch_add(1, Ordering::SeqCst);
            
            // Try to get the error
            let retrieved = get_last_error();
            assert!(retrieved.is_some());
        });
        handles.push(handle);
    }
    
    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }
    
    // Verify all threads executed
    assert_eq!(counter.load(Ordering::SeqCst), 10);
    
    // There should be some error stored (the last one set)
    assert!(get_last_error().is_some());
}

#[test]
fn test_all_error_types_can_be_stored_and_retrieved() {
    let errors = vec![
        LockError::PathNotFound("test1".into()),
        LockError::InvalidPath("test2".into()),
        LockError::SharingViolation("test3".into()),
        LockError::AccessDenied("test4".into()),
        LockError::InvalidHandle,
        LockError::Unknown("test5".into()),
    ];
    
    for error in errors {
        clear_last_error();
        set_last_error(error.clone());
        
        let retrieved = get_last_error();
        assert!(retrieved.is_some());
        
        let retrieved_error = retrieved.unwrap();
        assert_eq!(retrieved_error.to_error_code(), error.to_error_code());
    }
}

#[test]
fn test_error_clone_works_correctly() {
    let error1 = LockError::PathNotFound("test".into());
    let error2 = error1.clone();
    
    assert_eq!(error1.to_error_code(), error2.to_error_code());
    assert_eq!(error1.message(), error2.message());
}

#[test]
fn test_error_debug_format() {
    let error = LockError::PathNotFound("test_path".into());
    let debug_str = format!("{:?}", error);
    
    // Debug format should contain the variant name and path
    assert!(debug_str.contains("PathNotFound"));
    assert!(debug_str.contains("test_path"));
}
