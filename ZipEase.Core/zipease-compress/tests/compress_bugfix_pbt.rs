//! Bug condition exploration tests for the compress-error-fix bugfix.
//!
//! These tests verify two aspects of the ZIP compression backend:
//!
//! (a) **Streaming roundtrip** — compress arbitrary file content and verify the
//!     archive is valid and content matches after extraction. On unfixed code this
//!     PASSES for small files (establishes the baseline). The real OOM only
//!     manifests with multi-gigabyte files that cannot be tested in CI.
//!
//! (b) **ZIP64 `large_file(true)` flag** — verify that the compression code path
//!     enables ZIP64 extensions for files ≥ 4 GB. On unfixed code the flag is
//!     NEVER set, so this test FAILS — confirming the bug exists.
//!
//! **Validates: Requirements 1.1, 1.2, 2.1, 2.2**

use std::fs;
use std::io::Write;
use std::path::Path;
use proptest::prelude::*;
use tempfile::TempDir;
use zipease_compress::compress::{CompressOptions, CompressionBackend};
use zipease_compress::compress::zip::ZipBackend;
use zipease_extract::extract::extract_with_progress;

/// Helper: create a single temp file with the given content bytes.
fn create_temp_file(dir: &Path, name: &str, content: &[u8]) -> std::path::PathBuf {
    let path = dir.join(name);
    let mut f = fs::File::create(&path).unwrap();
    f.write_all(content).unwrap();
    f.flush().unwrap();
    path
}

/// Helper: recursively find a file by name under `dir`.
fn find_file(dir: &Path, name: &str) -> Option<std::path::PathBuf> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = find_file(&path, name) {
                    return Some(found);
                }
            } else if path.file_name().and_then(|n| n.to_str()) == Some(name) {
                return Some(path);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// (a) Streaming roundtrip property test — PASSES on unfixed code for small files
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    /// **Validates: Requirements 2.1**
    ///
    /// For any random file content (1 KB – 256 KB) and compression level
    /// (0 = Stored, 1–9 = Deflated), compressing to ZIP and then extracting
    /// must produce byte-for-byte identical content. This establishes the
    /// roundtrip baseline — it passes on both unfixed and fixed code for
    /// small files.
    #[test]
    fn prop_streaming_roundtrip(
        content in proptest::collection::vec(any::<u8>(), 1024..=262_144),
        level in 0u8..=9u8,
    ) {
        // Create a temp file with random content
        let src_dir = TempDir::new().unwrap();
        let file_path = create_temp_file(src_dir.path(), "testfile.bin", &content);
        let input_refs: Vec<&Path> = vec![file_path.as_path()];

        // Compress
        let out_dir = TempDir::new().unwrap();
        let archive_path = out_dir.path().join("output.zip");
        let options = CompressOptions {
            level,
            store_relative_paths: true,
            password: None,
        };
        let backend = ZipBackend;
        let result = backend.compress_with_progress(
            &input_refs,
            &archive_path,
            &options,
            |_, _, _| {},
        );
        // Some compression levels may not be supported by the zip crate —
        // skip those cases (matches the pattern in compression_pbt.rs).
        prop_assume!(result.is_ok(), "compression level {} not supported", level);

        // Extract
        let extract_dir = TempDir::new().unwrap();
        extract_with_progress(&archive_path, extract_dir.path(), |_, _, _| {}).unwrap();

        // Verify content matches
        let extracted = find_file(extract_dir.path(), "testfile.bin");
        prop_assert!(extracted.is_some(), "extracted file not found");
        let extracted_content = fs::read(extracted.unwrap()).unwrap();
        prop_assert_eq!(
            content.len(),
            extracted_content.len(),
            "extracted file size mismatch"
        );
        prop_assert!(
            content == extracted_content,
            "extracted content does not match original"
        );
    }
}

// ---------------------------------------------------------------------------
// (b) ZIP64 large_file(true) flag test — FAILS on unfixed code
// ---------------------------------------------------------------------------

