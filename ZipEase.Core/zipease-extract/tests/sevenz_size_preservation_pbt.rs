//! Preservation property tests for the 7z size bugfix.
//!
//! These tests capture baseline behavior that MUST remain unchanged after the fix:
//! - ZIP/APK archives continue to display correct file sizes
//! - 7z directory entries continue to report `size: -1`
//! - 7z entry names and `is_directory` flags remain correct
//!
//! **IMPORTANT**: These tests are written BEFORE the fix is applied.
//! They MUST PASS on unfixed code — they capture behavior to preserve.
//!
//! **Validates: Requirements 3.1, 3.2, 3.3, 3.4**

use std::fs;
use std::io::Write;
use std::path::Path;
use proptest::prelude::*;
use tempfile::TempDir;
use zipease_extract::extract::zip::ZipBackend;
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

/// Helper: create a ZIP archive from a set of files.
fn create_zip_archive(files: &[(&str, &[u8])], archive_path: &Path) {
    let file = fs::File::create(archive_path).unwrap();
    let mut zip_writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    for (name, content) in files {
        zip_writer.start_file(*name, options).unwrap();
        zip_writer.write_all(content).unwrap();
    }
    zip_writer.finish().unwrap();
}

/// Helper: create a ZIP archive with directories.
fn create_zip_archive_with_dirs(
    dirs: &[&str],
    files: &[(&str, &[u8])],
    archive_path: &Path,
) {
    let file = fs::File::create(archive_path).unwrap();
    let mut zip_writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    for dir in dirs {
        zip_writer.add_directory(*dir, options).unwrap();
    }
    for (name, content) in files {
        zip_writer.start_file(*name, options).unwrap();
        zip_writer.write_all(content).unwrap();
    }
    zip_writer.finish().unwrap();
}

/// Helper: create a 7z archive using push_source_path on a directory.
/// This produces entries with correct relative path names.
fn create_7z_archive_from_dir(files: &[(&str, &[u8])], archive_path: &Path) {
    let src_dir = TempDir::new().unwrap();
    for (name, content) in files {
        create_temp_file(src_dir.path(), name, content);
    }

    let mut writer = sevenz_rust::SevenZWriter::create(archive_path)
        .expect("Failed to create 7z archive");

    writer.push_source_path(src_dir.path(), |_| true)
        .expect("Failed to add source path to 7z archive");

    writer.finish().expect("Failed to finalize 7z archive");
}

/// Helper: create a 7z archive with explicit directory entries using push_archive_entry.
fn create_7z_archive_with_dir_entries(
    dirs: &[&str],
    files: &[(&str, &[u8])],
    archive_path: &Path,
) {
    let mut writer = sevenz_rust::SevenZWriter::create(archive_path)
        .expect("Failed to create 7z archive");

    // Add directory entries
    for dir_name in dirs {
        let mut dir_entry = sevenz_rust::SevenZArchiveEntry::default();
        dir_entry.name = dir_name.to_string();
        dir_entry.is_directory = true;
        writer.push_archive_entry(dir_entry, None::<&[u8]>).unwrap();
    }

    // Add file entries
    for (name, content) in files {
        let src_dir = TempDir::new().unwrap();
        let file_path = create_temp_file(src_dir.path(), "tmp", content);
        let entry = sevenz_rust::SevenZArchiveEntry::from_path(
            &file_path,
            name.to_string(),
        );
        writer.push_archive_entry(entry, Some(*content)).unwrap();
    }

    writer.finish().expect("Failed to finalize 7z archive");
}

