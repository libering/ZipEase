use std::path::Path;
use zipease_shared::LockError;
use super::{CompressOptions, CompressionBackend};
use super::zip::ZipBackend;
use super::tar_gz::TarBackend;
use super::sevenz::SevenZBackend;

/// Dispatch compression to the correct backend based on the output file extension.
pub fn compress_with_progress<F>(
    input_paths: &[&Path],
    output_path: &Path,
    options: &CompressOptions,
    progress_fn: F,
) -> Result<(), LockError>
where
    F: Fn(usize, usize, &str),
{
    // Handle double extensions like .tar.gz — check the stem's extension first
    let ext = output_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "zip" => ZipBackend.compress_with_progress(input_paths, output_path, options, progress_fn),
        "7z" => SevenZBackend.compress_with_progress(input_paths, output_path, options, progress_fn),
        "gz" | "bz2" | "xz" | "zst" | "tgz" | "tbz2" | "txz" | "tzst" | "tar" => {
            TarBackend.compress_with_progress(input_paths, output_path, options, progress_fn)
        }
        other => Err(LockError::UnsupportedFormat(other.to_string())),
    }
}