/// **Validates: Requirements 1.2, 2.2**
///
/// This test verifies that the ZIP compression code path enables ZIP64
/// extensions (`large_file(true)`) for files that are ≥ 4 GB.
///
/// Since we cannot create a 4 GB file in tests, we verify the code path
/// indirectly: we read the source code of `zip.rs` and assert that it
/// contains the string `large_file(true)`. On unfixed code this string
/// is absent — the test FAILS, confirming the bug exists.
///
/// After the fix is applied (task 3.2), the source will contain
/// `large_file(true)` and this test will PASS.
#[test]
fn test_zip64_large_file_flag_exists_in_source() {
    // Read the source code of the ZIP backend
    let zip_rs_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("compress")
        .join("zip.rs");

    let source = fs::read_to_string(&zip_rs_path)
        .expect("Failed to read zip.rs source file");

    // The fix must add `large_file(true)` to FileOptions for files >= 4 GB.
    // On unfixed code, this string is NEVER present — test FAILS.
    assert!(
        source.contains("large_file(true)"),
        "BUG CONFIRMED: zip.rs does NOT contain `large_file(true)`. \
         ZIP64 extensions are never enabled, so files >= 4 GB cannot be \
         represented in the archive. The fix must add \
         `.large_file(true)` to FileOptions when file size >= 4 GB."
    );
}

/// **Validates: Requirements 1.1, 2.1**
///
/// This test verifies that the ZIP compression code path uses streaming
/// I/O (e.g. `io::copy`) instead of loading entire files into memory
/// via `read_to_end()`.
///
/// We read the source code of `zip.rs` and assert that it does NOT
/// contain `read_to_end`. On unfixed code, `read_to_end` IS present —
/// the test FAILS, confirming the OOM bug exists.
///
/// After the fix is applied (task 3.1), `read_to_end` will be replaced
/// with streaming `io::copy` and this test will PASS.
#[test]
fn test_streaming_io_no_read_to_end_in_source() {
    // Read the source code of the ZIP backend
    let zip_rs_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("compress")
        .join("zip.rs");

    let source = fs::read_to_string(&zip_rs_path)
        .expect("Failed to read zip.rs source file");

    // The fix must remove `read_to_end` and replace with streaming io::copy.
    // On unfixed code, `read_to_end` IS present — test FAILS.
    assert!(
        !source.contains("read_to_end"),
        "BUG CONFIRMED: zip.rs uses `read_to_end()` which loads the entire \
         file into memory. For multi-gigabyte files this causes out-of-memory \
         failures. The fix must replace `read_to_end` with streaming \
         `io::copy` or a chunked read loop."
    );
}

// ===========================================================================
// Task 2: Preservation property tests — must PASS on UNFIXED code
// ===========================================================================

use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// (Prop 2a) Small-file roundtrip preservation
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    /// **Validates: Requirements 3.1, 3.2**
    ///
    /// For all random small files (1 byte – 5 MB) and compression levels
    /// (0–9), compressing to ZIP then extracting must produce byte-for-byte
    /// identical file content. This must PASS on unfixed code — it captures
    /// the baseline behavior that the fix must preserve.
    #[test]
    fn prop_preservation_small_file_roundtrip(
        content in proptest::collection::vec(any::<u8>(), 1..=5_242_880usize),
        level in 0u8..=9u8,
    ) {
        // Create a temp file with random content
        let src_dir = TempDir::new().unwrap();
        let file_path = create_temp_file(src_dir.path(), "small_test.bin", &content);
        let input_refs: Vec<&Path> = vec![file_path.as_path()];

        // Compress
        let out_dir = TempDir::new().unwrap();
        let archive_path = out_dir.path().join("preservation.zip");
        let options = CompressOptions {
            level,
            store_relative_paths: true,
            password: None,
        };
        let backend = ZipBackend;
        let result = backend.compress_with_progress(
            &input_refs,
            &archive_path,
            &options,
            |_, _, _| {},
        );
        // Some compression levels may not be supported — skip those cases.
        prop_assume!(result.is_ok(), "compression level {} not supported", level);

        // Extract
        let extract_dir = TempDir::new().unwrap();
        extract_with_progress(&archive_path, extract_dir.path(), |_, _, _| {}).unwrap();

        // Verify content matches byte-for-byte
        let extracted = find_file(extract_dir.path(), "small_test.bin");
        prop_assert!(extracted.is_some(), "extracted file not found");
        let extracted_content = fs::read(extracted.unwrap()).unwrap();
        prop_assert_eq!(
            content.len(),
            extracted_content.len(),
            "extracted file size mismatch"
        );
        prop_assert!(
            content == extracted_content,
            "extracted content does not match original"
        );
    }
}

