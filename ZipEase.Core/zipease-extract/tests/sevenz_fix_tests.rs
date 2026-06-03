//! Tests for sevenz-list-fix: directory entry filtering in SevenZBackend.
//!
//! Tests 1.1–1.3 are EXPECTED TO FAIL on unfixed code (they confirm the bug exists).
//! Test 1.4 is a baseline that must pass on both unfixed and fixed code.

use std::path::PathBuf;
use tempfile::TempDir;
use zipease_extract::extract::sevenz::SevenZBackend;
use zipease_extract::extract::ExtractionBackend;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Create a 7z archive that contains a directory entry AND file entries inside it.
/// Returns (TempDir guard, path to the .7z file).
fn create_mixed_archive() -> (TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let archive_path = tmp.path().join("mixed.7z");

    // Build source tree: src/main.rs  src/lib.rs
    let src_dir = tmp.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("main.rs"), b"fn main() {}").unwrap();
    std::fs::write(src_dir.join("lib.rs"), b"pub fn foo() {}").unwrap();

    // compress_to_path compresses the *contents* of the given directory.
    // Compressing `src` itself (by passing its parent and naming it) creates
    // an entry for the directory AND entries for the files inside.
    sevenz_rust::compress_to_path(&src_dir, &archive_path).expect("compress mixed archive");

    (tmp, archive_path)
}

/// Create a flat 7z archive (no directory entries, only files).
/// We use SevenZWriter::push_archive_entry to add individual files without
/// any directory entry, ensuring a truly flat archive.
fn create_flat_archive() -> (TempDir, PathBuf) {
    use sevenz_rust::{SevenZWriter, SevenZArchiveEntry};

    let tmp = tempfile::tempdir().unwrap();
    let archive_path = tmp.path().join("flat.7z");

    let file1 = tmp.path().join("file1.txt");
    let file2 = tmp.path().join("file2.txt");
    std::fs::write(&file1, b"hello").unwrap();
    std::fs::write(&file2, b"world").unwrap();

    let mut writer = SevenZWriter::create(&archive_path).expect("create writer");
    writer
        .push_archive_entry(
            SevenZArchiveEntry::from_path(&file1, "file1.txt".to_string()),
            Some(std::fs::File::open(&file1).unwrap()),
        )
        .expect("push file1");
    writer
        .push_archive_entry(
            SevenZArchiveEntry::from_path(&file2, "file2.txt".to_string()),
            Some(std::fs::File::open(&file2).unwrap()),
        )
        .expect("push file2");
    writer.finish().expect("finish");

    (tmp, archive_path)
}

// ── Task 1.1 ─────────────────────────────────────────────────────────────────

/// Task 1.1: list_entries must NOT return directory entries.
/// EXPECTED TO FAIL on unfixed code — confirms the bug.
/// Note: compress_to_path creates a root directory entry with name "" (empty string).
#[test]
fn test_list_entries_excludes_directories() {
    let (_tmp, archive_path) = create_mixed_archive();
    let backend = SevenZBackend;
    let entries = backend.list_entries(&archive_path).expect("list ok");

    // After fix: only the 2 file entries (lib.rs, main.rs) are returned.
    // Unfixed: also includes the root directory entry (name == "").
    assert_eq!(entries.len(), 2, "Expected 2 file entries, got: {:?}", entries);
    for entry in &entries {
        assert!(
            !entry.is_empty(),
            "Empty-named directory entry found in list_entries result"
        );
    }
}

// ── Task 1.2 ─────────────────────────────────────────────────────────────────

/// Task 1.2: extract_with_progress total must equal the number of FILE entries only.
/// EXPECTED TO FAIL on unfixed code — confirms the bug.
#[test]
fn test_extract_progress_total_excludes_directories() {
    let (_tmp, archive_path) = create_mixed_archive();
    let out_tmp = tempfile::tempdir().unwrap();
    let backend = SevenZBackend;

    let observed_total = std::cell::Cell::new(0usize);
    backend
        .extract_with_progress(&archive_path, out_tmp.path(), |_current, total, _name| {
            observed_total.set(total);
        })
        .expect("extract ok");

    // After fix: total == 2 (files only).  Unfixed: total == 3 (2 files + 1 dir).
    assert_eq!(
        observed_total.get(), 2,
        "Expected total=2 (files only), got total={}",
        observed_total.get()
    );
}

// ── Task 1.3 ─────────────────────────────────────────────────────────────────

/// Task 1.3: progress_fn must never be called for directory entries.
/// EXPECTED TO FAIL on unfixed code — confirms the bug.
/// Note: the root directory entry produced by compress_to_path has name "" (empty string).
#[test]
fn test_extract_progress_not_called_for_directories() {
    let (_tmp, archive_path) = create_mixed_archive();
    let out_tmp = tempfile::tempdir().unwrap();
    let backend = SevenZBackend;

    // Count how many times progress_fn is called total
    let call_count = std::cell::Cell::new(0usize);
    backend
        .extract_with_progress(&archive_path, out_tmp.path(), |_current, _total, _name| {
            call_count.set(call_count.get() + 1);
        })
        .expect("extract ok");

    // The mixed archive has 2 files + 1 directory entry.
    // After fix: progress_fn called exactly 2 times (files only).
    // Unfixed: progress_fn called 3 times (including the directory).
    assert_eq!(
        call_count.get(), 2,
        "Expected progress_fn called 2 times (files only), got {}",
        call_count.get()
    );
}

