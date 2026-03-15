use std::path::Path;
use crate::error::types::LockError;
use super::ExtractionBackend;
use sevenz_rust::{SevenZReader, Password};

pub struct SevenZBackend;

impl ExtractionBackend for SevenZBackend {
    fn extract(&self, archive_path: &Path, output_dir: &Path) -> Result<(), LockError> {
        self.extract_with_progress(archive_path, output_dir, |_, _, _| {})
    }

    fn list_entries(&self, archive_path: &Path) -> Result<Vec<String>, LockError> {
        let reader = SevenZReader::open(archive_path, Password::empty())
            .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
        
        let entries = reader.archive().files.iter()
            .filter(|e| !e.is_directory())
            .map(|e| e.name().to_string())
            .collect();
        
        Ok(entries)
    }

    fn list_entries_info(&self, archive_path: &Path) -> Result<Vec<super::ArchiveEntryInfo>, LockError> {
        let reader = SevenZReader::open(archive_path, Password::empty())
            .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
        let entries = reader.archive().files.iter()
            .map(|e| super::ArchiveEntryInfo {
                name: e.name().to_string(),
                is_directory: e.is_directory(),
                size: -1,
            })
            .collect();
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
        use std::fs::{create_dir_all, File};
        
        let mut reader = SevenZReader::open(archive_path, Password::empty())
            .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
        
        let total = reader.archive().files.iter()
            .filter(|e| !e.is_directory())
            .count();
        let mut current_index = 0;
        
        reader.for_each_entries(|entry, reader| {
            let name = entry.name();
            
            let dest_path = output_dir.join(name);
            
            if entry.is_directory() {
                create_dir_all(&dest_path)
                    .map_err(|e| sevenz_rust::Error::other(e.to_string()))?;
            } else {
                current_index += 1;
                progress_fn(current_index, total, name);
                
                if let Some(parent) = dest_path.parent() {
                    create_dir_all(parent)
                        .map_err(|e| sevenz_rust::Error::other(e.to_string()))?;
                }
                
                let mut outfile = File::create(&dest_path)
                    .map_err(|e| sevenz_rust::Error::other(e.to_string()))?;
                
                std::io::copy(reader, &mut outfile)
                    .map_err(|e| sevenz_rust::Error::other(e.to_string()))?;
            }
            
            Ok(true)
        }).map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
        
        Ok(())
    }
}

impl SevenZBackend {
    pub fn list_entries_info_with_password(&self, archive_path: &Path, password: &str) -> Result<Vec<super::ArchiveEntryInfo>, LockError> {
        use super::ArchiveEntryInfo;
        let reader = SevenZReader::open(archive_path, Password::from(password))
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("password") || msg.contains("Password") || msg.contains("encrypted") {
                    LockError::PasswordRequired(msg)
                } else {
                    LockError::ExtractionFailed(msg)
                }
            })?;
        let entries = reader.archive().files.iter()
            .map(|e| ArchiveEntryInfo {
                name: e.name().to_string(),
                is_directory: e.is_directory(),
                size: -1,
            })
            .collect();
        Ok(entries)
    }

    pub fn extract_with_password_progress<F>(&self, archive_path: &Path, output_dir: &Path, password: &str, progress_fn: F) -> Result<(), LockError>
    where
        F: Fn(usize, usize, &str)
    {
        use std::fs::{create_dir_all, File};

        let mut reader = SevenZReader::open(archive_path, Password::from(password))
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("password") || msg.contains("Password") || msg.contains("encrypted") {
                    LockError::PasswordRequired(msg)
                } else {
                    LockError::ExtractionFailed(msg)
                }
            })?;

        let total = reader.archive().files.iter()
            .filter(|e| !e.is_directory())
            .count();
        let mut current_index = 0;

        reader.for_each_entries(|entry, reader| {
            let name = entry.name();
            let dest_path = output_dir.join(name);
            if entry.is_directory() {
                create_dir_all(&dest_path)
                    .map_err(|e| sevenz_rust::Error::other(e.to_string()))?;
            } else {
                current_index += 1;
                progress_fn(current_index, total, name);
                if let Some(parent) = dest_path.parent() {
                    create_dir_all(parent)
                        .map_err(|e| sevenz_rust::Error::other(e.to_string()))?;
                }
                let mut outfile = File::create(&dest_path)
                    .map_err(|e| sevenz_rust::Error::other(e.to_string()))?;
                std::io::copy(reader, &mut outfile)
                    .map_err(|e| sevenz_rust::Error::other(e.to_string()))?;
            }
            Ok(true)
        }).map_err(|e| {
            let msg = e.to_string();
            if msg.contains("password") || msg.contains("Password") || msg.contains("encrypted") {
                LockError::PasswordRequired(msg)
            } else {
                LockError::ExtractionFailed(msg)
            }
        })?;

        Ok(())
    }
}

