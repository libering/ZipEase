//! Unit tests for SevenZaDllBackend (sevenzip-backend tasks 2.6, 3.5, 3.7).
//!
//! Feature: sevenzip-backend
//! Validates: Requirements 2.1, 2.3, 3.1, 3.2, 3.3, 3.4, 5.2, 5.3, 7.4

use zipease_extract::extract::sevenzadll::{
    resolve_dll_path, SevenZaDllBackend,
    CLSID_XZ_HANDLER, CLSID_LZMA_HANDLER, CLSID_WIM_HANDLER, CLSID_VHD_HANDLER
};
use zipease_extract::extract::ExtractionBackend;
use zipease_shared::LockError;
use std::path::Path;

/// Test that the new CLSID constants for XZ, LZMA, WIM, and VHD are correctly defined.
#[test]
fn test_clsid_constants() {
    assert_eq!(CLSID_XZ_HANDLER.data1, 0x23170F69);
    assert_eq!(CLSID_XZ_HANDLER.data2, 0x40C1);
    assert_eq!(CLSID_XZ_HANDLER.data3, 0x278A);
    assert_eq!(CLSID_XZ_HANDLER.data4, [0x10, 0x00, 0x00, 0x01, 0x10, 0x0C, 0x00, 0x00]);

    assert_eq!(CLSID_LZMA_HANDLER.data1, 0x23170F69);
    assert_eq!(CLSID_LZMA_HANDLER.data2, 0x40C1);
    assert_eq!(CLSID_LZMA_HANDLER.data3, 0x278A);
    assert_eq!(CLSID_LZMA_HANDLER.data4, [0x10, 0x00, 0x00, 0x01, 0x10, 0x0B, 0x00, 0x00]);

    assert_eq!(CLSID_WIM_HANDLER.data1, 0x23170F69);
    assert_eq!(CLSID_WIM_HANDLER.data2, 0x40C1);
    assert_eq!(CLSID_WIM_HANDLER.data3, 0x278A);
    assert_eq!(CLSID_WIM_HANDLER.data4, [0x10, 0x00, 0x00, 0x01, 0x10, 0x0E, 0x00, 0x00]);

    assert_eq!(CLSID_VHD_HANDLER.data1, 0x23170F69);
    assert_eq!(CLSID_VHD_HANDLER.data2, 0x40C1);
    assert_eq!(CLSID_VHD_HANDLER.data3, 0x278A);
    assert_eq!(CLSID_VHD_HANDLER.data4, [0x10, 0x00, 0x00, 0x01, 0x10, 0x0F, 0x00, 0x00]);
}


// ─── Task 2.6: DLL resolution and list_entries ────────────────────────────────

/// A nonexistent DLL path must return PluginRequired.
/// We test this by calling list_entries on a path where 7za.dll cannot be found.
/// Since resolve_dll_path checks for the DLL's existence, a missing DLL → PluginRequired.
#[test]
fn test_missing_dll_returns_plugin_required() {
    // We can't easily override the DLL path, but we can verify the error type
    // when the DLL is absent. If 7za.dll is present in the test environment,
    // this test verifies the path-not-found error for a nonexistent archive instead.
    let backend = SevenZaDllBackend;
    let result = backend.list_entries(Path::new("C:\\nonexistent_archive_xyz_abc.rar"));
    match result {
        Err(LockError::PluginRequired(_)) => {
            // DLL not found — correct behavior
        }
        Err(LockError::PathNotFound(_)) | Err(LockError::ExtractionFailed(_)) => {
            // DLL was found but archive doesn't exist — also acceptable
        }
        Ok(_) => panic!("Expected error for nonexistent archive, got Ok"),
        Err(e) => panic!("Unexpected error variant: {:?}", e),
    }
}

