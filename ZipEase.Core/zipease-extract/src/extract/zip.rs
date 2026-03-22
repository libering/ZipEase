use std::path::Path;
use std::fs::File;
use zip::ZipArchive;
use zip::HasZipMetadata;
use zipease_shared::LockError;
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

            let outpath = super::safe_join(output_dir, &decoded_name)?;

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
                    let outpath = super::safe_join(output_dir, &decoded_name)?;
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

impl ZipBackend {
    /// Extract ZIP ignoring CRC errors — recovers as many files as possible from corrupt archives.
    /// Uses `by_index_raw` to bypass CRC validation entirely.
    pub fn extract_force_progress<F>(
        &self,
        archive_path: &Path,
        output_dir: &Path,
        progress_fn: F,
    ) -> Result<(), LockError>
    where
        F: Fn(usize, usize, &str),
    {
        use std::io::copy;
        use std::fs::create_dir_all;

        let file = std::fs::File::open(archive_path)
            .map_err(|e| LockError::PathNotFound(e.to_string()))?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;

        let total = archive.len();

        for i in 0..total {
            // by_index_raw skips CRC verification — reads raw compressed bytes
            let mut raw_file = match archive.by_index_raw(i) {
                Ok(f) => f,
                Err(_) => continue, // skip unreadable entries
            };

            let decoded_name = encoding::decode_zip_filename(raw_file.name_raw(), {
                raw_file.get_metadata().is_utf8
            });
            progress_fn(i + 1, total, &decoded_name);

            let outpath = super::safe_join(output_dir, &decoded_name)?;

            if decoded_name.ends_with('/') || decoded_name.ends_with('\\') {
                let _ = create_dir_all(&outpath);
                continue;
            }

            if let Some(parent) = outpath.parent() {
                let _ = create_dir_all(parent);
            }

            if let Ok(mut outfile) = std::fs::File::create(&outpath) {
                let _ = copy(&mut raw_file, &mut outfile);
                // Ignore copy errors — best-effort extraction
            }
        }

        Ok(())
    }

    /// Extract a single entry by index to output_dir.
    /// Returns the extracted file name, or error if index is out of range.
    pub fn extract_entry(
        &self,
        archive_path: &Path,
        entry_index: u32,
        output_dir: &Path,
    ) -> Result<String, LockError> {
        use std::io::copy;
        use std::fs::create_dir_all;

        let file = std::fs::File::open(archive_path)
            .map_err(|e| LockError::PathNotFound(e.to_string()))?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;

        if entry_index as usize >= archive.len() {
            return Err(LockError::ExtractionFailed(format!(
                "Entry index {} out of range (archive has {} entries)",
                entry_index, archive.len()
            )));
        }

        let mut entry = archive.by_index(entry_index as usize)
            .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;

        let decoded_name = encoding::decode_zip_filename(entry.name_raw(), utf8_flag(&entry));
        let outpath = super::safe_join(output_dir, &decoded_name)?;

        if entry.is_dir() {
            create_dir_all(&outpath)
                .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
        } else {
            if let Some(parent) = outpath.parent() {
                create_dir_all(parent)
                    .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
            }
            let mut outfile = std::fs::File::create(&outpath)
                .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
            copy(&mut entry, &mut outfile)
                .map_err(|e| LockError::ExtractionFailed(e.to_string()))?;
        }

        Ok(decoded_name)
    }
}