// ===========================================================================
// Property 2.1: ZIP Size Preservation
// For all ZIP archive entries, entry.size matches the actual uncompressed size
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    /// **Validates: Requirements 3.1**
    ///
    /// Property: For any ZIP archive containing files with known sizes,
    /// `ZipBackend::list_entries_info` SHALL return `entry.size` equal to
    /// the actual uncompressed size for all file entries.
    ///
    /// This test MUST PASS on both unfixed and fixed code — ZIP behavior is unchanged.
    #[test]
    fn prop_zip_list_entries_info_returns_correct_sizes(
        file_size in 0usize..=32768usize,
    ) {
        let content: Vec<u8> = vec![0x42; file_size];
        let out_dir = TempDir::new().unwrap();
        let archive_path = out_dir.path().join("test.zip");

        create_zip_archive(&[("testfile.bin", &content)], &archive_path);

        let backend = ZipBackend;
        let entries = backend.list_entries_info(&archive_path)
            .expect("list_entries_info should succeed for ZIP");

        let file_entries: Vec<_> = entries.iter()
            .filter(|e| !e.is_directory)
            .collect();

        prop_assert!(!file_entries.is_empty(), "Should have at least one file entry");

        for entry in &file_entries {
            prop_assert_eq!(
                entry.size as usize, file_size,
                "ZIP size mismatch: entry '{}' reports size {} but actual size is {}",
                entry.name, entry.size, file_size
            );
        }
    }

    /// **Validates: Requirements 3.1**
    ///
    /// Property: For a ZIP archive with multiple files of varying sizes,
    /// ALL entries must report their actual uncompressed size.
    #[test]
    fn prop_zip_multiple_files_all_sizes_correct(
        size_a in 0usize..=16384usize,
        size_b in 0usize..=16384usize,
    ) {
        let content_a: Vec<u8> = vec![0xAA; size_a];
        let content_b: Vec<u8> = vec![0xBB; size_b];
        let out_dir = TempDir::new().unwrap();
        let archive_path = out_dir.path().join("multi.zip");

        create_zip_archive(
            &[("alpha.dat", &content_a), ("beta.dat", &content_b)],
            &archive_path,
        );

        let backend = ZipBackend;
        let entries = backend.list_entries_info(&archive_path)
            .expect("list_entries_info should succeed for ZIP");

        let file_entries: Vec<_> = entries.iter()
            .filter(|e| !e.is_directory)
            .collect();

        prop_assert!(
            file_entries.len() >= 2,
            "Expected at least 2 file entries, got {}",
            file_entries.len()
        );

        // Find entries by name and verify sizes
        for entry in &file_entries {
            let expected_size = if entry.name == "alpha.dat" {
                size_a
            } else if entry.name == "beta.dat" {
                size_b
            } else {
                continue;
            };
            prop_assert_eq!(
                entry.size as usize, expected_size,
                "ZIP size mismatch for '{}': got {}, expected {}",
                entry.name, entry.size, expected_size
            );
        }
    }
}

// ===========================================================================
// Property 2.2: 7z Directory Entry Preservation
// For all 7z directory entries, entry.size == -1 AND entry.is_directory == true
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    /// **Validates: Requirements 3.4**
    ///
    /// Property: For any 7z archive containing directory entries,
    /// `SevenZBackend::list_entries_info` SHALL return `size: -1` and
    /// `is_directory: true` for all directory entries.
    ///
    /// This test MUST PASS on both unfixed and fixed code — directory behavior
    /// must be preserved (directories have no meaningful uncompressed size).
    #[test]
    fn prop_sevenz_directory_entries_report_size_minus_one(
        file_size in 1usize..=8192usize,
    ) {
        let content: Vec<u8> = vec![0xDD; file_size];
        let out_dir = TempDir::new().unwrap();
        let archive_path = out_dir.path().join("dirs.7z");

        // Create archive with explicit directory entries using push_archive_entry
        create_7z_archive_with_dir_entries(
            &["subdir"],
            &[("subdir/nested.bin", &content)],
            &archive_path,
        );

        let backend = SevenZBackend;
        let entries = backend.list_entries_info(&archive_path)
            .expect("list_entries_info should succeed for 7z");

        // Find directory entries
        let dir_entries: Vec<_> = entries.iter()
            .filter(|e| e.is_directory)
            .collect();

        // Must have at least one directory entry
        prop_assert!(
            !dir_entries.is_empty(),
            "Expected at least one directory entry in 7z archive, got entries: {:?}",
            entries.iter().map(|e| format!("{}(dir={})", e.name, e.is_directory)).collect::<Vec<_>>()
        );

        // Verify all directory entries have size: -1
        for entry in &dir_entries {
            prop_assert_eq!(
                entry.size, -1_i64,
                "7z directory entry '{}' should have size: -1, got: {}",
                entry.name, entry.size
            );
            prop_assert!(
                entry.is_directory,
                "Entry '{}' filtered as directory should have is_directory: true",
                entry.name
            );
        }
    }
}