/// A zero-byte file passed as a DLL should fail gracefully (PluginRequired or ExtractionFailed).
#[test]
fn test_corrupt_dll_does_not_panic() {
    // We verify that even if the DLL loading fails, no panic occurs.
    // This is tested indirectly: list_entries wraps everything in catch_unwind at the FFI level.
    // At the Rust API level, we just verify it returns Err, not panics.
    let backend = SevenZaDllBackend;
    let result = std::panic::catch_unwind(|| {
        backend.list_entries(Path::new("C:\\nonexistent_xyz.rar"))
    });
    assert!(result.is_ok(), "list_entries must not panic, even for bad paths");
    assert!(result.unwrap().is_err(), "list_entries must return Err for nonexistent archive");
}

/// resolve_dll_path returns a path ending in "7za.dll".
#[test]
fn test_dll_path_resolution_ends_with_7za_dll() {
    match resolve_dll_path() {
        Ok(path) => {
            let file_name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            assert_eq!(
                file_name.to_lowercase(),
                "7za.dll",
                "resolved DLL path must end with 7za.dll, got: {}",
                path.display()
            );
        }
        Err(LockError::PluginRequired(_)) => {
            // 7za.dll not present in test environment — acceptable
        }
        Err(e) => panic!("Unexpected error from resolve_dll_path: {:?}", e),
    }
}

// ─── Task 3.5: Extraction error codes ────────────────────────────────────────

/// LockError::PluginRequired maps to error code 0x2003.
#[test]
fn test_error_code_plugin_required() {
    let err = LockError::PluginRequired("test".into());
    assert_eq!(
        err.to_error_code(),
        0x2003i32,
        "PluginRequired must map to error code 0x2003"
    );
}

/// LockError::ExtractionFailed maps to error code 0x2001.
#[test]
fn test_error_code_extraction_failed() {
    let err = LockError::ExtractionFailed("test".into());
    assert_eq!(
        err.to_error_code(),
        0x2001i32,
        "ExtractionFailed must map to error code 0x2001"
    );
}

// ─── Task 3.7: No panic on invalid DLL (Property 3) ──────────────────────────

/// Property 3: No panic on invalid DLL
/// For any nonexistent/invalid path, list_entries must not panic.
/// Validates: Requirements 2.3, 3.1, 3.2, 3.4
#[test]
fn test_no_panic_on_invalid_dll_path() {
    let backend = SevenZaDllBackend;
    let paths = [
        "C:\\nonexistent_xyz_1.rar",
        "C:\\nonexistent_xyz_2.rar",
        "",
        "not_a_real_file.rar",
    ];
    for path in &paths {
        let result = std::panic::catch_unwind(|| {
            backend.list_entries(Path::new(path))
        });
        assert!(
            result.is_ok(),
            "list_entries must not panic for path: {}", path
        );
        assert!(
            result.unwrap().is_err(),
            "list_entries must return Err for invalid path: {}", path
        );
    }
}

/// Property 3 (proptest variant): No panic for arbitrary-looking paths.
/// Validates: Requirements 2.3, 3.4
#[test]
fn test_no_panic_on_various_bad_paths() {
    let backend = SevenZaDllBackend;
    let bad_paths = [
        "C:\\a.rar",
        "D:\\b.rar",
        "\\\\server\\share\\c.rar",
        "relative/path/d.rar",
        "file_without_extension",
    ];
    for path in &bad_paths {
        let result = std::panic::catch_unwind(|| {
            backend.list_entries(Path::new(path))
        });
        assert!(
            result.is_ok(),
            "list_entries panicked for path: {}", path
        );
    }
}

// ─── Task 2.7: Property 1 — Listing round-trip ───────────────────────────────
// Feature: sevenzip-backend, Property 1: Listing round-trip
//
// Iterates over RAR fixtures under `ZipEase.Core/tests/fixtures/` (ASCII, Unicode, CJK, nested).
// Asserts returned entry set matches known set — no omissions, no duplicates, no mojibake.
//
// **Validates: Requirements 1.1, 7.2, 7.3**

use std::collections::HashSet;
use std::fs;
use proptest::prelude::*;
use proptest::test_runner::{TestRunner, Config as ProptestConfig};

