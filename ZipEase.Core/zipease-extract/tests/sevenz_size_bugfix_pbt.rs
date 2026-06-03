//! Bug condition exploration tests for 7z file sizes hardcoded to -1.
//!
//! These tests verify that `SevenZBackend::list_entries_info` and
//! `SevenZBackend::list_entries_info_with_password` return the actual
//! uncompressed size for file entries in 7z archives.
//!
//! **IMPORTANT**: These tests are written BEFORE the fix is applied.
//! They are EXPECTED TO FAIL on unfixed code — failure confirms the bug exists.
//!
//! Bug Condition: input.archiveFormat == "7z" AND returnedSize(input.entry) == -1
//! Expected Behavior: entry.size == actualUncompressedSize(entry) for non-directory entries
//!
//! **Validates: Requirements 1.1, 1.2, 1.3**

use std::fs;
use std::io::Write;
use std::path::Path;
use proptest::prelude::*;
use tempfile::TempDir;
use zipease_extract::extract::sevenz::SevenZBackend;
use zipease_extract::extract::ExtractionBackend;

/// Helper: create a temporary file with the given content.
fn create_temp_file(dir: &Path, name: &str, content: &[u8]) -> std::path::PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut f = fs::File::create(&path).unwrap();
    f.write_all(content).unwrap();
    f.flush().unwrap();
    path
}

/// Helper: create a 7z archive from a set of files using sevenz-rust.
/// Returns the path to the created archive.
fn create_7z_archive(files: &[(&str, &[u8])], archive_path: &Path) {
    // Create a temp directory with the source files
    let src_dir = TempDir::new().unwrap();
    for (name, content) in files {
        create_temp_file(src_dir.path(), name, content);
    }

    // Use sevenz-rust SevenZWriter to create the archive
    let mut writer = sevenz_rust::SevenZWriter::create(archive_path)
        .expect("Failed to create 7z archive");

    for (name, _content) in files {
        let file_path = src_dir.path().join(name);
        writer.push_source_path(&file_path, |_| true)
            .expect("Failed to add file to 7z archive");
    }

    writer.finish().expect("Failed to finalize 7z archive");
}

/// Helper: create a password-protected 7z archive.
/// Note: sevenz-rust 0.6.1 may not support password-protected archive creation.
/// If not available, we create a regular archive and test with password API anyway
/// (the API should still be able to list entries from non-encrypted archives).
fn create_7z_archive_with_password(files: &[(&str, &[u8])], archive_path: &Path, _password: &str) {
    // sevenz-rust 0.6.1 does not expose password-protected archive creation.
    // We create a regular archive — the list_entries_info_with_password function
    // should still work on non-encrypted archives (it just passes the password
    // to the reader which ignores it for non-encrypted content).
    create_7z_archive(files, archive_path);
}

