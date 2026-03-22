use std::path::Path;
use zipease_shared::LockError;

pub mod zip;
pub mod tar;
pub mod sevenz;
pub mod sevenzadll;
pub mod smart;
pub mod encoding;
pub mod cab;
pub mod iso;

/// Safely join `entry_name` onto `base`, rejecting any path that would escape `base`.
///
/// Defends against:
/// - Zip Slip (`../../../etc/passwd`)
/// - Absolute paths (`/etc/passwd`, `C:\Windows\evil.dll`)
/// - Windows device paths (`\\.\COM1`)
/// - Null bytes in names
///
/// Returns `Err(LockError::ExtractionFailed)` if the resolved path does not start
/// with the canonical `base` directory.
pub fn safe_join(base: &Path, entry_name: &str) -> Result<std::path::PathBuf, LockError> {
    // Reject null bytes — they can truncate paths on some systems
    if entry_name.contains('\0') {
        return Err(LockError::ExtractionFailed(format!(
            "Rejected entry with null byte in name: {:?}", entry_name
        )));
    }

    // Strip leading separators / drive letters to prevent absolute-path injection.
    // We strip component-by-component so that "C:\foo\bar" becomes "foo/bar".
    let sanitized: std::path::PathBuf = std::path::Path::new(entry_name)
        .components()
        .filter(|c| matches!(c,
            std::path::Component::Normal(_)
        ))
        .collect();

    if sanitized.as_os_str().is_empty() {
        return Err(LockError::ExtractionFailed(format!(
            "Rejected empty or root-only entry name: {:?}", entry_name
        )));
    }

    let joined = base.join(&sanitized);

    // Canonicalize base so symlinks in the base path are resolved before comparison.
    // If base doesn't exist yet (e.g. temp dir not created), fall back to lexical check.
    let canonical_base = std::fs::canonicalize(base)
        .unwrap_or_else(|_| base.to_path_buf());

    // For the joined path we can't canonicalize (file doesn't exist yet), so we
    // normalise lexically: resolve any remaining `..` components.
    let mut resolved = std::path::PathBuf::new();
    for component in joined.components() {
        match component {
            std::path::Component::ParentDir => { resolved.pop(); }
            std::path::Component::CurDir => {}
            c => resolved.push(c),
        }
    }

    if !resolved.starts_with(&canonical_base) {
        return Err(LockError::ExtractionFailed(format!(
            "Path traversal detected: entry {:?} resolves outside output directory",
            entry_name
        )));
    }

    Ok(resolved)
}

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