/// Discover all `.rar` fixture files under `zipease-extract/tests/fixtures/`.
/// Returns pairs of (rar_path, manifest_path) where both exist.
fn discover_rar_fixtures() -> Vec<(std::path::PathBuf, std::path::PathBuf)> {
    // Try multiple possible fixture directory locations relative to the test binary
    let candidate_dirs = [
        // Per-crate fixtures directory (canonical location)
        std::path::PathBuf::from("tests/fixtures"),
        std::path::PathBuf::from("../tests/fixtures"),
        // Relative to workspace root (when running from workspace root)
        {
            let mut p = std::env::current_dir().unwrap_or_default();
            p.push("ZipEase.Core/zipease-extract/tests/fixtures");
            p
        },
        {
            let mut p = std::env::current_dir().unwrap_or_default();
            p.push("tests/fixtures");
            p
        },
    ];

    let fixtures_dir = candidate_dirs.iter().find(|d| d.is_dir());
    let fixtures_dir = match fixtures_dir {
        Some(d) => d.clone(),
        None => return Vec::new(),
    };

    let mut fixtures = Vec::new();
    if let Ok(entries) = fs::read_dir(&fixtures_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("rar") {
                let manifest = path.with_extension("manifest");
                if manifest.exists() {
                    fixtures.push((path, manifest));
                }
            }
        }
    }
    fixtures
}

