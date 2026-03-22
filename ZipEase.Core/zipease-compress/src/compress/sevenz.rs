use std::fs;
use std::path::Path;
use zipease_shared::LockError;
use super::{CompressOptions, CompressionBackend};

pub struct SevenZBackend;

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

impl CompressionBackend for SevenZBackend {
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

        let result = (|| -> Result<(), LockError> {
            // sevenz-rust compress API: compress a list of source paths into an archive.
            // We use the lower-level SevenZWriter for per-file progress.
            let mut sz = sevenz_rust::SevenZWriter::create(output_path)
                .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;

            for (i, (disk_path, archive_name)) in all_files.iter().enumerate() {
                progress_fn(i + 1, total, archive_name);
                sz.push_source_path(disk_path, |_| true)
                    .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
            }

            sz.finish()
                .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
            Ok(())
        })();

        if result.is_err() {
            let _ = fs::remove_file(output_path);
        }
        result
    }
}
