//! Property-based tests for compression backends.
//!
//! Feature: archive-compression
//! Property 1: progress callback count equals input_count
//! Property 2: round-trip compress then extract produces identical file tree
//! Property 3: non-empty output archive on success

use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use proptest::prelude::*;
use zipease_compress::compress::{CompressOptions, compress_with_progress};
use zipease_extract::extract::extract_with_progress;

/// Create N temp files with known content in a temp dir, return (TempDir, Vec<PathBuf>)
fn make_temp_files(dir: &Path, count: usize) -> Vec<std::path::PathBuf> {
    (0..count).map(|i| {
        let path = dir.join(format!("file_{}.txt", i));
        let mut f = fs::File::create(&path).unwrap();
        writeln!(f, "content of file {}", i).unwrap();
        path
    }).collect()
}

proptest! {
    // Property 1: progress callback fires exactly input_count times
    // Validates: archive-compression FFI postcondition — progress callback count
    #[test]
    fn prop_progress_callback_count(count in 1usize..=10usize, level in 0u8..=9u8) {
        let dir = TempDir::new().unwrap();
        let files = make_temp_files(dir.path(), count);
        let input_refs: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();

        let out_dir = TempDir::new().unwrap();
        let output = out_dir.path().join("out.zip");

        let call_count = Arc::new(Mutex::new(0usize));
        let call_count_clone = Arc::clone(&call_count);

        let options = CompressOptions { level, store_relative_paths: true };
        let result = compress_with_progress(
            &input_refs,
            &output,
            &options,
            |_pct, _total, _name| {
                *call_count_clone.lock().unwrap() += 1;
            },
        );

        prop_assume!(result.is_ok());
        let calls = *call_count.lock().unwrap();
        prop_assert_eq!(calls, count, "callback should fire exactly once per file");
    }

    // Property 3: output archive size > 0 on success
    // Validates: archive-compression CompressionBackend postcondition — archive exists
    #[test]
    fn prop_nonempty_output(count in 1usize..=5usize, level in 0u8..=9u8) {
        let dir = TempDir::new().unwrap();
        let files = make_temp_files(dir.path(), count);
        let input_refs: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();

        let out_dir = TempDir::new().unwrap();
        let output = out_dir.path().join("out.zip");

        let options = CompressOptions { level, store_relative_paths: true };
        let result = compress_with_progress(&input_refs, &output, &options, |_, _, _| {});

        prop_assume!(result.is_ok());
        let size = fs::metadata(&output).unwrap().len();
        prop_assert!(size > 0, "output archive must be non-empty");
    }
}

// Property 2: round-trip compress then extract produces identical file tree
// This is a regular test (not proptest) since it uses fixed inputs for determinism
#[test]
fn test_zip_round_trip() {
    let src_dir = TempDir::new().unwrap();
    let files = make_temp_files(src_dir.path(), 3);
    let input_refs: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();

    let out_dir = TempDir::new().unwrap();
    let output = out_dir.path().join("out.zip");

    let options = CompressOptions { level: 6, store_relative_paths: true };
    compress_with_progress(&input_refs, &output, &options, |_, _, _| {}).unwrap();

    let extract_dir = TempDir::new().unwrap();
    extract_with_progress(&output, extract_dir.path(), |_, _, _| {}).unwrap();

    // Verify all original files are present with same content
    for (i, original) in files.iter().enumerate() {
        let name = format!("file_{}.txt", i);
        // Walk extract_dir to find the file
        let extracted = find_file(extract_dir.path(), &name);
        assert!(extracted.is_some(), "file {} not found in extracted output", name);
        let orig_content = fs::read_to_string(original).unwrap();
        let ext_content = fs::read_to_string(extracted.unwrap()).unwrap();
        assert_eq!(orig_content, ext_content, "content mismatch for {}", name);
    }
}

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