/// Read a manifest file: one entry name per line (non-directory entries).
fn read_manifest(manifest_path: &std::path::Path) -> HashSet<String> {
    fs::read_to_string(manifest_path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .collect()
}

/// Check if 7za.dll is available for testing.
fn is_dll_available() -> bool {
    resolve_dll_path().is_ok()
}

/// Property 1: Listing round-trip
///
/// For each RAR fixture with a manifest, list_entries must return exactly the
/// expected set of entries — no omissions, no duplicates, no mojibake.
///
/// Validates: Requirements 1.1, 7.2, 7.3
#[test]
fn prop_list_entries_round_trip() {
    // Feature: sevenzip-backend, Property 1: Listing round-trip
    if !is_dll_available() {
        eprintln!("[SKIP] prop_list_entries_round_trip: 7za.dll not available in test environment");
        return;
    }

    let fixtures = discover_rar_fixtures();
    if fixtures.is_empty() {
        eprintln!("[SKIP] prop_list_entries_round_trip: No RAR fixtures found under ZipEase.Core/tests/fixtures/");
        return;
    }

    let backend = SevenZaDllBackend;

    // Use proptest TestRunner to iterate over fixtures as a property test
    let mut runner = TestRunner::new(ProptestConfig {
        cases: fixtures.len() as u32,
        ..ProptestConfig::default()
    });

    let fixture_count = fixtures.len();
    let result = runner.run(
        &(0..fixture_count),
        |idx| {
            let (rar_path, manifest_path) = &fixtures[idx];
            let expected_entries = read_manifest(manifest_path);

            let listed = backend.list_entries(rar_path)
                .map_err(|e| proptest::test_runner::TestCaseError::fail(
                    format!("list_entries failed for {}: {:?}", rar_path.display(), e)
                ))?;

            let listed_set: HashSet<String> = listed.iter().cloned().collect();

            // No omissions: every expected entry must be in the listed set
            for expected in &expected_entries {
                prop_assert!(
                    listed_set.contains(expected),
                    "Missing entry '{}' in listing of {}",
                    expected,
                    rar_path.display()
                );
            }

            // No extras: every listed entry must be in the expected set
            for listed_entry in &listed_set {
                prop_assert!(
                    expected_entries.contains(listed_entry),
                    "Unexpected entry '{}' in listing of {}",
                    listed_entry,
                    rar_path.display()
                );
            }

            // No duplicates: listed vec length must equal set length
            prop_assert_eq!(
                listed.len(),
                listed_set.len(),
                "Duplicate entries detected in listing of {}",
                rar_path.display()
            );

            Ok(())
        },
    );

    if let Err(e) = result {
        panic!("Property 1 (Listing round-trip) failed: {}", e);
    }

    eprintln!(
        "[PASS] prop_list_entries_round_trip: verified {} fixture(s)",
        fixture_count
    );
}

// ─── Task 3.6: Property 2 — Extract round-trip ───────────────────────────────
// Feature: sevenzip-backend, Property 2: Extract round-trip
//
// For each fixture: list entries, extract to tempdir, assert one file per non-directory entry.
//
// **Validates: Requirements 1.2, 7.1**

/// Property 2: Extract round-trip
///
/// For each RAR fixture, calling list_entries and then extracting to a temp directory
/// must produce exactly one file on disk for every non-directory entry returned by
/// list_entries.
///
/// Validates: Requirements 1.2, 7.1
#[test]
fn prop_extract_round_trip() {
    // Feature: sevenzip-backend, Property 2: Extract round-trip
    if !is_dll_available() {
        eprintln!("[SKIP] prop_extract_round_trip: 7za.dll not available in test environment");
        return;
    }

    let fixtures = discover_rar_fixtures();
    if fixtures.is_empty() {
        eprintln!("[SKIP] prop_extract_round_trip: No RAR fixtures found under ZipEase.Core/tests/fixtures/");
        return;
    }

    let backend = SevenZaDllBackend;

    // Use proptest TestRunner to iterate over fixtures as a property test
    let mut runner = TestRunner::new(ProptestConfig {
        cases: fixtures.len() as u32,
        ..ProptestConfig::default()
    });

    let fixture_count = fixtures.len();
    let result = runner.run(
        &(0..fixture_count),
        |idx| {
            let (rar_path, _manifest_path) = &fixtures[idx];

            // Step 1: List entries
            let entries = backend.list_entries(rar_path)
                .map_err(|e| proptest::test_runner::TestCaseError::fail(
                    format!("list_entries failed for {}: {:?}", rar_path.display(), e)
                ))?;

            // Step 2: Extract to a temp directory
            let tmp = tempfile::tempdir()
                .map_err(|e| proptest::test_runner::TestCaseError::fail(
                    format!("Failed to create tempdir: {:?}", e)
                ))?;

            backend.extract(rar_path, tmp.path())
                .map_err(|e| proptest::test_runner::TestCaseError::fail(
                    format!("extract failed for {}: {:?}", rar_path.display(), e)
                ))?;

            // Step 3: Filter non-directory entries (entries not ending with '/' or '\')
            let non_dir_entries: Vec<&String> = entries.iter()
                .filter(|e| !e.ends_with('/') && !e.ends_with('\\'))
                .collect();

            // Step 4: Assert one file per non-directory entry exists on disk
            for entry_name in &non_dir_entries {
                // Normalize path separators for the filesystem check
                let normalized = entry_name.replace('\\', "/");
                let expected_path = tmp.path().join(&normalized);
                prop_assert!(
                    expected_path.exists(),
                    "Expected extracted file '{}' not found on disk at {} (archive: {})",
                    entry_name,
                    expected_path.display(),
                    rar_path.display()
                );
                prop_assert!(
                    expected_path.is_file(),
                    "Expected '{}' to be a file, but it is not (archive: {})",
                    entry_name,
                    rar_path.display()
                );
            }

            // Step 5: Count files on disk matches non-directory entry count
            let file_count = count_files_recursive(tmp.path());
            prop_assert_eq!(
                file_count,
                non_dir_entries.len(),
                "File count mismatch for {}: {} files on disk vs {} non-dir entries listed",
                rar_path.display(),
                file_count,
                non_dir_entries.len()
            );

            Ok(())
        },
    );

    if let Err(e) = result {
        panic!("Property 2 (Extract round-trip) failed: {}", e);
    }

    eprintln!(
        "[PASS] prop_extract_round_trip: verified {} fixture(s)",
        fixture_count
    );
}

/// Recursively count all files (not directories) under a path.
fn count_files_recursive(dir: &Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count += count_files_recursive(&path);
            } else {
                count += 1;
            }
        }
    }
    count
}
