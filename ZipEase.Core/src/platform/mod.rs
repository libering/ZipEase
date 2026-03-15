// Platform-specific implementations
// Currently only Windows is supported

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
pub use windows::WindowsDirectoryLock;