// ===========================================================================
// Property 1: Bug Condition — 7z File Sizes Hardcoded to -1
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    /// **Validates: Requirements 1.1**
    ///
    /// Property: For any 7z archive containing file entries with known sizes,
    /// `SevenZBackend::list_entries_info` SHALL return `size > 0` for all
    /// non-directory entries.
    ///
    /// Bug Condition: input.archiveFormat == "7z" AND returnedSize(input.entry) == -1
    ///
    /// On UNFIXED code, this test FAILS because size is hardcoded to -1.
    #[test]
    fn prop_sevenz_list_entries_info_returns_actual_size(
        file_size in 1usize..=65536usize,
    ) {
        // Create a file with known size
        let content: Vec<u8> = vec![0xAB; file_size];
        let out_dir = TempDir::new().unwrap();
        let archive_path = out_dir.path().join("test.7z");

        create_7z_archive(&[("testfile.bin", &content)], &archive_path);

        // List entries using the SevenZBackend
        let backend = SevenZBackend;
        let entries = backend.list_entries_info(&archive_path)
            .expect("list_entries_info should succeed");

        // Find the file entry (not directory)
        let file_entries: Vec<_> = entries.iter()
            .filter(|e| !e.is_directory)
            .collect();

        prop_assert!(!file_entries.is_empty(), "Should have at least one file entry");

        // Assert: for all non-directory entries, size must equal the actual uncompressed size
        for entry in &file_entries {
            prop_assert!(
                entry.size > 0,
                "BUG CONFIRMED: list_entries_info returns size: {} for a {}-byte file '{}'. \
                 Expected size: {}",
                entry.size, file_size, entry.name, file_size
            );
            prop_assert_eq!(
                entry.size as usize, file_size,
                "Size mismatch: entry '{}' reports size {} but actual size is {}",
                entry.name, entry.size, file_size
            );
        }
    }

    /// **Validates: Requirements 1.3**
    ///
    /// Property: For any 7z archive listed via `list_entries_info_with_password`,
    /// non-directory entries SHALL have `size > 0` reflecting actual content size.
    ///
    /// On UNFIXED code, this test FAILS because size is hardcoded to -1.
    #[test]
    fn prop_sevenz_list_entries_info_with_password_returns_actual_size(
        file_size in 1usize..=65536usize,
    ) {
        let content: Vec<u8> = vec![0xCD; file_size];
        let out_dir = TempDir::new().unwrap();
        let archive_path = out_dir.path().join("test_pw.7z");

        // Create archive (non-encrypted since sevenz-rust doesn't support encrypted creation)
        create_7z_archive_with_password(&[("secret.bin", &content)], &archive_path, "testpass");

        // List entries using the password variant
        let backend = SevenZBackend;
        let entries = backend.list_entries_info_with_password(&archive_path, "testpass")
            .expect("list_entries_info_with_password should succeed");

        // Find file entries
        let file_entries: Vec<_> = entries.iter()
            .filter(|e| !e.is_directory)
            .collect();

        prop_assert!(!file_entries.is_empty(), "Should have at least one file entry");

        // Assert: for all non-directory entries, size must equal actual uncompressed size
        for entry in &file_entries {
            prop_assert!(
                entry.size > 0,
                "BUG CONFIRMED: list_entries_info_with_password returns size: {} for a {}-byte file '{}'. \
                 Expected size: {}",
                entry.size, file_size, entry.name, file_size
            );
            prop_assert_eq!(
                entry.size as usize, file_size,
                "Size mismatch: entry '{}' reports size {} but actual size is {}",
                entry.name, entry.size, file_size
            );
        }
    }

    /// **Validates: Requirements 1.1**
    ///
    /// Property: For a 7z archive with multiple files of varying sizes,
    /// ALL non-directory entries must report their actual uncompressed size.
    ///
    /// On UNFIXED code, this test FAILS because ALL entries return size: -1.
    #[test]
    fn prop_sevenz_multiple_files_all_sizes_correct(
        size_a in 1usize..=32768usize,
        size_b in 1usize..=32768usize,
    ) {
        let content_a: Vec<u8> = vec![0x11; size_a];
        let content_b: Vec<u8> = vec![0x22; size_b];
        let out_dir = TempDir::new().unwrap();
        let archive_path = out_dir.path().join("multi.7z");

        create_7z_archive(
            &[("file_a.dat", &content_a), ("file_b.dat", &content_b)],
            &archive_path,
        );

        let backend = SevenZBackend;
        let entries = backend.list_entries_info(&archive_path)
            .expect("list_entries_info should succeed");

        let file_entries: Vec<_> = entries.iter()
            .filter(|e| !e.is_directory)
            .collect();

        prop_assert!(
            file_entries.len() >= 2,
            "Expected at least 2 file entries, got {}",
            file_entries.len()
        );

        // All file entries must have size > 0
        for entry in &file_entries {
            prop_assert!(
                entry.size > 0,
                "BUG CONFIRMED: list_entries_info returns size: {} for file '{}'. \
                 All file entries return -1 regardless of actual content size.",
                entry.size, entry.name
            );
        }
    }
}

// ===========================================================================
// Concrete unit test — provides a clear, deterministic counterexample
// ===========================================================================

/// **Validates: Requirements 1.1**
///
/// Concrete test: a 1024-byte file in a 7z archive must report size: 1024.
/// On UNFIXED code, this FAILS with size: -1.
#[test]
fn test_sevenz_1024_byte_file_reports_correct_size() {
    let content = vec![0xFF; 1024];
    let out_dir = TempDir::new().unwrap();
    let archive_path = out_dir.path().join("concrete.7z");

    create_7z_archive(&[("readme.txt", &content)], &archive_path);

    let backend = SevenZBackend;
    let entries = backend.list_entries_info(&archive_path)
        .expect("list_entries_info should succeed");

    let file_entries: Vec<_> = entries.iter()
        .filter(|e| !e.is_directory)
        .collect();

    assert!(!file_entries.is_empty(), "Should have at least one file entry");

    for entry in &file_entries {
        assert!(
            entry.size > 0,
            "BUG CONFIRMED: list_entries_info returns size: {} for a 1024-byte file '{}'. \
             Expected: size == 1024. The sevenz-rust backend hardcodes size: -1.",
            entry.size, entry.name
        );
        assert_eq!(
            entry.size, 1024,
            "Size mismatch for '{}': got {}, expected 1024",
            entry.name, entry.size
        );
    }
}

/// **Validates: Requirements 1.2**
///
/// Note on SevenZaDllBackendWithClsid: This backend requires COM/DLL loading
/// (7za.dll) which may not be available in the test environment. The bug
/// condition is the same — KPID_SIZE is never queried, so size is always -1.
/// The sevenz-rust backend test above confirms the pattern. The sevenzadll
/// fix will be validated in task 3.5 after implementation.
///
/// If the DLL is available, this test would call:
///   SevenZaDllBackendWithClsid::list_entries_info(&archive_path)
/// and assert size > 0 for file entries.
#[test]
fn test_sevenzadll_limitation_documented() {
    // This test documents that SevenZaDllBackendWithClsid cannot be tested
    // in a pure unit test environment because it requires:
    // 1. 7za.dll to be present and loadable
    // 2. COM initialization
    // 3. CLSID registration
    //
    // The bug condition is identical: KPID_SIZE (property 7) is never queried,
    // so all entries return size: -1.
    //
    // The fix (task 3.3 + 3.4) will add KPID_SIZE querying to the backend.
    // Verification will happen in task 3.5 via integration testing.
    eprintln!(
        "NOTE: SevenZaDllBackendWithClsid requires COM/DLL loading. \
         Bug condition is the same as sevenz-rust: size is hardcoded to -1. \
         See design.md for details on KPID_SIZE (property 7) not being queried."
    );
}
