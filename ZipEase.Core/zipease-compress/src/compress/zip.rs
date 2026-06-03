use std::fs::{self, File};
use std::io;
use std::path::Path;
use zip::write::{FileOptions, ZipWriter};
use zip::CompressionMethod;
use zipease_shared::LockError;
use super::{CompressOptions, CompressionBackend};

pub struct ZipBackend;

/// Walk a path recursively, collecting (disk_path, archive_name) pairs.
fn collect_files(root: &Path, base: &Path) -> io::Result<Vec<(std::path::PathBuf, String)>> {
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

impl CompressionBackend for ZipBackend {
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
        // Collect all files first so we know the total
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
            let out_file = File::create(output_path)
                .map_err(|e| LockError::AccessDenied(e.to_string()))?;
            let mut zip = ZipWriter::new(out_file);

            let compression = match options.level {
                0 => CompressionMethod::Stored,
                _ => CompressionMethod::Deflated,
            };

            for (i, (disk_path, archive_name)) in all_files.iter().enumerate() {
                progress_fn(i + 1, total, archive_name);

                // Check file size to enable ZIP64 for files >= 4 GB
                let file_size = fs::metadata(disk_path)
                    .map_err(|e| LockError::PathNotFound(e.to_string()))?
                    .len();
                let needs_zip64 = file_size >= 4_294_967_296;

                if let Some(ref pwd) = options.password {
                    // AES-256 encrypted entry
                    let mut file_options = FileOptions::<zip::write::ExtendedFileOptions>::default()
                        .compression_method(compression)
                        .compression_level(Some(options.level as i64))
                        .with_aes_encryption(zip::AesMode::Aes256, pwd);
                    if needs_zip64 {
                        file_options = file_options.large_file(true);
                    }
                    zip.start_file(archive_name, file_options)
                        .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                } else {
                    let mut file_options: FileOptions<()> = FileOptions::default()
                        .compression_method(compression)
                        .compression_level(Some(options.level as i64));
                    if needs_zip64 {
                        file_options = file_options.large_file(true);
                    }
                    zip.start_file(archive_name, file_options)
                        .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                }

                // Stream file data directly into the archive without loading
                // the entire file into memory — fixes OOM for large files.
                let mut file = File::open(disk_path)
                    .map_err(|e| LockError::PathNotFound(e.to_string()))?;
                io::copy(&mut file, &mut zip)
                    .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
            }

            zip.finish()
                .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
            Ok(())
        })();

        if result.is_err() {
            let _ = fs::remove_file(output_path);
        }
        result
    }
}
