use std::path::Path;
use std::fs::File;
use zip::ZipArchive;
use zip::HasZipMetadata;
use crate::error::types::LockError;
use super::ExtractionBackend;
use super::encoding;

pub struct ZipBackend;

fn utf8_flag(file: &zip::read::ZipFile) -> bool {
    file.get_metadata().is_utf8
}

impl ExtractionBackend for ZipBackend {
    fn extract(&self, archive_path: &Path, output_dir: &Path) -> Result<(), LockError> {
        self.extract_with_progress(archive_path, output_dir, |_, _, _| {})
    }

    fn list_entries(&self, archive_path: &Path) -> Result<Vec<String>, LockError> {
        let file = File::open(archive_path)
            .map_err(|e| LockError::PathNotFound(e.to_string()))?;

        let mut archive = ZipArchive::new(file)
            .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;

        let mut entries = Vec::new();
        for i in 0..archive.len() {
            let file = archive.by_index(i)
                .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
            let name = encoding::decode_zip_filename(file.name_raw(), utf8_flag(&file));
            entries.push(name);
        }

        Ok(entries)
    }

    fn list_entries_info(&self, archive_path: &Path) -> Result<Vec<super::ArchiveEntryInfo>, LockError> {
        let file = File::open(archive_path)
            .map_err(|e| LockError::PathNotFound(e.to_string()))?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;

        let mut entries = Vec::new();
        for i in 0..archive.len() {
            let file = archive.by_index(i)
                .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
            let name = encoding::decode_zip_filename(file.name_raw(), utf8_flag(&file));
            entries.push(super::ArchiveEntryInfo {
                is_directory: file.is_dir(),
                name,
                size: file.size() as i64,
            });
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
        use std::io::copy;
        use std::fs::create_dir_all;

        let file = File::open(archive_path)
            .map_err(|e| LockError::PathNotFound(e.to_string()))?;

        let mut archive = ZipArchive::new(file)
            .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;

        let total = archive.len();

        for i in 0..total {
            let mut file = archive.by_index(i)
                .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;

            let decoded_name = encoding::decode_zip_filename(file.name_raw(), utf8_flag(&file));
            progress_fn(i + 1, total, &decoded_name);

            let outpath = output_dir.join(&decoded_name);

            if file.is_dir() {
                create_dir_all(&outpath)
                    .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
            } else {
                if let Some(parent) = outpath.parent() {
                    create_dir_all(parent)
                        .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                }

                let mut outfile = File::create(&outpath)
                    .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;

                copy(&mut file, &mut outfile)
                    .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
            }

            // Set permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode))
                        .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                }
            }
        }

        Ok(())
    }
}

impl ZipBackend {
    pub fn list_entries_info_with_password(&self, archive_path: &Path, password: &str) -> Result<Vec<super::ArchiveEntryInfo>, LockError> {
        use super::ArchiveEntryInfo;
        let file = File::open(archive_path)
            .map_err(|e| LockError::PathNotFound(e.to_string()))?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;

        let mut entries = Vec::new();
        for i in 0..archive.len() {
            match archive.by_index_decrypt(i, password.as_bytes()) {
                Ok(file) => {
                    let name = encoding::decode_zip_filename(file.name_raw(), utf8_flag(&file));
                    entries.push(ArchiveEntryInfo {
                        is_directory: file.is_dir(),
                        name,
                        size: file.size() as i64,
                    });
                }
                Err(zip::result::ZipError::InvalidPassword) => {
                    return Err(LockError::PasswordRequired("Incorrect password".into()));
                }
                Err(e) => {
                    return Err(LockError::ExtractionFailed(e.to_string()));
                }
            }
        }
        Ok(entries)
    }

    pub fn extract_with_password_progress<F>(&self, archive_path: &Path, output_dir: &Path, password: &str, progress_fn: F) -> Result<(), LockError>
    where
        F: Fn(usize, usize, &str)
    {
        use std::io::copy;
        use std::fs::create_dir_all;

        let file = File::open(archive_path)
            .map_err(|e| LockError::PathNotFound(e.to_string()))?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
        let total = archive.len();

        for i in 0..total {
            match archive.by_index_decrypt(i, password.as_bytes()) {
                Ok(mut file) => {
                    let decoded_name = encoding::decode_zip_filename(file.name_raw(), utf8_flag(&file));
                    progress_fn(i + 1, total, &decoded_name);
                    let outpath = output_dir.join(&decoded_name);
                    if file.is_dir() {
                        create_dir_all(&outpath)
                            .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                    } else {
                        if let Some(parent) = outpath.parent() {
                            create_dir_all(parent)
                                .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                        }
                        let mut outfile = File::create(&outpath)
                            .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                        copy(&mut file, &mut outfile)
                            .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
                    }
                }
                Err(zip::result::ZipError::InvalidPassword) => {
                    return Err(LockError::PasswordRequired("Incorrect password".into()));
                }
                Err(e) => {
                    return Err(LockError::ExtractionFailed(e.to_string()));
                }
            }
        }
        Ok(())
    }
}