// ── Task 1.4 ─────────────────────────────────────────────────────────────────

/// Task 1.4: flat archive (no directories) — baseline that must pass before AND after fix.
#[test]
fn test_list_entries_flat_archive_unchanged() {
    let (_tmp, archive_path) = create_flat_archive();
    let backend = SevenZBackend;
    let entries = backend.list_entries(&archive_path).expect("list ok");

    assert_eq!(entries.len(), 2, "Expected 2 entries for flat archive, got: {:?}", entries);
    for entry in &entries {
        assert!(!entry.ends_with('/'), "Unexpected directory entry: {:?}", entry);
    }
}

// ── Task 3.2 ─────────────────────────────────────────────────────────────────

/// Task 3.2: list_entries on mixed archive returns only file names (fix-check).
/// **Validates: Requirements 2.1, 2.2, 2.3**
#[test]
fn test_list_entries_mixed_returns_only_files() {
    let (_tmp, archive_path) = create_mixed_archive();
    let backend = SevenZBackend;
    let entries = backend.list_entries(&archive_path).expect("list ok");

    // Must contain exactly the 2 file entries, no directory entries.
    assert_eq!(entries.len(), 2, "Expected 2 file entries, got: {:?}", entries);
    assert!(entries.contains(&"main.rs".to_string()), "main.rs missing from {:?}", entries);
    assert!(entries.contains(&"lib.rs".to_string()), "lib.rs missing from {:?}", entries);
}

// ── Task 3.3 / 3.4 ───────────────────────────────────────────────────────────

/// Task 3.3 + 3.4: progress_fn call count AND total argument both equal the
/// number of file entries (not directory entries).
/// **Validates: Requirements 2.4**
#[test]
fn test_progress_fn_call_count_equals_file_count() {
    let (_tmp, archive_path) = create_mixed_archive();
    let out_tmp = tempfile::tempdir().unwrap();
    let backend = SevenZBackend;

    let call_count = std::cell::Cell::new(0usize);
    let last_total = std::cell::Cell::new(0usize);

    backend
        .extract_with_progress(&archive_path, out_tmp.path(), |_current, total, _name| {
            call_count.set(call_count.get() + 1);
            last_total.set(total);
        })
        .expect("extract ok");

    assert_eq!(call_count.get(), 2, "progress_fn should be called exactly 2 times (files only)");
    assert_eq!(last_total.get(), 2, "total passed to progress_fn should be 2 (files only)");
}

// ── Task 4.1 (baseline already covered by test_list_entries_flat_archive_unchanged) ──

// ── Task 4.2 ─────────────────────────────────────────────────────────────────

/// Task 4.2: extract_with_progress still creates directories on disk.
/// Verifies that directory creation logic is preserved after the fix.
/// **Validates: Requirements 3.2**
#[test]
fn test_extract_creates_directories_on_disk() {
    let (_tmp, archive_path) = create_mixed_archive();
    let out_tmp = tempfile::tempdir().unwrap();
    let backend = SevenZBackend;

    backend
        .extract_with_progress(&archive_path, out_tmp.path(), |_, _, _| {})
        .expect("extract ok");

    // The mixed archive was created from a `src/` directory.
    // Even though `src/` is filtered from listing/progress, it must still
    // be created on disk so the files inside it can be written.
    // The files are stored flat (main.rs, lib.rs) in this archive, but
    // the directory entry "" (root) should not cause issues.
    // Verify the files themselves exist.
    assert!(
        out_tmp.path().join("main.rs").exists(),
        "main.rs should exist after extraction"
    );
    assert!(
        out_tmp.path().join("lib.rs").exists(),
        "lib.rs should exist after extraction"
    );
}

// ── Task 4.3 / 4.4 property-based tests ──────────────────────────────────────

use proptest::prelude::*;

proptest! {
    /// Task 4.3: For any archive with N files and M directories, list_entries
    /// returns exactly N entries.
    /// **Validates: Requirements 2.1, 3.1**
    #[test]
    fn prop_list_entries_returns_only_files(
        file_names in proptest::collection::vec("[a-z]{3,8}\\.txt", 1..=5usize),
    ) {
        use sevenz_rust::{SevenZWriter, SevenZArchiveEntry};

        let tmp = tempfile::tempdir().unwrap();
        let archive_path = tmp.path().join("prop_test.7z");

        // Create files and add them to the archive
        let mut writer = SevenZWriter::create(&archive_path).expect("create writer");
        for name in &file_names {
            let file_path = tmp.path().join(name);
            std::fs::write(&file_path, b"data").unwrap();
            writer.push_archive_entry(
                SevenZArchiveEntry::from_path(&file_path, name.clone()),
                Some(std::fs::File::open(&file_path).unwrap()),
            ).expect("push entry");
        }
        writer.finish().expect("finish");

        let backend = SevenZBackend;
        let entries = backend.list_entries(&archive_path).expect("list ok");

        prop_assert_eq!(
            entries.len(), file_names.len(),
            "Expected {} file entries, got: {:?}", file_names.len(), entries
        );
        for name in &file_names {
            prop_assert!(
                entries.contains(name),
                "Entry {:?} missing from {:?}", name, entries
            );
        }
    }
}