// ===========================================================================
// Property 2.3: 7z Entry Name Preservation
// For all 7z entries, entry.name matches the original file name
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    /// **Validates: Requirements 3.2**
    ///
    /// Property: For any 7z archive, `SevenZBackend::list_entries_info` SHALL
    /// return entry names that match the original file names used when creating
    /// the archive, and `is_directory` flags must be correct.
    ///
    /// This test MUST PASS on both unfixed and fixed code — names and directory
    /// flags are unrelated to the size bug.
    #[test]
    fn prop_sevenz_entry_names_match_original(
        file_size in 1usize..=4096usize,
    ) {
        let content: Vec<u8> = vec![0xEE; file_size];
        let out_dir = TempDir::new().unwrap();
        let archive_path = out_dir.path().join("names.7z");

        // Use push_source_path on a directory to get proper relative names
        create_7z_archive_from_dir(&[("myfile.txt", &content)], &archive_path);

        let backend = SevenZBackend;
        let entries = backend.list_entries_info(&archive_path)
            .expect("list_entries_info should succeed for 7z");

        // Find non-directory entries
        let file_entries: Vec<_> = entries.iter()
            .filter(|e| !e.is_directory)
            .collect();

        prop_assert!(!file_entries.is_empty(), "Should have at least one file entry");

        // The file entry name should be "myfile.txt"
        let found = file_entries.iter().any(|e| e.name == "myfile.txt");
        prop_assert!(
            found,
            "Expected to find entry with name 'myfile.txt', got entries: {:?}",
            file_entries.iter().map(|e| &e.name).collect::<Vec<_>>()
        );

        // Verify is_directory is false for file entries
        for entry in &file_entries {
            prop_assert!(
                !entry.is_directory,
                "File entry '{}' should have is_directory: false",
                entry.name
            );
        }
    }

    /// **Validates: Requirements 3.2**
    ///
    /// Property: For a 7z archive with multiple files, ALL entry names must
    /// be present and correctly identified as files (not directories).
    #[test]
    fn prop_sevenz_multiple_entry_names_preserved(
        size_a in 1usize..=4096usize,
        size_b in 1usize..=4096usize,
    ) {
        let content_a: Vec<u8> = vec![0x11; size_a];
        let content_b: Vec<u8> = vec![0x22; size_b];
        let out_dir = TempDir::new().unwrap();
        let archive_path = out_dir.path().join("multi_names.7z");

        // Use push_source_path on a directory to get proper relative names
        create_7z_archive_from_dir(
            &[("first.dat", &content_a), ("second.dat", &content_b)],
            &archive_path,
        );

        let backend = SevenZBackend;
        let entries = backend.list_entries_info(&archive_path)
            .expect("list_entries_info should succeed for 7z");

        let file_entries: Vec<_> = entries.iter()
            .filter(|e| !e.is_directory)
            .collect();

        // Both files should be present
        let has_first = file_entries.iter().any(|e| e.name == "first.dat");
        let has_second = file_entries.iter().any(|e| e.name == "second.dat");

        prop_assert!(
            has_first,
            "Expected to find 'first.dat' in entries: {:?}",
            file_entries.iter().map(|e| &e.name).collect::<Vec<_>>()
        );
        prop_assert!(
            has_second,
            "Expected to find 'second.dat' in entries: {:?}",
            file_entries.iter().map(|e| &e.name).collect::<Vec<_>>()
        );

        // All file entries should have is_directory: false
        for entry in &file_entries {
            prop_assert!(
                !entry.is_directory,
                "File entry '{}' should have is_directory: false",
                entry.name
            );
        }
    }
}

// ===========================================================================
// Concrete unit tests — deterministic baseline checks
// ===========================================================================

/// **Validates: Requirements 3.1**
///
/// Concrete test: a 512-byte file in a ZIP archive must report size: 512.
#[test]
fn test_zip_512_byte_file_reports_correct_size() {
    let content = vec![0xFF; 512];
    let out_dir = TempDir::new().unwrap();
    let archive_path = out_dir.path().join("concrete.zip");

    create_zip_archive(&[("data.bin", &content)], &archive_path);

    let backend = ZipBackend;
    let entries = backend.list_entries_info(&archive_path)
        .expect("list_entries_info should succeed for ZIP");

    let file_entries: Vec<_> = entries.iter()
        .filter(|e| !e.is_directory)
        .collect();

    assert!(!file_entries.is_empty(), "Should have at least one file entry");
    assert_eq!(file_entries[0].size, 512, "ZIP entry should report size: 512");
    assert_eq!(file_entries[0].name, "data.bin");
    assert!(!file_entries[0].is_directory);
}

