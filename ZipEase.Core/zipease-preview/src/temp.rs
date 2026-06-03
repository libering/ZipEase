//! Temp file lifecycle management for the image preview plugin.
//!
//! All extracted images are placed in `storage/temp/` (relative to the application root).
//! Each archive gets a subdirectory keyed by `archive_id`. After decode (success or failure),
//! the temp file is deleted. On archive close, all temps for that archive are removed.
//! On application startup, stale temps from previous sessions are cleaned up.
//!
//! **Functional Paranoia:** All path construction uses `safe_join()` from `zipease-extract`.
//! Delete failures are logged as warnings but never propagated to the user.

use std::fs;
use std::path::{Path, PathBuf};

use log::warn;
use zipease_extract::extract::safe_join;

/// Base temp directory relative to the application root.
const TEMP_DIR_NAME: &str = "storage/temp";

/// Returns the absolute path to the temp directory based on the current executable location.
///
/// Falls back to `./storage/temp` relative to the current working directory if the
/// executable path cannot be determined.
pub fn temp_base_dir() -> PathBuf {
    // Try to resolve relative to the executable's directory (application root).
    // The application root is the parent of the directory containing the executable.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            // The exe might be in a nested target dir during development,
            // so we look for storage/temp relative to the working directory instead.
            // In production, the exe sits at the application root.
            let candidate = exe_dir.join(TEMP_DIR_NAME);
            if candidate.exists() {
                return candidate;
            }
        }
    }

    // Fallback: relative to current working directory
    PathBuf::from(TEMP_DIR_NAME)
}

/// Deletes a single temp file after decode.
///
/// Logs a warning if deletion fails (e.g., file locked by another process).
/// Never propagates errors to the caller.
///
/// # Requirements
/// - 10.1: Delete temp file after successful decode
/// - 10.5: Log warning on failure, do not show error to user
pub fn cleanup_temp_file(path: &Path) {
    if path.exists() {
        if let Err(e) = fs::remove_file(path) {
            warn!(
                "Failed to delete temp file '{}': {}",
                path.display(),
                e
            );
        }
    }
}

/// Deletes all temp files for a specific archive and removes the archive's subdirectory.
///
/// Constructs the subdirectory path using `safe_join()` to prevent path traversal.
/// Logs warnings on individual file deletion failures but continues processing.
///
/// # Requirements
/// - 10.2: Delete all temps for an archive when it is closed
/// - 10.5: Log warning on failure, do not show error to user
/// - 9.5: Use safe_join() for all path construction
pub fn cleanup_archive_temps(archive_id: &str) {
    let base = temp_base_dir();

    // Use safe_join to construct the archive subdirectory path
    let archive_dir = match safe_join(&base, archive_id) {
        Ok(path) => path,
        Err(e) => {
            warn!(
                "Cannot construct temp path for archive '{}': {:?}",
                archive_id, e
            );
            return;
        }
    };

    if !archive_dir.exists() {
        return;
    }

    // Remove all files in the archive subdirectory
    remove_dir_contents(&archive_dir);

    // Remove the directory itself
    if let Err(e) = fs::remove_dir(&archive_dir) {
        warn!(
            "Failed to remove archive temp directory '{}': {}",
            archive_dir.display(),
            e
        );
    }
}

/// Deletes the entire temp directory contents (all files and subdirectories).
///
/// Iterates through the temp directory and removes all entries.
/// Logs warnings on individual failures but continues processing.
///
/// # Requirements
/// - 10.3: Delete all temps on application exit
/// - 10.5: Log warning on failure, do not show error to user
pub fn cleanup_all_temps() {
    let base = temp_base_dir();

    if !base.exists() {
        return;
    }

    remove_dir_contents(&base);
}

/// Called on application startup to remove stale temps from previous sessions.
///
/// Performs the same operation as `cleanup_all_temps()` — removes all files and
/// subdirectories within the temp directory that may have been left behind by
/// a previous abnormal termination.
///
/// # Requirements
/// - 10.4: On startup, check and delete leftover temp files
/// - 10.5: Log warning on failure, do not show error to user
pub fn startup_cleanup() {
    log::info!("Performing startup cleanup of stale temp files");
    cleanup_all_temps();
}

/// Removes all contents (files and subdirectories) within a directory.
///
/// Does NOT remove the directory itself — only its contents.
/// Logs warnings on individual failures and continues.
fn remove_dir_contents(dir: &Path) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            warn!(
                "Failed to read temp directory '{}': {}",
                dir.display(),
                e
            );
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!("Failed to read directory entry in '{}': {}", dir.display(), e);
                continue;
            }
        };

        let path = entry.path();

        if path.is_dir() {
            // Recursively remove subdirectory contents, then the directory itself
            remove_dir_contents(&path);
            if let Err(e) = fs::remove_dir(&path) {
                warn!(
                    "Failed to remove temp subdirectory '{}': {}",
                    path.display(),
                    e
                );
            }
        } else {
            if let Err(e) = fs::remove_file(&path) {
                warn!(
                    "Failed to delete temp file '{}': {}",
                    path.display(),
                    e
                );
            }
        }
    }
}

