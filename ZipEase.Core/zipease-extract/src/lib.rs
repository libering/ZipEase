// FFI functions receive raw pointers from C# P/Invoke and must dereference them,
// but cannot be marked `unsafe` as that changes the extern "C" calling convention.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

pub mod batch;
pub mod extract;
pub mod ffi;
pub mod lock_detector;
pub mod notify;
pub mod repair;
pub mod search;
pub mod trash;

pub use extract::{extract, extract_with_progress, ArchiveEntryInfo, ExtractionBackend};
pub use ffi::zip_ease_trash_file;
pub use ffi::zip_ease_who_locks;

use std::sync::Mutex;

/// Global log file handle. Written directly — no `log` crate, no feature flags.
/// Works in both debug and release builds.
static LOG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// Write a timestamped line directly to the Rust log file.
/// Call this anywhere in the codebase for diagnostics.
pub fn zlog(msg: &str) {
    if let Ok(mut guard) = LOG_FILE.lock() {
        if let Some(ref mut f) = *guard {
            use std::io::Write;
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let _ = writeln!(f, "[{ts}] {msg}");
            let _ = f.flush();
        }
    }
}

/// Initialise direct file logging. Called once via `#[ctor]` in zipease-core.
/// Writes to `%TEMP%\ZipEase_rust_<unix_timestamp>.log`.
pub fn init_logging() {
    use std::sync::OnceLock;
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let log_path = std::env::temp_dir()
            .join(format!("ZipEase_rust_{timestamp}.log"));

        if let Ok(file) = std::fs::OpenOptions::new()
            .create(true).write(true).truncate(true)
            .open(&log_path)
        {
            if let Ok(mut guard) = LOG_FILE.lock() {
                *guard = Some(file);
            }
            zlog(&format!("ZipEase Rust logger initialised: {log_path:?}"));
        }
    });
}
