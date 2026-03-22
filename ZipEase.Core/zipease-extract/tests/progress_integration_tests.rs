//! Integration tests for progress reporting (ui-integration task 2.7).
//!
//! Feature: ui-integration, task 2.7
//! Validates: Requirements 5.2 — progress callback invoked for each file,
//!            percentages increase monotonically, all filenames reported.

use std::fs;
use std::io::Write;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use zipease_extract::extract::extract_with_progress;

/// Build a ZIP archive in memory containing `file_names` entries, each with `content`.
fn make_zip(dir: &std::path::Path, file_names: &[&str]) -> std::path::PathBuf {
    let zip_path = dir.join("test.zip");
    let file = fs::File::create(&zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);
    for name in file_names {
        zip.start_file(*name, options).unwrap();
        zip.write_all(format!("content of {}", name).as_bytes()).unwrap();
    }
    zip.finish().unwrap();
    zip_path
}

#[test]
fn test_progress_callback_invoked_for_each_file() {
    let src = TempDir::new().unwrap();
    let names = ["alpha.txt", "beta.txt", "gamma.txt"];
    let zip_path = make_zip(src.path(), &names);

    let out = TempDir::new().unwrap();
    let calls: Arc<Mutex<Vec<(usize, usize, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let calls_clone = Arc::clone(&calls);

    extract_with_progress(&zip_path, out.path(), |current, total, name| {
        calls_clone.lock().unwrap().push((current, total, name.to_string()));
    }).unwrap();

    let recorded = calls.lock().unwrap();
    // Callback must fire exactly once per file
    assert_eq!(recorded.len(), names.len(), "callback must fire once per file");
    // All filenames must be reported
    let reported_names: Vec<&str> = recorded.iter().map(|(_, _, n)| n.as_str()).collect();
    for name in &names {
        assert!(
            reported_names.contains(name),
            "filename '{}' not reported in progress callbacks", name
        );
    }
}

#[test]
fn test_progress_percentage_increases_monotonically() {
    let src = TempDir::new().unwrap();
    let names = ["a.txt", "b.txt", "c.txt", "d.txt", "e.txt"];
    let zip_path = make_zip(src.path(), &names);

    let out = TempDir::new().unwrap();
    let percentages: Arc<Mutex<Vec<i32>>> = Arc::new(Mutex::new(Vec::new()));
    let pct_clone = Arc::clone(&percentages);

    // We compute percentage the same way the FFI layer does: (current / total) * 100
    extract_with_progress(&zip_path, out.path(), |current, total, _name| {
        let pct = if total > 0 { ((current as f64 / total as f64) * 100.0) as i32 } else { 0 };
        pct_clone.lock().unwrap().push(pct);
    }).unwrap();

    let pcts = percentages.lock().unwrap();
    assert!(!pcts.is_empty(), "at least one progress call expected");
    // Percentages must be non-decreasing
    for window in pcts.windows(2) {
        assert!(
            window[1] >= window[0],
            "percentages must be non-decreasing: {:?}", &*pcts
        );
    }
    // Last percentage must be 100
    assert_eq!(*pcts.last().unwrap(), 100, "final percentage must be 100");
}

#[test]
fn test_progress_total_matches_file_count() {
    let src = TempDir::new().unwrap();
    let names = ["x.txt", "y.txt"];
    let zip_path = make_zip(src.path(), &names);

    let out = TempDir::new().unwrap();
    let totals: Arc<Mutex<Vec<usize>>> = Arc::new(Mutex::new(Vec::new()));
    let totals_clone = Arc::clone(&totals);

    extract_with_progress(&zip_path, out.path(), |_current, total, _name| {
        totals_clone.lock().unwrap().push(total);
    }).unwrap();

    let recorded = totals.lock().unwrap();
    for &t in recorded.iter() {
        assert_eq!(t, names.len(), "total reported in callback must equal file count");
    }
}
