// Error handling module
// Provides unified error management across all layers

pub mod types;
pub mod storage;

// Re-export commonly used types and functions
pub use types::LockError;
pub use storage::{set_last_error, get_last_error, clear_last_error};
