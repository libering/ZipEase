use std::path::Path;
use zipease_shared::LockError;

pub mod zip;
pub mod tar_gz;
pub mod sevenz;
pub mod smart;

/// Options controlling how an archive is created.
pub struct CompressOptions {
    /// Compression level 0–9 (format-specific interpretation).
    pub level: u8,
    /// If true, store paths relative to a common base directory.
    pub store_relative_paths: bool,
}

impl Default for CompressOptions {
    fn default() -> Self {
        Self { level: 6, store_relative_paths: true }
    }
}

pub trait CompressionBackend {
    /// Create an archive at `output_path` from the given `input_paths`.
    fn compress(
        &self,
        input_paths: &[&Path],
        output_path: &Path,
        options: &CompressOptions,
    ) -> Result<(), LockError>;

    /// Same as `compress` but fires `progress_fn(current, total, filename)` per file.
    fn compress_with_progress<F>(
        &self,
        input_paths: &[&Path],
        output_path: &Path,
        options: &CompressOptions,
        progress_fn: F,
    ) -> Result<(), LockError>
    where
        F: Fn(usize, usize, &str);
}

/// Unified entry point — delegates to smart dispatch.
pub fn compress_with_progress<F>(
    input_paths: &[&Path],
    output_path: &Path,
    options: &CompressOptions,
    progress_fn: F,
) -> Result<(), LockError>
where
    F: Fn(usize, usize, &str),
{
    smart::compress_with_progress(input_paths, output_path, options, progress_fn)
}
