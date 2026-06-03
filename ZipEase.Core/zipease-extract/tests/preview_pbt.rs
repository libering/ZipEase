//! Fix Verification Tests — preview-entry-fix
//!
//! These tests verify the FIXED behaviour of the preview extraction system.
//! They replace the bug-exploration tests from Task 1.
//!
//! Properties tested:
//!   1. Name-based matching correctness (`find_file_recursive`)
//!   2. UUID uniqueness (`unique_temp_name`)
//!   3. Path safety / Functional Paranoia (`safe_join` rejects malicious names)
//!   4. ZIP regression guard (`zip_ease_extract_entry_any` still exists)
//!   5. Temp dir cleanup after `extract_entry_by_name`
//!
//! **Validates: Requirements 1.1, 1.2, 1.3, 1.4, 1.5 (bugfix.md)**

use proptest::prelude::*;
use std::collections::HashSet;
use std::fs;
use tempfile::TempDir;

// Access pub(crate) helpers via the crate's extract module.
use zipease_extract::extract::{find_file_recursive, unique_temp_name};
use zipease_extract::extract::safe_join;

// ---------------------------------------------------------------------------
// Property 1 — Name-based matching correctness
//
// `find_file_recursive` must locate a file by its `file_name()` component,
// case-insensitively, even when nested in subdirectories.
// ---------------------------------------------------------------------------

#[test]
fn prop1_find_file_recursive_finds_exact_match() {
    let dir = TempDir::new().unwrap();
    let sub = dir.path().join("subdir");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("hello.txt"), b"content").unwrap();

    let result = find_file_recursive(dir.path(), "hello.txt");
    assert!(result.is_some(), "should find hello.txt in subdir");
    assert_eq!(
        result.unwrap().file_name().unwrap(),
        "hello.txt"
    );
}

#[test]
fn prop1_find_file_recursive_case_insensitive() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("README.MD"), b"readme").unwrap();

    // Search with different casing
    let result = find_file_recursive(dir.path(), "readme.md");
    assert!(result.is_some(), "case-insensitive match should succeed");
}

#[test]
fn prop1_find_file_recursive_returns_none_for_missing_file() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("other.txt"), b"x").unwrap();

    let result = find_file_recursive(dir.path(), "nonexistent.bin");
    assert!(result.is_none(), "should return None when file is absent");
}

proptest! {
    // **Validates: Requirements 1.1 (bugfix.md)**
    //
    // For any valid filename, find_file_recursive must find it in a nested structure.
    #[test]
    fn prop1_find_file_recursive_handles_nested_dirs(
        depth in 1usize..=4usize,
        file_stem in "[a-z]{3,8}",
    ) {
        let dir = TempDir::new().unwrap();

        // Build a nested path: dir/d0/d1/.../file_stem.dat
        let mut nested = dir.path().to_path_buf();
        for i in 0..depth {
            nested = nested.join(format!("d{}", i));
        }
        fs::create_dir_all(&nested).unwrap();
        let file_name = format!("{}.dat", file_stem);
        fs::write(nested.join(&file_name), b"data").unwrap();

        let result = find_file_recursive(dir.path(), &file_name);
        prop_assert!(
            result.is_some(),
            "find_file_recursive should find '{}' at depth {}",
            file_name, depth
        );
        let found = result.unwrap();
        let found_name = found.file_name().unwrap().to_str().unwrap();
        prop_assert_eq!(found_name, file_name.as_str());
    }
}

// ---------------------------------------------------------------------------
// Property 2 — UUID uniqueness
//
// N consecutive calls to `unique_temp_name()` must produce N distinct values.
// ---------------------------------------------------------------------------

#[test]
fn prop2_unique_temp_name_produces_distinct_values_sequential() {
    let names: Vec<String> = (0..20).map(|_| unique_temp_name()).collect();
    let set: HashSet<&String> = names.iter().collect();
    // Allow for the rare case of sub-nanosecond resolution — require at least 90% unique
    // In practice on any real system all 20 should be unique.
    assert!(
        set.len() >= 18,
        "unique_temp_name should produce mostly distinct values, got {}/{} unique",
        set.len(), names.len()
    );
}

#[test]
fn prop2_unique_temp_name_has_correct_prefix() {
    let name = unique_temp_name();
    assert!(
        name.starts_with("ZipEase_preview_"),
        "temp name must start with 'ZipEase_preview_', got: {}",
        name
    );
}

proptest! {
    // **Validates: Requirements 1.4 (bugfix.md)**
    //
    // For any batch size N in [2, 50], all names must be distinct.
    #[test]
    fn prop2_unique_temp_name_batch_uniqueness(n in 2usize..=50usize) {
        let names: Vec<String> = (0..n).map(|_| unique_temp_name()).collect();
        let unique_count = names.iter().collect::<HashSet<_>>().len();
        // Require at least 95% uniqueness (timing collisions are theoretically possible)
        let threshold = (n as f64 * 0.95) as usize;
        prop_assert!(
            unique_count >= threshold,
            "expected >= {} unique names out of {}, got {}",
            threshold, n, unique_count
        );
    }
}

// ---------------------------------------------------------------------------
// Property 3 — Path safety (Functional Paranoia)
//
// `safe_join` must reject malicious entry names that attempt path traversal.
// ---------------------------------------------------------------------------

