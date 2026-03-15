// Error storage implementation
// Provides thread-safe global error storage for FFI error reporting

use crate::error::LockError;
use once_cell::sync::Lazy;
use std::sync::Mutex;

/// Global thread-safe storage for the last error that occurred
/// 
/// This allows FFI functions to store error information that can be
/// queried later by calling `zip_ease_get_last_error()`.
static LAST_ERROR: Lazy<Mutex<Option<LockError>>> = Lazy::new(|| Mutex::new(None));

/// Stores an error as the last error
/// 
/// This function is called by internal functions when an error occurs.
/// The error can later be retrieved by calling `get_last_error()`.
/// 
/// # Arguments
/// * `error` - The error to store
/// 
/// # Thread Safety
/// This function is thread-safe. Each thread will see the most recent error
/// stored by any thread.
pub fn set_last_error(error: LockError) {
    if let Ok(mut last_error) = LAST_ERROR.lock() {
        *last_error = Some(error);
    }
}

/// Retrieves the last error that occurred
/// 
/// Returns a clone of the last error, or None if no error has occurred
/// or if the error has been cleared.
/// 
/// # Returns
/// * `Some(LockError)` - The last error that occurred
/// * `None` - No error has occurred or the error was cleared
/// 
/// # Thread Safety
/// This function is thread-safe.
pub fn get_last_error() -> Option<LockError> {
    LAST_ERROR.lock().ok().and_then(|guard| guard.clone())
}

/// Clears the last error
/// 
/// This function resets the error storage to None. It's useful for
/// clearing errors after they have been handled.
/// 
/// # Thread Safety
/// This function is thread-safe.
pub fn clear_last_error() {
    if let Ok(mut last_error) = LAST_ERROR.lock() {
        *last_error = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get_error() {
        clear_last_error();
        
        let error = LockError::PathNotFound("test_path".into());
        set_last_error(error.clone());
        
        let retrieved = get_last_error();
        assert!(retrieved.is_some());
        
        let retrieved_error = retrieved.unwrap();
        assert_eq!(retrieved_error.to_error_code(), error.to_error_code());
        assert_eq!(retrieved_error.message(), error.message());
    }

    #[test]
    fn test_clear_error() {
        let error = LockError::InvalidPath("test".into());
        set_last_error(error);
        
        assert!(get_last_error().is_some());
        
        clear_last_error();
        
        assert!(get_last_error().is_none());
    }

    #[test]
    fn test_overwrite_error() {
        clear_last_error();
        let error1 = LockError::PathNotFound("path1".into());
        let error2 = LockError::AccessDenied("path2".into());
        
        set_last_error(error1);
        set_last_error(error2.clone());
        
        let retrieved = get_last_error().unwrap();
        assert_eq!(retrieved.to_error_code(), error2.to_error_code());
    }

    #[test]
    fn test_multiple_error_types() {
        clear_last_error();
        
        let errors = vec![
            LockError::PathNotFound("test1".into()),
            LockError::InvalidPath("test2".into()),
            LockError::SharingViolation("test3".into()),
            LockError::AccessDenied("test4".into()),
            LockError::InvalidHandle,
            LockError::Unknown("test5".into()),
        ];
        
        for error in errors {
            set_last_error(error.clone());
            let retrieved = get_last_error().unwrap();
            assert_eq!(retrieved.to_error_code(), error.to_error_code());
        }
    }
}
