// Error types definition
// Defines all possible errors that can occur during directory locking operations

/// Represents all possible errors in the lock management system
#[derive(Debug, Clone)]
pub enum LockError {
    /// The specified path does not exist
    PathNotFound(String),
    
    /// The path format is invalid (empty, null, or contains invalid characters)
    InvalidPath(String),
    
    /// The directory is already locked by another process
    SharingViolation(String),
    
    /// Insufficient permissions to lock the directory
    AccessDenied(String),
    
    /// The provided handle is invalid or has been released
    InvalidHandle,

    /// Extraction failed
    ExtractionFailed(String),

    /// The archive format is not supported
    UnsupportedFormat(String),

    /// A required plugin or native library is not installed
    PluginRequired(String),

    /// The archive is password-protected; a password is required or the supplied password is wrong
    PasswordRequired(String),
    
    /// An unknown or unexpected error occurred
    Unknown(String),
}

impl LockError {
    /// Maps the error to a Windows error code
    /// 
    /// Returns the corresponding Windows error code for each error type.
    /// These codes match the standard Windows API error codes.
    pub fn to_error_code(&self) -> i32 {
        match self {
            LockError::PathNotFound(_) => 3,      // ERROR_PATH_NOT_FOUND
            LockError::InvalidPath(_) => 123,     // ERROR_INVALID_NAME
            LockError::SharingViolation(_) => 32, // ERROR_SHARING_VIOLATION
            LockError::AccessDenied(_) => 5,      // ERROR_ACCESS_DENIED
            LockError::InvalidHandle => 6,        // ERROR_INVALID_HANDLE
            LockError::ExtractionFailed(_) => 0x2001, // Custom code
            LockError::UnsupportedFormat(_) => 0x2002, // Custom code
            LockError::PluginRequired(_) => 0x2003,
            LockError::PasswordRequired(_) => 0x2004,
            LockError::Unknown(_) => -1,          // Custom error code for unknown errors
        }
    }
    
    /// Generates a human-readable error message
    /// 
    /// Returns a descriptive error message that can be displayed to users
    /// or logged for debugging purposes.
    pub fn message(&self) -> String {
        match self {
            LockError::PathNotFound(path) => {
                format!("Path not found: {}", path)
            }
            LockError::InvalidPath(path) => {
                format!("Invalid path: {}", path)
            }
            LockError::SharingViolation(path) => {
                format!("Directory is already locked by another process: {}", path)
            }
            LockError::AccessDenied(path) => {
                format!("Access denied: insufficient permissions to lock {}", path)
            }
            LockError::InvalidHandle => {
                "Invalid handle: the handle is invalid or has been released".to_string()
            }
            LockError::ExtractionFailed(msg) => {
                format!("Extraction failed: {}", msg)
            }
            LockError::UnsupportedFormat(fmt) => {
                format!("Unsupported archive format: {}", fmt)
            }
            LockError::PluginRequired(plugin) => {
                format!("Plugin required: {}. This format requires an optional plugin to be installed.", plugin)
            }
            LockError::PasswordRequired(msg) => {
                format!("Password required: {}", msg)
            }
            LockError::Unknown(msg) => {
                format!("Unknown error: {}", msg)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_mapping() {
        assert_eq!(LockError::PathNotFound("test".into()).to_error_code(), 3);
        assert_eq!(LockError::InvalidPath("test".into()).to_error_code(), 123);
        assert_eq!(LockError::SharingViolation("test".into()).to_error_code(), 32);
        assert_eq!(LockError::AccessDenied("test".into()).to_error_code(), 5);
        assert_eq!(LockError::InvalidHandle.to_error_code(), 6);
        assert_eq!(LockError::Unknown("test".into()).to_error_code(), -1);
    }

    #[test]
    fn test_error_messages() {
        let path = "C:\\test\\path";
        
        let err = LockError::PathNotFound(path.into());
        assert!(err.message().contains("Path not found"));
        assert!(err.message().contains(path));
        
        let err = LockError::InvalidPath(path.into());
        assert!(err.message().contains("Invalid path"));
        assert!(err.message().contains(path));
        
        let err = LockError::SharingViolation(path.into());
        assert!(err.message().contains("already locked"));
        assert!(err.message().contains(path));
        
        let err = LockError::AccessDenied(path.into());
        assert!(err.message().contains("Access denied"));
        assert!(err.message().contains(path));
        
        let err = LockError::InvalidHandle;
        assert!(err.message().contains("Invalid handle"));
        
        let err = LockError::Unknown("custom message".into());
        assert!(err.message().contains("Unknown error"));
        assert!(err.message().contains("custom message"));
    }
}
