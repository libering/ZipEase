use std::path::Path;
use std::fs::File;
use tar::Archive;
use crate::error::types::LockError;
use super::ExtractionBackend;

pub struct TarBackend;

impl ExtractionBackend for TarBackend {
    fn extract(&self, archive_path: &Path, output_dir: &Path) -> Result<(), LockError> {
        self.extract_with_progress(archive_path, output_dir, |_, _, _| {})
    }

    fn list_entries(&self, archive_path: &Path) -> Result<Vec<String>, LockError> {
        let file = File::open(archive_path)
            .map_err(|e| LockError::PathNotFound(e.to_string()))?;
        
        let ext = archive_path.extension().and_then(|s| s.to_str()).unwrap_or("");
        
        let mut entries = Vec::new();
        
        macro_rules! list_tar_entries {
            ($decoder:expr) => {{
                let mut archive = Archive::new($decoder);
                for entry in archive.entries().map_err(|e| LockError::ExtractionFailed(e.to_string()))? {
                    let entry = entry.map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                    // Skip directory entries
                    if entry.header().entry_type().is_dir() {
                        continue;
                    }
                    if let Ok(path) = entry.path() {
                        if let Some(path_str) = path.to_str() {
                            // Strip leading "./" prefix
                            let normalized = path_str.trim_start_matches("./").to_string();
                            if !normalized.is_empty() {
                                entries.push(normalized);
                            }
                        }
                    }
                }
            }};
        }

        match ext {
            "gz" | "tgz" => list_tar_entries!(flate2::read::GzDecoder::new(file)),
            "bz2" | "tbz2" => list_tar_entries!(bzip2::read::BzDecoder::new(file)),
            "xz" | "txz" => list_tar_entries!(xz2::read::XzDecoder::new(file)),
            "zst" | "tzst" => list_tar_entries!(zstd::stream::read::Decoder::new(file).map_err(|e| LockError::ExtractionFailed(e.to_string()))?),
            _ => list_tar_entries!(file),
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
        F: Fn(usize, usize, &str)
    {
        let ext = archive_path.extension().and_then(|s| s.to_str()).unwrap_or("");
        
        match ext {
            "gz" | "tgz" => {
                // First pass: count non-directory entries
                let file1 = File::open(archive_path)
                    .map_err(|e| LockError::PathNotFound(e.to_string()))?;
                let decoder1 = flate2::read::GzDecoder::new(file1);
                let mut archive1 = Archive::new(decoder1);
                let total = archive1.entries()
                    .map_err(|e| LockError::ExtractionFailed(e.to_string()))?
                    .filter(|e| e.as_ref().map(|e| !e.header().entry_type().is_dir()).unwrap_or(false))
                    .count();
                
                // Second pass: extract with progress
                let file2 = File::open(archive_path)
                    .map_err(|e| LockError::PathNotFound(e.to_string()))?;
                let decoder2 = flate2::read::GzDecoder::new(file2);
                let mut archive2 = Archive::new(decoder2);
                
                let mut file_count = 0usize;
                for entry in archive2.entries()
                    .map_err(|e| LockError::ExtractionFailed(e.to_string()))?
                {
                    let mut entry = entry.map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                    let is_dir = entry.header().entry_type().is_dir();
                    
                    if !is_dir {
                        file_count += 1;
                        let path = entry.path().map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                        let file_name = path.to_str().unwrap_or("unknown");
                        let file_name = file_name.trim_start_matches("./");
                        progress_fn(file_count, total, file_name);
                    }
                    
                    entry.unpack_in(output_dir)
                        .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                }
                
                Ok(())
            }
            "bz2" | "tbz2" => {
                let file1 = File::open(archive_path)
                    .map_err(|e| LockError::PathNotFound(e.to_string()))?;
                let decoder1 = bzip2::read::BzDecoder::new(file1);
                let mut archive1 = Archive::new(decoder1);
                let total = archive1.entries()
                    .map_err(|e| LockError::ExtractionFailed(e.to_string()))?
                    .filter(|e| e.as_ref().map(|e| !e.header().entry_type().is_dir()).unwrap_or(false))
                    .count();
                
                let file2 = File::open(archive_path)
                    .map_err(|e| LockError::PathNotFound(e.to_string()))?;
                let decoder2 = bzip2::read::BzDecoder::new(file2);
                let mut archive2 = Archive::new(decoder2);
                
                let mut file_count = 0usize;
                for entry in archive2.entries()
                    .map_err(|e| LockError::ExtractionFailed(e.to_string()))?
                {
                    let mut entry = entry.map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                    let is_dir = entry.header().entry_type().is_dir();
                    
                    if !is_dir {
                        file_count += 1;
                        let path = entry.path().map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                        let file_name = path.to_str().unwrap_or("unknown");
                        let file_name = file_name.trim_start_matches("./");
                        progress_fn(file_count, total, file_name);
                    }
                    
                    entry.unpack_in(output_dir)
                        .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                }
                
                Ok(())
            }
            "xz" | "txz" => {
                let file1 = File::open(archive_path)
                    .map_err(|e| LockError::PathNotFound(e.to_string()))?;
                let decoder1 = xz2::read::XzDecoder::new(file1);
                let mut archive1 = Archive::new(decoder1);
                let total = archive1.entries()
                    .map_err(|e| LockError::ExtractionFailed(e.to_string()))?
                    .filter(|e| e.as_ref().map(|e| !e.header().entry_type().is_dir()).unwrap_or(false))
                    .count();
                
                let file2 = File::open(archive_path)
                    .map_err(|e| LockError::PathNotFound(e.to_string()))?;
                let decoder2 = xz2::read::XzDecoder::new(file2);
                let mut archive2 = Archive::new(decoder2);
                
                let mut file_count = 0usize;
                for entry in archive2.entries()
                    .map_err(|e| LockError::ExtractionFailed(e.to_string()))?
                {
                    let mut entry = entry.map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                    let is_dir = entry.header().entry_type().is_dir();
                    
                    if !is_dir {
                        file_count += 1;
                        let path = entry.path().map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                        let file_name = path.to_str().unwrap_or("unknown");
                        let file_name = file_name.trim_start_matches("./");
                        progress_fn(file_count, total, file_name);
                    }
                    
                    entry.unpack_in(output_dir)
                        .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                }
                
                Ok(())
            }
            "zst" | "tzst" => {
                let file1 = File::open(archive_path)
                    .map_err(|e| LockError::PathNotFound(e.to_string()))?;
                let decoder1 = zstd::stream::read::Decoder::new(file1)
                    .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                let mut archive1 = Archive::new(decoder1);
                let total = archive1.entries()
                    .map_err(|e| LockError::ExtractionFailed(e.to_string()))?
                    .filter(|e| e.as_ref().map(|e| !e.header().entry_type().is_dir()).unwrap_or(false))
                    .count();
                
                let file2 = File::open(archive_path)
                    .map_err(|e| LockError::PathNotFound(e.to_string()))?;
                let decoder2 = zstd::stream::read::Decoder::new(file2)
                    .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                let mut archive2 = Archive::new(decoder2);
                
                let mut file_count = 0usize;
                for entry in archive2.entries()
                    .map_err(|e| LockError::ExtractionFailed(e.to_string()))?
                {
                    let mut entry = entry.map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                    let is_dir = entry.header().entry_type().is_dir();
                    
                    if !is_dir {
                        file_count += 1;
                        let path = entry.path().map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                        let file_name = path.to_str().unwrap_or("unknown");
                        let file_name = file_name.trim_start_matches("./");
                        progress_fn(file_count, total, file_name);
                    }
                    
                    entry.unpack_in(output_dir)
                        .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                }
                
                Ok(())
            }
            _ => {
                let file1 = File::open(archive_path)
                    .map_err(|e| LockError::PathNotFound(e.to_string()))?;
                let mut archive1 = Archive::new(file1);
                let total = archive1.entries()
                    .map_err(|e| LockError::ExtractionFailed(e.to_string()))?
                    .filter(|e| e.as_ref().map(|e| !e.header().entry_type().is_dir()).unwrap_or(false))
                    .count();
                
                let file2 = File::open(archive_path)
                    .map_err(|e| LockError::PathNotFound(e.to_string()))?;
                let mut archive2 = Archive::new(file2);
                
                let mut file_count = 0usize;
                for entry in archive2.entries()
                    .map_err(|e| LockError::ExtractionFailed(e.to_string()))?
                {
                    let mut entry = entry.map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                    let is_dir = entry.header().entry_type().is_dir();
                    
                    if !is_dir {
                        file_count += 1;
                        let path = entry.path().map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                        let file_name = path.to_str().unwrap_or("unknown");
                        let file_name = file_name.trim_start_matches("./");
                        progress_fn(file_count, total, file_name);
                    }
                    
                    entry.unpack_in(output_dir)
                        .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                }
                
                Ok(())
            }
        }
    }
}
