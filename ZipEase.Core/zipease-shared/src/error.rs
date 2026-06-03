use once_cell::sync::Lazy;
use std::sync::Mutex;

/// Represents all possible errors in the lock management system
#[derive(Debug, Clone)]
pub enum LockError {
    PathNotFound(String),
    InvalidPath(String),
    SharingViolation(String),
    AccessDenied(String),
    InvalidHandle,
    ExtractionFailed(String),
    UnsupportedFormat(String),
    PluginRequired(String),
    PasswordRequired(String),
    ZipBomb(String),
    Unknown(String),
}

impl LockError {
    pub fn to_error_code(&self) -> i32 {
        match self {
            LockError::PathNotFound(_) => 3,
            LockError::InvalidPath(_) => 123,
            LockError::SharingViolation(_) => 32,
            LockError::AccessDenied(_) => 5,
            LockError::InvalidHandle => 6,
            LockError::ExtractionFailed(_) => 0x2001,
            LockError::UnsupportedFormat(_) => 0x2002,
            LockError::PluginRequired(_) => 0x2003,
            LockError::PasswordRequired(_) => 0x2004,
            LockError::ZipBomb(_) => 0x2005,
            LockError::Unknown(_) => -1,
        }
    }

    pub fn message(&self) -> String {
        match self {
            LockError::PathNotFound(path) => format!("Path not found: {path}"),
            LockError::InvalidPath(path) => format!("Invalid path: {path}"),
            LockError::SharingViolation(path) => format!("Directory is already locked by another process: {path}"),
            LockError::AccessDenied(path) => format!("Access denied: insufficient permissions to lock {path}"),
            LockError::InvalidHandle => "Invalid handle: the handle is invalid or has been released".to_string(),
            LockError::ExtractionFailed(msg) => format!("Extraction failed: {msg}"),
            LockError::UnsupportedFormat(fmt) => format!("Unsupported archive format: {fmt}"),
            LockError::PluginRequired(plugin) => format!("Plugin required: {plugin}. This format requires an optional plugin to be installed."),
            LockError::PasswordRequired(msg) => format!("Password required: {msg}"),
            LockError::ZipBomb(msg) => msg.clone(),
            LockError::Unknown(msg) => format!("Unknown error: {msg}"),
        }
    }
}

static LAST_ERROR: Lazy<Mutex<Option<LockError>>> = Lazy::new(|| Mutex::new(None));

pub fn set_last_error(error: LockError) {
    if let Ok(mut last_error) = LAST_ERROR.lock() {
        *last_error = Some(error);
    }
}

pub fn get_last_error() -> Option<LockError> {
    LAST_ERROR.lock().ok().and_then(|guard| guard.clone())
}

pub fn clear_last_error() {
    if let Ok(mut last_error) = LAST_ERROR.lock() {
        *last_error = None;
    }
}