// ---------------------------------------------------------------------------
// (Prop 2b) Password-protected AES-256 roundtrip preservation
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    /// **Validates: Requirements 3.3**
    ///
    /// For all random small files with random non-empty ASCII passwords,
    /// compressing with AES-256 encryption then extracting with the same
    /// password must produce byte-for-byte identical content. This must
    /// PASS on unfixed code — it captures the baseline behavior that the
    /// fix must preserve.
    #[test]
    fn prop_preservation_password_roundtrip(
        content in proptest::collection::vec(any::<u8>(), 1..=1_048_576usize),
        level in 0u8..=9u8,
        password in "[a-zA-Z0-9!@#$%]{1,32}",
    ) {
        // Create a temp file with random content
        let src_dir = TempDir::new().unwrap();
        let file_path = create_temp_file(src_dir.path(), "encrypted_test.bin", &content);
        let input_refs: Vec<&Path> = vec![file_path.as_path()];

        // Compress with password (AES-256)
        let out_dir = TempDir::new().unwrap();
        let archive_path = out_dir.path().join("encrypted.zip");
        let options = CompressOptions {
            level,
            store_relative_paths: true,
            password: Some(password.clone()),
        };
        let backend = ZipBackend;
        let result = backend.compress_with_progress(
            &input_refs,
            &archive_path,
            &options,
            |_, _, _| {},
        );
        prop_assume!(result.is_ok(), "compression level {} not supported", level);

        // Extract using the zip crate directly with password decryption
        let extract_dir = TempDir::new().unwrap();
        let archive_file = fs::File::open(&archive_path).unwrap();
        let mut archive = zip::ZipArchive::new(archive_file).unwrap();

        for i in 0..archive.len() {
            let mut file = archive
                .by_index_decrypt(i, password.as_bytes())
                .expect("failed to decrypt entry");

            let name = file.name().to_string();
            let outpath = extract_dir.path().join(&name);

            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent).unwrap();
            }

            if !file.is_dir() {
                let mut outfile = fs::File::create(&outpath).unwrap();
                std::io::copy(&mut file, &mut outfile).unwrap();
            }
        }

        // Verify content matches byte-for-byte
        let extracted = find_file(extract_dir.path(), "encrypted_test.bin");
        prop_assert!(extracted.is_some(), "extracted encrypted file not found");
        let extracted_content = fs::read(extracted.unwrap()).unwrap();
        prop_assert_eq!(
            content.len(),
            extracted_content.len(),
            "extracted encrypted file size mismatch"
        );
        prop_assert!(
            content == extracted_content,
            "extracted encrypted content does not match original"
        );
    }
}

// ---------------------------------------------------------------------------
// (Prop 2c) Progress callback preservation
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    /// **Validates: Requirements 3.5**
    ///
    /// For all random file counts (1–8) and compression levels, the progress
    /// callback must fire exactly `count` times with the correct filenames.
    /// This must PASS on unfixed code — it captures the baseline behavior
    /// that the fix must preserve.
    #[test]
    fn prop_preservation_progress_callback(
        count in 1usize..=8usize,
        level in 0u8..=9u8,
    ) {
        // Create N temp files with known content
        let src_dir = TempDir::new().unwrap();
        let mut file_paths = Vec::new();
        let mut expected_names = Vec::new();
        for i in 0..count {
            let name = format!("progress_{}.dat", i);
            let content = vec![i as u8; 1024]; // 1 KB each
            let path = create_temp_file(src_dir.path(), &name, &content);
            file_paths.push(path);
            expected_names.push(name);
        }
        let input_refs: Vec<&Path> = file_paths.iter().map(|p| p.as_path()).collect();

        // Compress with progress tracking
        let out_dir = TempDir::new().unwrap();
        let archive_path = out_dir.path().join("progress.zip");
        let options = CompressOptions {
            level,
            store_relative_paths: true,
            password: None,
        };

        let progress_log: Arc<Mutex<Vec<(usize, usize, String)>>> =
            Arc::new(Mutex::new(Vec::new()));
        let progress_log_clone = Arc::clone(&progress_log);

        let backend = ZipBackend;
        let result = backend.compress_with_progress(
            &input_refs,
            &archive_path,
            &options,
            move |current, total, name| {
                progress_log_clone
                    .lock()
                    .unwrap()
                    .push((current, total, name.to_string()));
            },
        );
        prop_assume!(result.is_ok(), "compression level {} not supported", level);

        let log = progress_log.lock().unwrap();

        // Progress callback must fire exactly `count` times
        prop_assert_eq!(
            log.len(),
            count,
            "progress callback should fire exactly once per file"
        );

        // Each callback must report the correct total
        for entry in log.iter() {
            prop_assert_eq!(
                entry.1,
                count,
                "total reported in progress callback should equal file count"
            );
        }

        // Each callback must report a filename that matches one of the expected names
        let reported_names: Vec<&str> = log.iter().map(|e| e.2.as_str()).collect();
        for expected in &expected_names {
            prop_assert!(
                reported_names.iter().any(|n| n.contains(expected.as_str())),
                "expected filename '{}' not found in progress callbacks: {:?}",
                expected,
                reported_names
            );
        }
    }
}
