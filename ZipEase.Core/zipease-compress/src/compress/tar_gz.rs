use std::fs::{self, File};
use std::path::Path;
use tar::Builder;
use zipease_shared::LockError;
use super::{CompressOptions, CompressionBackend};

pub struct TarBackend;

/// Walk a path recursively, collecting (disk_path, archive_name) pairs.
fn collect_files(root: &Path, base: &Path) -> std::io::Result<Vec<(std::path::PathBuf, String)>> {
    let mut result = Vec::new();
    if root.is_dir() {
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let path = entry.path();
            let mut sub = collect_files(&path, base)?;
            result.append(&mut sub);
        }
    } else {
        let archive_name = root
            .strip_prefix(base)
            .unwrap_or(root)
            .to_string_lossy()
            .replace('\\', "/");
        result.push((root.to_path_buf(), archive_name));
    }
    Ok(result)
}

fn compress_tar<W: std::io::Write>(
    writer: W,
    input_paths: &[&Path],
    options: &CompressOptions,
    progress_fn: &dyn Fn(usize, usize, &str),
) -> Result<(), LockError> {
    let mut all_files: Vec<(std::path::PathBuf, String)> = Vec::new();
    for &input in input_paths {
        let base = if options.store_relative_paths {
            input.parent().unwrap_or(input)
        } else {
            input
        };
        let mut files = collect_files(input, base)
            .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
        all_files.append(&mut files);
    }

    let total = all_files.len();
    if total == 0 {
        return Err(LockError::InvalidPath("No files to compress".into()));
    }

    let mut builder = Builder::new(writer);
    for (i, (disk_path, archive_name)) in all_files.iter().enumerate() {
        progress_fn(i + 1, total, archive_name);
        let mut f = File::open(disk_path)
            .map_err(|e| LockError::PathNotFound(e.to_string()))?;
        builder.append_file(archive_name, &mut f)
            .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
    }
    builder.finish()
        .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
    Ok(())
}

impl CompressionBackend for TarBackend {
    fn compress(
        &self,
        input_paths: &[&Path],
        output_path: &Path,
        options: &CompressOptions,
    ) -> Result<(), LockError> {
        self.compress_with_progress(input_paths, output_path, options, |_, _, _| {})
    }

    fn compress_with_progress<F>(
        &self,
        input_paths: &[&Path],
        output_path: &Path,
        options: &CompressOptions,
        progress_fn: F,
    ) -> Result<(), LockError>
    where
        F: Fn(usize, usize, &str),
    {
        let ext = output_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // For .tar.gz / .tar.bz2 etc., the "real" extension is the second-to-last
        let compression_ext = if ext == "tar" {
            // bare .tar — no compression
            "tar"
        } else {
            // e.g. output.tar.gz → ext = "gz"
            ext.as_str()
        };

        let result = (|| -> Result<(), LockError> {
            let out_file = File::create(output_path)
                .map_err(|e| LockError::AccessDenied(e.to_string()))?;

            match compression_ext {
                "gz" | "tgz" => {
                    let level = flate2::Compression::new(options.level as u32);
                    let encoder = flate2::write::GzEncoder::new(out_file, level);
                    compress_tar(encoder, input_paths, options, &progress_fn)
                }
                "bz2" | "tbz2" => {
                    let level = bzip2::Compression::new(options.level as u32);
                    let encoder = bzip2::write::BzEncoder::new(out_file, level);
                    compress_tar(encoder, input_paths, options, &progress_fn)
                }
                "xz" | "txz" => {
                    let encoder = xz2::write::XzEncoder::new(out_file, options.level as u32);
                    compress_tar(encoder, input_paths, options, &progress_fn)
                }
                "zst" | "tzst" => {
                    let encoder = zstd::stream::write::Encoder::new(out_file, options.level as i32)
                        .map_err(|e| LockError::ExtractionFailed(e.to_string()))?
                        .auto_finish();
                    compress_tar(encoder, input_paths, options, &progress_fn)
                }
                _ => {
                    // bare .tar
                    compress_tar(out_file, input_paths, options, &progress_fn)
                }
            }
        })();

        if result.is_err() {
            let _ = fs::remove_file(output_path);
        }
        result
    }
}
