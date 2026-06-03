use std::path::{Path, PathBuf};
use zipease_shared::LockError;
use super::{zip::ZipBackend, tar::TarBackend, sevenz::SevenZBackend, sevenzadll::{SevenZaDllBackend, SevenZaDllBackendWithClsid, CLSID_7Z_HANDLER, CLSID_ZIP_HANDLER}, cab::CabBackend, iso::IsoBackend, rar::RarBackend, ExtractionBackend, ArchiveEntryInfo};

/// Enum to hold concrete backend types (avoids trait object issues with generics)
pub(crate) enum Backend {
    Zip(ZipBackend),
    Tar(TarBackend),
    SevenZ(SevenZBackend),
    SevenZaDll(SevenZaDllBackend),
    SevenZaDllClsid(SevenZaDllBackendWithClsid),
    Cab(CabBackend),
    Iso(IsoBackend),
    Rar(RarBackend),
}

impl Backend {
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
            Backend::SevenZaDllClsid(b) => b.extract_with_progress(archive_path, output_dir, progress_fn),
            Backend::Cab(b) => b.extract_with_progress(archive_path, output_dir, progress_fn),
            Backend::Iso(b) => b.extract_with_progress(archive_path, output_dir, progress_fn),
            Backend::Rar(b) => b.extract_with_progress(archive_path, output_dir, progress_fn),
        }
    }
    
    fn list_entries(&self, archive_path: &Path) -> Result<Vec<String>, LockError> {
        match self {
            Backend::Zip(b) => b.list_entries(archive_path),
            Backend::Tar(b) => b.list_entries(archive_path),
            Backend::SevenZ(b) => b.list_entries(archive_path),
            Backend::SevenZaDll(b) => b.list_entries(archive_path),
            Backend::SevenZaDllClsid(b) => b.list_entries(archive_path),
            Backend::Cab(b) => b.list_entries(archive_path),
            Backend::Iso(b) => b.list_entries(archive_path),
            Backend::Rar(b) => b.list_entries(archive_path),
        }
    }

    fn list_entries_info(&self, archive_path: &Path) -> Result<Vec<ArchiveEntryInfo>, LockError> {
        match self {
            Backend::Zip(b) => b.list_entries_info(archive_path),
            Backend::Tar(b) => b.list_entries_info(archive_path),
            Backend::SevenZ(b) => b.list_entries_info(archive_path),
            Backend::SevenZaDll(b) => b.list_entries_info(archive_path),
            Backend::SevenZaDllClsid(b) => b.list_entries_info(archive_path),
            Backend::Cab(b) => b.list_entries_info(archive_path),
            Backend::Iso(b) => b.list_entries_info(archive_path),
            Backend::Rar(b) => b.list_entries_info(archive_path),
        }
    }
}

/// Detect archive format and return concrete backend
pub(crate) fn detect_backend(path: &Path) -> Result<Backend, LockError> {
    let ext = path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .ok_or_else(|| LockError::UnsupportedFormat("No extension".to_string()))?;

    match ext.as_str() {
        "zip" | "apk" | "ipa" | "jar" | "war" | "ear" => Ok(Backend::Zip(ZipBackend)),
        "tar" | "gz" | "bz2" | "xz" | "zst" => Ok(Backend::Tar(TarBackend)),
        "7z" => Ok(Backend::SevenZ(SevenZBackend)),
        "rar" => Ok(Backend::Rar(RarBackend)),
        "cab" => Ok(Backend::Cab(CabBackend)),
        "iso" => Ok(Backend::Iso(IsoBackend)),
        // Split archive formats — route through 7za.dll with appropriate CLSID
        // .7z.001, .7z.002, ... (7-Zip split)
        "001" => detect_split_backend(path),
        // .zip.001 is handled by detect_split_backend too
        // .z01, .z02, ... (WinZip split)
        ext if ext.starts_with('z') && ext[1..].parse::<u32>().is_ok() => {
            Ok(Backend::SevenZaDllClsid(SevenZaDllBackendWithClsid(CLSID_ZIP_HANDLER)))
        }
        // .part1.rar, .part2.rar, ... (WinRAR split) — already handled by RAR CLSID
        _ => {
            crate::zlog(&format!("[smart] detect_backend: unsupported ext {:?} for {:?}", ext, path));
            Err(LockError::UnsupportedFormat(ext))
        },
    }
}

/// Detect the correct backend for `.001` split archives by inspecting the stem extension.
/// e.g. `archive.7z.001` → stem is `archive.7z` → use 7z CLSID
///      `archive.zip.001` → stem is `archive.zip` → use ZIP CLSID
///      `archive.rar.001` → stem is `archive.rar` → use RAR CLSID
///      `archive.001` → no recognisable stem ext → try 7z CLSID (most common)
fn detect_split_backend(path: &Path) -> Result<Backend, LockError> {
    let stem = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    let inner_ext = std::path::Path::new(stem)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    match inner_ext.as_str() {
        "zip" => Ok(Backend::SevenZaDllClsid(SevenZaDllBackendWithClsid(CLSID_ZIP_HANDLER))),
        "rar" => Ok(Backend::SevenZaDll(SevenZaDllBackend)),
        // "7z" or unknown — default to 7z handler
        _ => Ok(Backend::SevenZaDllClsid(SevenZaDllBackendWithClsid(CLSID_7Z_HANDLER))),
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

/// Extract directly to output_dir without any smart wrapping.
/// The caller is responsible for creating a wrapper folder if desired.
pub fn extract_direct<F>(
    archive_path: &Path,
    output_dir: &Path,
    progress_fn: F,
) -> Result<(), LockError>
where
    F: Fn(usize, usize, &str)
{
    let backend = detect_backend(archive_path)?;
    backend.extract_with_progress(archive_path, output_dir, progress_fn)
}

/// List all entries in an archive, returning entry info including directory metadata.
pub fn smart_list_entries(archive_path: &Path) -> Result<Vec<ArchiveEntryInfo>, LockError> {
    let backend = detect_backend(archive_path)?;
    let entries = backend.list_entries_info(archive_path)?;
    crate::zlog(&format!("[smart] smart_list_entries: {} entries for {:?}", entries.len(), archive_path));
    Ok(entries)
}

fn get_top_level_items(entries: &[String]) -> Vec<String> {
    let mut items = std::collections::HashSet::new();
    for entry in entries {
        let path = Path::new(entry);
        if let Some(std::path::Component::Normal(name)) = path.components().next() {
            if let Some(name_str) = name.to_str() {
                items.insert(name_str.to_string());
            }
        }
    }
    items.into_iter().collect()
}
