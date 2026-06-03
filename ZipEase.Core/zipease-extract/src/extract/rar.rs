//! RAR extraction backend using the `unrar` crate (statically linked unrar C++ library).
//!
//! Replaces the previous `SevenZaDllBackend` RAR path which required `7za.dll` to export
//! a RAR handler CLSID — something the standalone `7za.dll` does not support.
//!
//! The `unrar` crate compiles the official unrar C++ source from rarlab.com at build time
//! and links it statically, so no external DLL is required at runtime.
//!
//! # Extraction strategy
//! We use `open_for_processing` + `read()` to get raw bytes, then write them ourselves.
//! This avoids relying on `extract_to`'s internal path handling (which uses the raw
//! backslash-separated Windows path from the RAR header and may fail if the output
//! directory structure doesn't match exactly).

use std::io::Write;
use std::path::Path;
use zipease_shared::LockError;
use super::{ExtractionBackend, ArchiveEntryInfo, safe_join};

pub struct RarBackend;

impl ExtractionBackend for RarBackend {
    fn list_entries(&self, archive_path: &Path) -> Result<Vec<String>, LockError> {
        let entries = self.list_entries_info(archive_path)?;
        Ok(entries.into_iter()
            .filter(|e| !e.is_directory)
            .map(|e| e.name)
            .collect())
    }

    fn list_entries_info(&self, archive_path: &Path) -> Result<Vec<ArchiveEntryInfo>, LockError> {
        let archive = unrar::Archive::new(archive_path)
            .open_for_listing()
            .map_err(|e| LockError::ExtractionFailed(format!("Cannot open RAR archive: {e}")))?;

        let mut entries = Vec::new();
        for entry in archive {
            let entry = entry.map_err(|e| LockError::ExtractionFailed(
                format!("Error reading RAR entry: {e}")
            ))?;

            // Normalise backslash → forward slash (RAR on Windows uses backslash)
            let name = entry.filename.to_string_lossy().replace('\\', "/");
            let is_dir = entry.is_directory();
            let size = entry.unpacked_size as i64;

            entries.push(ArchiveEntryInfo { name, is_directory: is_dir, size });
        }

        Ok(entries)
    }

    fn extract_with_progress<F>(
        &self,
        archive_path: &Path,
        output_dir: &Path,
        progress_fn: F,
    ) -> Result<(), LockError>
    where
        F: Fn(usize, usize, &str),
    {
        // List first to get total count for progress reporting
        let total = self.list_entries_info(archive_path)?
            .into_iter()
            .filter(|e| !e.is_directory)
            .count();

        // Use open_for_processing + read() to get raw bytes, then write ourselves.
        // This avoids extract_to's internal path handling which can fail on Windows
        // when the RAR header uses backslashes and the output dir uses forward slashes.
        let archive = unrar::Archive::new(archive_path)
            .open_for_processing()
            .map_err(|e| LockError::ExtractionFailed(
                format!("Cannot open RAR for extraction: {e}")
            ))?;

        let mut current = 0usize;
        let mut before_header = archive;

        loop {
            let before_file = match before_header.read_header() {
                Ok(Some(bf)) => bf,
                Ok(None) => break,
                Err(e) => return Err(LockError::ExtractionFailed(
                    format!("RAR read header error: {e}")
                )),
            };

            let entry_is_dir = before_file.entry().is_directory();
            // Normalise backslash → forward slash for safe_join
            let entry_name = before_file.entry().filename.to_string_lossy().replace('\\', "/");

            if entry_is_dir {
                // Create directory and skip payload
                if let Ok(dir_path) = safe_join(output_dir, &entry_name) {
                    let _ = std::fs::create_dir_all(&dir_path);
                }
                before_header = before_file.skip()
                    .map_err(|e| LockError::ExtractionFailed(format!("RAR skip error: {e}")))?;
            } else {
                // Build output path via safe_join (path traversal protection)
                let out_path = safe_join(output_dir, &entry_name)
                    .map_err(|_| LockError::ExtractionFailed(
                        format!("Path safety error for entry: {entry_name}")
                    ))?;

                // Pre-create parent directories
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| LockError::ExtractionFailed(
                            format!("Cannot create dir {parent:?}: {e}")
                        ))?;
                }

                progress_fn(current, total, &entry_name);
                current += 1;

                // read() returns (Vec<u8>, next_archive_state)
                let (data, next) = before_file.read()
                    .map_err(|e| LockError::ExtractionFailed(format!("RAR read error: {e}")))?;

                // Write the bytes ourselves — no dependency on unrar's path handling
                let mut outfile = std::fs::File::create(&out_path)
                    .map_err(|e| LockError::ExtractionFailed(
                        format!("Cannot create file {out_path:?}: {e}")
                    ))?;
                outfile.write_all(&data)
                    .map_err(|e| LockError::ExtractionFailed(format!("Write error: {e}")))?;

                before_header = next;
            }
        }

        Ok(())
    }

    fn extract(&self, archive_path: &Path, output_dir: &Path) -> Result<(), LockError> {
        self.extract_with_progress(archive_path, output_dir, |_, _, _| {})
    }
}
