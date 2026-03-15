// Lock management module
// Provides business logic for directory locking

pub mod manager;
pub mod handle;

// Re-export commonly used types
pub use manager::LockManager;
pub use handle::LockHandle;