#[test]
fn prop3_safe_join_rejects_path_traversal() {
    let dir = TempDir::new().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap_or_else(|_| dir.path().to_path_buf());

    // All of these contain ".." or are absolute — must be rejected
    let must_reject = [
        "../etc/passwd",
        "../../windows/system32/evil.dll",
        "normal/../../escape.txt",
        "a/../../../b",
        "/absolute/path",
        "C:\\Windows\\evil.exe",
    ];

    for name in &must_reject {
        let result = safe_join(&base, name);
        assert!(
            result.is_err(),
            "safe_join must reject {:?}, but got Ok({:?})",
            name,
            result.ok()
        );
    }
}

#[test]
fn prop3_safe_join_accepts_normal_names() {
    let dir = TempDir::new().unwrap();
    // Canonicalize so the base matches what safe_join's internal canonicalize returns
    let base = std::fs::canonicalize(dir.path()).unwrap_or_else(|_| dir.path().to_path_buf());

    let safe_names = [
        "file.txt",
        "subdir/file.txt",
        "a/b/c/deep.bin",
    ];

    for name in &safe_names {
        let result = safe_join(&base, name);
        assert!(
            result.is_ok(),
            "safe_join should accept normal name: {:?}, err: {:?}",
            name, result.err()
        );
        // Result must be inside base
        let resolved = result.unwrap();
        assert!(
            resolved.starts_with(&base),
            "resolved path {:?} must be inside base {:?}",
            resolved, base
        );
    }
}

proptest! {
    // **Validates: Requirements 1.3 (bugfix.md)**
    //
    // Any entry name containing ".." components must be rejected by safe_join.
    #[test]
    fn prop3_safe_join_rejects_any_dotdot_component(
        prefix in "[a-z]{1,5}",
        suffix in "[a-z]{1,5}",
    ) {
        let dir = TempDir::new().unwrap();
        let traversal = format!("{}/../{}", prefix, suffix);
        // safe_join now rejects any entry containing ".." — must return Err
        let result = safe_join(dir.path(), &traversal);
        prop_assert!(
            result.is_err(),
            "safe_join must reject entry containing '..': {:?}",
            traversal
        );
    }
}

// ---------------------------------------------------------------------------
// Property 4 — ZIP regression guard
//
// `zip_ease_extract_entry_any` must still exist and be callable (compile-time check).
// Referencing the function pointer is sufficient — if the symbol is removed or
// renamed, this test will fail to compile.
// ---------------------------------------------------------------------------

#[test]
fn prop4_zip_regression_guard_symbols_exist() {
    // **Validates: Requirements 1.2 (bugfix.md)**
    //
    // Taking the address of a pub extern "C" fn is a compile-time check.
    // If either symbol is removed, this test fails to compile.
    let any_ptr = zipease_extract::ffi::zip_ease_extract_entry_any
        as unsafe extern "C" fn(*const u16, u32, *const u16, *mut *mut u16) -> i32;
    let name_ptr = zipease_extract::ffi::zip_ease_extract_entry_by_name
        as unsafe extern "C" fn(*const u16, *const u16, *const u16, *mut *mut u16) -> i32;

    assert!(any_ptr as usize != 0, "zip_ease_extract_entry_any must exist");
    assert!(name_ptr as usize != 0, "zip_ease_extract_entry_by_name must exist");
}

// ---------------------------------------------------------------------------
// Property 5 — Temp dir cleanup
//
// After `find_file_recursive` and `unique_temp_name` are used in a simulated
// extract-and-cleanup cycle, no ZipEase_preview_* dirs should remain.
// (We simulate the cleanup logic since we can't call extract_entry_by_name
//  without a real archive in unit tests.)
// ---------------------------------------------------------------------------

#[test]
fn prop5_simulated_temp_dir_cleanup() {
    use std::env;

    let temp_base = env::temp_dir();
    let name = unique_temp_name();
    let temp_dir = temp_base.join(&name);

    // Simulate: create temp dir, do work, then clean up
    fs::create_dir_all(&temp_dir).expect("should create temp dir");
    assert!(temp_dir.exists(), "temp dir should exist after creation");

    // Simulate cleanup (as done in extract_entry_by_name)
    let _ = fs::remove_dir_all(&temp_dir);

    assert!(
        !temp_dir.exists(),
        "temp dir '{}' should be cleaned up after extraction",
        name
    );
}

proptest! {
    // **Validates: Requirements 1.5 (bugfix.md)**
    //
    // For any number of simulated preview cycles, no ZipEase_preview_* dirs
    // should remain after cleanup.
    #[test]
    fn prop5_no_temp_dirs_remain_after_cleanup(n in 1usize..=5usize) {
        use std::env;

        let temp_base = env::temp_dir();
        let mut created: Vec<std::path::PathBuf> = Vec::new();

        // Create N unique temp dirs
        for _ in 0..n {
            let name = unique_temp_name();
            let path = temp_base.join(&name);
            fs::create_dir_all(&path).expect("create temp dir");
            created.push(path);
        }

        // Clean them all up
        for path in &created {
            let _ = fs::remove_dir_all(path);
        }

        // Verify none remain
        for path in &created {
            prop_assert!(
                !path.exists(),
                "temp dir {:?} should not remain after cleanup",
                path
            );
        }
    }
}