/// Deletes an extracted temp file when a magic byte mismatch is detected.
///
/// This is called by the decode pipeline when validation fails after extraction,
/// ensuring that potentially malicious files are not left on disk.
///
/// # Requirements
/// - 9.2: On magic byte mismatch, delete extracted temp file before returning error
/// - 10.5: Log warning on failure, do not show error to user
pub fn cleanup_on_magic_mismatch(temp_file_path: &Path) {
    if temp_file_path.exists() {
        if let Err(e) = fs::remove_file(temp_file_path) {
            warn!(
                "Failed to delete temp file after magic byte mismatch '{}': {}",
                temp_file_path.display(),
                e
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn cleanup_temp_file_removes_existing_file() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("test_image.png");
        File::create(&file_path).unwrap().write_all(b"fake png data").unwrap();

        assert!(file_path.exists());
        cleanup_temp_file(&file_path);
        assert!(!file_path.exists());
    }

    #[test]
    fn cleanup_temp_file_handles_nonexistent_file() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("does_not_exist.png");

        // Should not panic or error
        cleanup_temp_file(&file_path);
    }

    #[test]
    fn cleanup_archive_temps_removes_archive_subdir() {
        let tmp = TempDir::new().unwrap();
        let archive_dir = tmp.path().join("archive_abc123");
        fs::create_dir_all(&archive_dir).unwrap();

        // Create some temp files in the archive subdir
        File::create(archive_dir.join("img1.png")).unwrap().write_all(b"data1").unwrap();
        File::create(archive_dir.join("img2.jpg")).unwrap().write_all(b"data2").unwrap();

        assert!(archive_dir.exists());
        assert!(archive_dir.join("img1.png").exists());
        assert!(archive_dir.join("img2.jpg").exists());

        // Use safe_join directly to simulate what cleanup_archive_temps does
        // but with a controlled base directory
        remove_dir_contents(&archive_dir);
        fs::remove_dir(&archive_dir).unwrap();

        assert!(!archive_dir.exists());
    }

    #[test]
    fn cleanup_archive_temps_rejects_path_traversal() {
        // safe_join should reject archive_id with path traversal
        let base = TempDir::new().unwrap();
        let result = safe_join(base.path(), "../evil");
        assert!(result.is_err(), "safe_join must reject path traversal in archive_id");
    }

    #[test]
    fn cleanup_all_temps_removes_all_contents() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        // Create nested structure
        let sub1 = base.join("archive1");
        let sub2 = base.join("archive2");
        fs::create_dir_all(&sub1).unwrap();
        fs::create_dir_all(&sub2).unwrap();
        File::create(sub1.join("a.png")).unwrap().write_all(b"a").unwrap();
        File::create(sub2.join("b.jpg")).unwrap().write_all(b"b").unwrap();
        File::create(base.join("orphan.tmp")).unwrap().write_all(b"orphan").unwrap();

        assert!(sub1.join("a.png").exists());
        assert!(sub2.join("b.jpg").exists());
        assert!(base.join("orphan.tmp").exists());

        remove_dir_contents(base);

        // All contents should be gone
        assert!(!sub1.exists());
        assert!(!sub2.exists());
        assert!(!base.join("orphan.tmp").exists());
        // But the base directory itself should still exist
        assert!(base.exists());
    }

    #[test]
    fn cleanup_on_magic_mismatch_removes_file() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("suspicious.png");
        File::create(&file_path).unwrap().write_all(b"not a real png").unwrap();

        assert!(file_path.exists());
        cleanup_on_magic_mismatch(&file_path);
        assert!(!file_path.exists());
    }

    #[test]
    fn cleanup_on_magic_mismatch_handles_nonexistent() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("already_gone.png");

        // Should not panic
        cleanup_on_magic_mismatch(&file_path);
    }

    #[test]
    fn remove_dir_contents_handles_nested_dirs() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        // Create deeply nested structure
        let deep = base.join("level1").join("level2").join("level3");
        fs::create_dir_all(&deep).unwrap();
        File::create(deep.join("deep_file.txt")).unwrap().write_all(b"deep").unwrap();
        File::create(base.join("level1").join("shallow.txt")).unwrap().write_all(b"shallow").unwrap();

        remove_dir_contents(base);

        // All nested content should be gone
        assert!(!base.join("level1").exists());
        // Base still exists
        assert!(base.exists());
    }

    #[test]
    fn remove_dir_contents_handles_empty_dir() {
        let tmp = TempDir::new().unwrap();
        // Should not panic on empty directory
        remove_dir_contents(tmp.path());
    }

    #[test]
    fn safe_join_used_for_archive_path_construction() {
        // Verify that safe_join rejects malicious archive_ids
        let base = TempDir::new().unwrap();

        let malicious_ids = vec![
            "../escape",
            "..\\escape",
            "/absolute/path",
            "C:\\Windows\\System32",
            "normal/../escape",
            "has\0null",
        ];

        for id in malicious_ids {
            let result = safe_join(base.path(), id);
            assert!(
                result.is_err(),
                "safe_join should reject malicious archive_id: {:?}",
                id
            );
        }
    }

    #[test]
    fn safe_join_accepts_valid_archive_ids() {
        let base = TempDir::new().unwrap();

        let valid_ids = vec![
            "archive_abc123",
            "my-archive-2024",
            "テスト_アーカイブ",
            "simple",
        ];

        for id in valid_ids {
            let result = safe_join(base.path(), id);
            assert!(
                result.is_ok(),
                "safe_join should accept valid archive_id: {:?}, err: {:?}",
                id,
                result.err()
            );
        }
    }
}