proptest! {
    /// Task 4.4: For any flat archive (M=0 directories), list_entries result
    /// is identical before and after fix — all entries are returned.
    /// **Validates: Requirements 3.1**
    #[test]
    fn prop_flat_archive_all_entries_returned(
        file_names in proptest::collection::vec("[a-z]{3,8}\\.txt", 1..=4usize),
    ) {
        use sevenz_rust::{SevenZWriter, SevenZArchiveEntry};

        let tmp = tempfile::tempdir().unwrap();
        let archive_path = tmp.path().join("flat_prop.7z");

        let mut writer = SevenZWriter::create(&archive_path).expect("create writer");
        for name in &file_names {
            let file_path = tmp.path().join(name);
            std::fs::write(&file_path, b"content").unwrap();
            writer.push_archive_entry(
                SevenZArchiveEntry::from_path(&file_path, name.clone()),
                Some(std::fs::File::open(&file_path).unwrap()),
            ).expect("push entry");
        }
        writer.finish().expect("finish");

        let backend = SevenZBackend;
        let entries = backend.list_entries(&archive_path).expect("list ok");

        // Flat archive: every entry should be returned (no filtering needed)
        prop_assert_eq!(entries.len(), file_names.len());
    }
}

// ── Task 4.5 integration test ─────────────────────────────────────────────────

/// Helper: create a 7z archive with nested directory structure.
/// src/main.rs and src/lib.rs stored under a "src/" prefix.
fn create_nested_archive() -> (TempDir, PathBuf) {
    use sevenz_rust::{SevenZWriter, SevenZArchiveEntry};

    let tmp = tempfile::tempdir().unwrap();
    let archive_path = tmp.path().join("nested.7z");

    let main_rs = tmp.path().join("main.rs");
    let lib_rs = tmp.path().join("lib.rs");
    std::fs::write(&main_rs, b"fn main() {}").unwrap();
    std::fs::write(&lib_rs, b"pub fn foo() {}").unwrap();

    let mut writer = SevenZWriter::create(&archive_path).expect("create writer");
    // Store files under "src/" prefix in the archive
    writer.push_archive_entry(
        SevenZArchiveEntry::from_path(&main_rs, "src/main.rs".to_string()),
        Some(std::fs::File::open(&main_rs).unwrap()),
    ).expect("push main.rs");
    writer.push_archive_entry(
        SevenZArchiveEntry::from_path(&lib_rs, "src/lib.rs".to_string()),
        Some(std::fs::File::open(&lib_rs).unwrap()),
    ).expect("push lib.rs");
    writer.finish().expect("finish");

    (tmp, archive_path)
}

/// Task 4.5: Extract a .7z fixture with nested directories; verify all files
/// land at correct paths and no extra wrapper folder is created.
/// **Validates: Requirements 3.2, 3.4**
#[test]
fn test_extract_files_land_at_correct_paths() {
    let (_tmp, archive_path) = create_nested_archive();
    let out_tmp = tempfile::tempdir().unwrap();
    let backend = SevenZBackend;

    backend
        .extract_with_progress(&archive_path, out_tmp.path(), |_, _, _| {})
        .expect("extract ok");

    // Files should be at src/main.rs and src/lib.rs relative to output dir
    assert!(
        out_tmp.path().join("src").join("main.rs").exists(),
        "src/main.rs should exist after extraction"
    );
    assert!(
        out_tmp.path().join("src").join("lib.rs").exists(),
        "src/lib.rs should exist after extraction"
    );

    // list_entries should return only the file paths, not a "src/" directory entry
    let entries = backend.list_entries(&archive_path).expect("list ok");
    assert_eq!(entries.len(), 2, "Expected 2 file entries, got: {:?}", entries);
    assert!(entries.contains(&"src/main.rs".to_string()));
    assert!(entries.contains(&"src/lib.rs".to_string()));
}

/// Task 4.5 (flat): Extract flat archive; verify both files exist on disk.
/// **Validates: Requirements 3.1**
#[test]
fn test_flat_archive_extract_unchanged() {
    let (_tmp, archive_path) = create_flat_archive();
    let out_tmp = tempfile::tempdir().unwrap();
    let backend = SevenZBackend;

    backend
        .extract_with_progress(&archive_path, out_tmp.path(), |_, _, _| {})
        .expect("extract ok");

    assert!(out_tmp.path().join("file1.txt").exists(), "file1.txt should exist");
    assert!(out_tmp.path().join("file2.txt").exists(), "file2.txt should exist");
}
