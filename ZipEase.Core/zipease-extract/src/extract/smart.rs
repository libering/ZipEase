use std::path::{Path, PathBuf};
use zipease_shared::LockError;
use super::{zip::ZipBackend, tar::TarBackend, sevenz::SevenZBackend, sevenzadll::SevenZaDllBackend, cab::CabBackend, iso::IsoBackend, ExtractionBackend, ArchiveEntryInfo};

/// Enum to hold concrete backend types (avoids trait object issues with generics)
enum Backend {
    Zip(ZipBackend),
    Tar(TarBackend),
    SevenZ(SevenZBackend),
    SevenZaDll(SevenZaDllBackend),
    Cab(CabBackend),
    Iso(IsoBackend),
}

impl Backend {
    #[allow(dead_code)]
    fn extract(&self, archive_path: &Path, output_dir: &Path) -> Result<(), LockError> {
        match self {
            Backend::Zip(b) => b.extract(archive_path, output_dir),
            Backend::Tar(b) => b.extract(archive_path, output_dir),
            Backend::SevenZ(b) => b.extract(archive_path, output_dir),
            Backend::SevenZaDll(b) => b.extract(archive_path, output_dir),
            Backend::Cab(b) => b.extract(archive_path, output_dir),
            Backend::Iso(b) => b.extract(archive_path, output_dir),
        }
    }
    
    fn extract_with_progress<F>(
        &self,
        archive_path: &Path,
        output_dir: &Path,
        progress_fn: F,
    ) -> Result<(), LockError>
    where
        F: Fn(usize, usize, &str)
    {
        match self {
            Backend::Zip(b) => b.extract_with_progress(archive_path, output_dir, progress_fn),
            Backend::Tar(b) => b.extract_with_progress(archive_path, output_dir, progress_fn),
            Backend::SevenZ(b) => b.extract_with_progress(archive_path, output_dir, progress_fn),
            Backend::SevenZaDll(b) => b.extract_with_progress(archive_path, output_dir, progress_fn),
            Backend::Cab(b) => b.extract_with_progress(archive_path, output_dir, progress_fn),
            Backend::Iso(b) => b.extract_with_progress(archive_path, output_dir, progress_fn),
        }
    }
    
    fn list_entries(&self, archive_path: &Path) -> Result<Vec<String>, LockError> {
        match self {
            Backend::Zip(b) => b.list_entries(archive_path),
            Backend::Tar(b) => b.list_entries(archive_path),
            Backend::SevenZ(b) => b.list_entries(archive_path),
            Backend::SevenZaDll(b) => b.list_entries(archive_path),
            Backend::Cab(b) => b.list_entries(archive_path),
            Backend::Iso(b) => b.list_entries(archive_path),
        }
    }

    fn list_entries_info(&self, archive_path: &Path) -> Result<Vec<ArchiveEntryInfo>, LockError> {
        match self {
            Backend::Zip(b) => b.list_entries_info(archive_path),
            Backend::Tar(b) => b.list_entries_info(archive_path),
            Backend::SevenZ(b) => b.list_entries_info(archive_path),
            Backend::SevenZaDll(b) => b.list_entries_info(archive_path),
            Backend::Cab(b) => b.list_entries_info(archive_path),
            Backend::Iso(b) => b.list_entries_info(archive_path),
        }
    }
}

/// Detect archive format and return concrete backend
fn detect_backend(path: &Path) -> Result<Backend, LockError> {
    let ext = path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .ok_or_else(|| LockError::UnsupportedFormat("No extension".to_string()))?;

    match ext.as_str() {
        "zip" => Ok(Backend::Zip(ZipBackend)),
        "tar" | "gz" | "bz2" | "xz" | "zst" => Ok(Backend::Tar(TarBackend)),
        "7z" => Ok(Backend::SevenZ(SevenZBackend)),
        "rar" => Ok(Backend::SevenZaDll(SevenZaDllBackend)),
        "cab" => Ok(Backend::Cab(CabBackend)),
        "iso" => Ok(Backend::Iso(IsoBackend)),
        _ => Err(LockError::UnsupportedFormat(ext)),
    }
}

/// Implements "Smart Unpacking" logic with progress reporting:
/// 1. If the archive contains only one top-level directory, extract that directory's contents
///    directly into the output directory (or just extract it if it's already a single folder).
/// 2. If the archive contains multiple top-level items or files, create a new folder
///    named after the archive and extract everything into it.
pub fn smart_extract_with_progress<F>(
    archive_path: &Path,
    output_dir: &Path,
    progress_fn: F,
) -> Result<(), LockError>
where
    F: Fn(usize, usize, &str)
{
    let backend = detect_backend(archive_path)?;
    let entries = backend.list_entries(archive_path)?;

    if entries.is_empty() {
        return Err(LockError::ExtractionFailed("Archive is empty".to_string()));
    }

    let top_level_items = get_top_level_items(&entries);

    if top_level_items.len() == 1 {
        // Only one top-level item. If it's a directory, we can extract directly.
        backend.extract_with_progress(archive_path, output_dir, progress_fn)
    } else {
        // Multiple items or files at root. Create a wrapper folder.
        let archive_name = archive_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("extracted");
        
        let mut new_output_dir = PathBuf::from(output_dir);
        new_output_dir.push(archive_name);
        
        if !new_output_dir.exists() {
            std::fs::create_dir_all(&new_output_dir)
                .map_err(|e| LockError::Unknown(format!("Failed to create directory: {}", e)))?;
        }
        
        backend.extract_with_progress(archive_path, &new_output_dir, progress_fn)
    }
}

/// Implements "Smart Unpacking" logic (no progress reporting)
pub fn smart_extract(archive_path: &Path, output_dir: &Path) -> Result<(), LockError> {
    smart_extract_with_progress(archive_path, output_dir, |_, _, _| {})
}

/// List all entries in an archive, returning entry info including directory metadata.
pub fn smart_list_entries(archive_path: &Path) -> Result<Vec<ArchiveEntryInfo>, LockError> {
    let backend = detect_backend(archive_path)?;
    backend.list_entries_info(archive_path)
}

fn get_top_level_items(entries: &[String]) -> Vec<String> {
    let mut items = std::collections::HashSet::new();
    for entry in entries {
        let path = Path::new(entry);
        if let Some(first_part) = path.components().next() {
            if let std::path::Component::Normal(name) = first_part {
                if let Some(name_str) = name.to_str() {
                    items.insert(name_str.to_string());
                }
            }
        }
    }
    items.into_iter().collect()
}