/// **Validates: Requirements 3.1**
///
/// Concrete test: an empty file (0 bytes) in a ZIP archive must report size: 0.
#[test]
fn test_zip_empty_file_reports_size_zero() {
    let content: Vec<u8> = vec![];
    let out_dir = TempDir::new().unwrap();
    let archive_path = out_dir.path().join("empty.zip");

    create_zip_archive(&[("empty.txt", &content)], &archive_path);

    let backend = ZipBackend;
    let entries = backend.list_entries_info(&archive_path)
        .expect("list_entries_info should succeed for ZIP");

    let file_entries: Vec<_> = entries.iter()
        .filter(|e| !e.is_directory)
        .collect();

    assert!(!file_entries.is_empty());
    assert_eq!(file_entries[0].size, 0, "Empty ZIP entry should report size: 0");
}

/// **Validates: Requirements 3.4**
///
/// Concrete test: 7z directory entries report size: -1 and is_directory: true.
#[test]
fn test_sevenz_directory_entry_size_minus_one() {
    let content = vec![0xAB; 256];
    let out_dir = TempDir::new().unwrap();
    let archive_path = out_dir.path().join("with_dirs.7z");

    create_7z_archive_with_dir_entries(
        &["folder"],
        &[("folder/inner.bin", &content)],
        &archive_path,
    );

    let backend = SevenZBackend;
    let entries = backend.list_entries_info(&archive_path)
        .expect("list_entries_info should succeed for 7z");

    let dir_entries: Vec<_> = entries.iter()
        .filter(|e| e.is_directory)
        .collect();

    assert!(
        !dir_entries.is_empty(),
        "Expected at least one directory entry in 7z archive"
    );

    for entry in &dir_entries {
        assert_eq!(
            entry.size, -1,
            "7z directory entry '{}' should have size: -1, got: {}",
            entry.name, entry.size
        );
        assert!(entry.is_directory, "Directory entry should have is_directory: true");
    }
}

/// **Validates: Requirements 3.2**
///
/// Concrete test: 7z entry names match original file names.
#[test]
fn test_sevenz_entry_names_correct() {
    let content = vec![0xCC; 128];
    let out_dir = TempDir::new().unwrap();
    let archive_path = out_dir.path().join("names.7z");

    // Use push_source_path on a directory for proper names
    create_7z_archive_from_dir(&[("hello.txt", &content)], &archive_path);

    let backend = SevenZBackend;
    let entries = backend.list_entries_info(&archive_path)
        .expect("list_entries_info should succeed for 7z");

    let file_entries: Vec<_> = entries.iter()
        .filter(|e| !e.is_directory)
        .collect();

    assert!(!file_entries.is_empty(), "Should have at least one file entry");

    let found = file_entries.iter().any(|e| e.name == "hello.txt");
    assert!(
        found,
        "Expected to find 'hello.txt' in entries: {:?}",
        file_entries.iter().map(|e| &e.name).collect::<Vec<_>>()
    );
}

/// **Validates: Requirements 3.1**
///
/// Concrete test: ZIP directory entries report size: 0 and is_directory: true.
#[test]
fn test_zip_directory_entry_preserved() {
    let content = vec![0x55; 100];
    let out_dir = TempDir::new().unwrap();
    let archive_path = out_dir.path().join("zip_dirs.zip");

    create_zip_archive_with_dirs(
        &["mydir/"],
        &[("mydir/file.txt", &content)],
        &archive_path,
    );

    let backend = ZipBackend;
    let entries = backend.list_entries_info(&archive_path)
        .expect("list_entries_info should succeed for ZIP");

    let dir_entries: Vec<_> = entries.iter()
        .filter(|e| e.is_directory)
        .collect();

    assert!(!dir_entries.is_empty(), "Should have at least one directory entry");

    for entry in &dir_entries {
        assert!(entry.is_directory, "Directory entry should have is_directory: true");
        // ZIP directories report size: 0 (not -1)
        assert_eq!(
            entry.size, 0,
            "ZIP directory entry '{}' should have size: 0, got: {}",
            entry.name, entry.size
        );
    }
}
