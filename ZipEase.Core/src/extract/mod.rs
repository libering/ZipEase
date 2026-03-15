use std::path::Path;
use crate::error::types::LockError;

pub mod zip;
pub mod tar;
pub mod sevenz;
pub mod sevenzadll;
pub mod smart;
pub mod encoding;

/// Represents a single entry in an archive, including directory entries.
#[derive(Debug, Clone)]
pub struct ArchiveEntryInfo {
    pub name: String,
    pub is_directory: bool,
    pub size: i64,  // -1 if unknown
}

/// Trait for different archive format backends
pub trait ExtractionBackend {
    /// Extract the archive to the target directory
    fn extract(&self, archive_path: &Path, output_dir: &Path) -> Result<(), LockError>;
    
    /// Get a list of all entries in the archive
    fn list_entries(&self, archive_path: &Path) -> Result<Vec<String>, LockError>;
    
    /// Extract the archive with progress reporting
    /// 
    /// # Arguments
    /// * `archive_path` - Path to the archive file
    /// * `output_dir` - Directory where files will be extracted
    /// * `progress_fn` - Callback invoked for each file: (current_index, total_files, filename)
    fn extract_with_progress<F>(
        &self,
        archive_path: &Path,
        output_dir: &Path,
        progress_fn: F,
    ) -> Result<(), LockError>
    where
        F: Fn(usize, usize, &str);

    /// Get a list of all entries including directories, with metadata.
    /// Default implementation wraps `list_entries` treating all as files.
    fn list_entries_info(&self, archive_path: &Path) -> Result<Vec<ArchiveEntryInfo>, LockError> {
        let names = self.list_entries(archive_path)?;
        Ok(names.into_iter().map(|name| ArchiveEntryInfo {
            name,
            is_directory: false,
            size: -1,
        }).collect())
    }
}

/// Unified entry point for extraction with progress reporting
pub fn extract_with_progress<F>(
    archive_path: &Path,
    output_dir: &Path,
    progress_fn: F,
) -> Result<(), LockError>
where
    F: Fn(usize, usize, &str)
{
    smart::smart_extract_with_progress(archive_path, output_dir, progress_fn)
}

/// Unified entry point for extraction (no progress reporting)
pub fn extract(archive_path: &Path, output_dir: &Path) -> Result<(), LockError> {
    extract_with_progress(archive_path, output_dir, |_, _, _| {})
}
