use std::fs::File;
use std::io;
use std::path::Path;
use zipease_shared::LockError;
use super::{ExtractionBackend, ArchiveEntryInfo};

pub struct CabBackend;

impl ExtractionBackend for CabBackend {
    fn extract(&self, archive_path: &Path, output_dir: &Path) -> Result<(), LockError> {
        self.extract_with_progress(archive_path, output_dir, |_, _, _| {})
    }

    fn list_entries(&self, archive_path: &Path) -> Result<Vec<String>, LockError> {
        let file = File::open(archive_path)
            .map_err(|e| LockError::PathNotFound(e.to_string()))?;
        let cabinet = cab::Cabinet::new(file)
            .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;

        let mut entries = Vec::new();
        for folder in cabinet.folder_entries() {
            for file_entry in folder.file_entries() {
                entries.push(file_entry.name().to_string());
            }
        }
        Ok(entries)
    }

    fn list_entries_info(&self, archive_path: &Path) -> Result<Vec<ArchiveEntryInfo>, LockError> {
        let file = File::open(archive_path)
            .map_err(|e| LockError::PathNotFound(e.to_string()))?;
        let cabinet = cab::Cabinet::new(file)
            .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;

        let mut entries = Vec::new();
        for folder in cabinet.folder_entries() {
            for file_entry in folder.file_entries() {
                entries.push(ArchiveEntryInfo {
                    name: file_entry.name().to_string(),
                    is_directory: false,
                    size: file_entry.uncompressed_size() as i64,
                });
            }
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
        let file = File::open(archive_path)
            .map_err(|e| LockError::PathNotFound(e.to_string()))?;
        let mut cabinet = cab::Cabinet::new(file)
            .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;

        // Collect all file names first (need to know total for progress)
        let all_names: Vec<String> = cabinet
            .folder_entries()
            .flat_map(|f| f.file_entries().map(|e| e.name().to_string()).collect::<Vec<_>>())
            .collect();
        let total = all_names.len();

        for (idx, name) in all_names.iter().enumerate() {
            progress_fn(idx, total, name);

            let out_path = super::safe_join(output_dir, name)?;

            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| LockError::Unknown(format!("mkdir failed: {}", e)))?;
            }

            let mut reader = cabinet
                .read_file(name)
                .map_err(|e| LockError::ExtractionFailed(format!("read_file '{}': {}", name, e)))?;

            let mut out_file = File::create(&out_path)
                .map_err(|e| LockError::Unknown(format!("create '{}': {}", out_path.display(), e)))?;

            io::copy(&mut reader, &mut out_file)
                .map_err(|e| LockError::ExtractionFailed(format!("copy '{}': {}", name, e)))?;
        }

        if total > 0 {
            progress_fn(total, total, "");
        }

        Ok(())
    }
}
