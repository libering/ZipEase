/// Property-based tests for archive search engine.
///
/// Feature: archive-search
use proptest::prelude::*;
use std::sync::atomic::AtomicBool;
use zipease_extract::search::search_entries;

// ---------------------------------------------------------------------------
// Property 1 — 子字串搜尋完整性與正確性
// Validates: Requirements 1.1, 2.3, 5.3
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// **Validates: Requirements 1.1, 2.3, 5.3**
    ///
    /// Feature: archive-search, Property 1: 子字串搜尋完整性與正確性
    ///
    /// For any entry list and any substring pattern (no `*` or `?`),
    /// search_entries should return exactly the indices where the filename
    /// contains the pattern (case-insensitive).
    #[test]
    fn prop_substring_search_completeness(
        entries in prop::collection::vec(
            "[a-zA-Z0-9\u{4e00}-\u{9fff}./]{1,50}".prop_map(|s| s),
            1..100
        ),
        needle in "[a-zA-Z0-9\u{4e00}-\u{9fff}]{1,10}"
    ) {
        // Filter out needles that contain glob chars (should not happen with
        // the regex above, but be defensive)
        prop_assume!(!needle.contains('*') && !needle.contains('?'));

        let cancelled = AtomicBool::new(false);
        let result = search_entries(&needle, &entries, &cancelled);

        // Manual reference: case-insensitive substring match
        let needle_lower = needle.to_lowercase();
        let expected: Vec<usize> = entries.iter().enumerate()
            .filter(|(_, e)| e.to_lowercase().contains(&needle_lower))
            .map(|(i, _)| i)
            .collect();

        // Completeness: every entry that contains the needle is in the result
        // Correctness: every entry in the result actually contains the needle
        prop_assert_eq!(
            &result, &expected,
            "search_entries result must match manual case-insensitive substring filter.\n\
             needle={:?}, entries={:?}", needle, entries
        );
    }
}

// ---------------------------------------------------------------------------
// Property 2 — Glob 模式匹配正確性
// Validates: Requirements 2.1, 2.2, 2.4
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// **Validates: Requirements 2.1, 2.2, 2.4**
    ///
    /// Feature: archive-search, Property 2: Glob 模式匹配正確性
    ///
    /// For any entry list and any glob pattern with `*` wildcard (e.g., `*.ext`),
    /// search_entries should return exactly the same indices as a direct globset
    /// reference matcher using the same compile logic.
    #[test]
    fn prop_glob_star_pattern_matching(
        entries in prop::collection::vec(
            "[a-zA-Z]{1,8}\\.(txt|jpg|rs|md|pdf|png)".prop_map(|s| s),
            1..50
        ),
        ext in "(txt|jpg|rs|md|pdf|png)"
    ) {
        let pattern = format!("*.{ext}");
        let cancelled = AtomicBool::new(false);
        let result = search_entries(&pattern, &entries, &cancelled);

        // Reference: use globset directly with same logic as compile_glob
        // compile_glob auto-prefixes with **/ when no path separator is present
        let effective_pattern = format!("**/*.{ext}");
        let glob = globset::Glob::new(&effective_pattern).unwrap().compile_matcher();

        let expected: Vec<usize> = entries.iter().enumerate()
            .filter(|(_, e)| glob.is_match(e.as_str()))
            .map(|(i, _)| i)
            .collect();

        prop_assert_eq!(
            &result, &expected,
            "Glob star pattern mismatch.\npattern={:?}, entries={:?}", pattern, entries
        );
    }

    /// **Validates: Requirements 2.1, 2.2, 2.4**
    ///
    /// Feature: archive-search, Property 2: Glob 模式匹配正確性
    ///
    /// For any entry list and any glob pattern with `?` wildcard,
    /// search_entries should return exactly the same indices as a direct globset
    /// reference matcher. `?` matches exactly one character.
    #[test]
    fn prop_glob_question_mark_matching(
        entries in prop::collection::vec(
            "[a-z]{1,4}\\.(txt|rs)".prop_map(|s| s),
            1..50
        ),
        ext in "(txt|rs)"
    ) {
        // ?? matches exactly 2 chars before the dot
        let pattern = format!("??.{ext}");
        let cancelled = AtomicBool::new(false);
        let result = search_entries(&pattern, &entries, &cancelled);

        // Reference matcher with same compile_glob logic
        let effective_pattern = format!("**/??.{ext}");
        let glob = globset::Glob::new(&effective_pattern).unwrap().compile_matcher();

        let expected: Vec<usize> = entries.iter().enumerate()
            .filter(|(_, e)| glob.is_match(e.as_str()))
            .map(|(i, _)| i)
            .collect();

        prop_assert_eq!(
            &result, &expected,
            "Glob question mark pattern mismatch.\npattern={:?}, entries={:?}", pattern, entries
        );
    }
}

// ---------------------------------------------------------------------------
// Property 3 — 深度搜尋完整性
// Validates: Requirements 3.1, 3.2
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// **Validates: Requirements 3.1, 3.2**
    ///
    /// Feature: archive-search, Property 3: 深度搜尋完整性
    ///
    /// For any multi-level nested path (depth 1–5), searching by the leaf filename
    /// should find the entry regardless of directory depth.
    #[test]
    fn prop_deep_search_completeness(
        // Generate path segments (1–5 levels deep)
        segments in prop::collection::vec("[a-z]{1,6}", 1..=5usize),
        filename in "[a-z]{1,8}\\.(txt|rs|pdf|jpg)"
    ) {
        // Build a nested path like "a/bb/ccc/file.txt"
        let mut path = segments.join("/");
        path.push('/');
        path.push_str(&filename);

        // Create entries with the deep path plus some decoys at other depths
        let entries = vec![
            "root_file.txt".to_string(),
            "shallow/other.rs".to_string(),
            path.clone(),
            "another/deep/nested/decoy.pdf".to_string(),
        ];

        let cancelled = AtomicBool::new(false);
        let result = search_entries(&filename, &entries, &cancelled);

        // The deep path entry (index 2) must always be found
        prop_assert!(
            result.contains(&2),
            "Deep path entry at depth {} was not found by searching filename {:?}.\n\
             path={:?}, result={:?}",
            segments.len(), filename, path, result
        );
    }

    /// **Validates: Requirements 3.1, 3.2**
    ///
    /// Feature: archive-search, Property 3: 深度搜尋完整性
    ///
    /// For any set of entries at varying depths that share a common filename substring,
    /// all of them should be found regardless of depth.
    #[test]
    fn prop_deep_search_finds_all_depths(
        // Generate a common filename component (use digits to avoid accidental matches)
        common in "[a-z]{2,5}",
        // Generate 1–5 entries at different depths
        depths in prop::collection::vec(1..=5usize, 1..=5)
    ) {
        let mut entries: Vec<String> = Vec::new();
        for (i, &depth) in depths.iter().enumerate() {
            // Use numeric-only path segments to avoid accidentally containing `common`
            let segments: Vec<String> = (0..depth)
                .map(|d| format!("{}{}", d, i))
                .collect();
            let mut path = segments.join("/");
            path.push('/');
            path.push_str(&format!("{common}_file{i}.txt"));
            entries.push(path);
        }

        // Add a decoy that uses only digits/underscores/uppercase — guaranteed not to contain
        // any lowercase alpha substring from `common` (which is [a-z]{2,5})
        entries.push("9999/0000_1111.000".to_string());

        let cancelled = AtomicBool::new(false);
        let result = search_entries(&common, &entries, &cancelled);

        // Verify: the decoy (last entry) should NOT be in results
        let decoy_idx = entries.len() - 1;
        prop_assert!(
            !result.contains(&decoy_idx),
            "Decoy should not match common={:?}", common
        );

        // All entries with the common substring (indices 0..depths.len()) should be found
        for i in 0..depths.len() {
            prop_assert!(
                result.contains(&i),
                "Entry at depth {} (index {}) should be found for common={:?}.\nentries={:?}, result={:?}",
                depths[i], i, common, entries, result
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Property 6 — 取消安全性
// Validates: Requirements 7.2
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// **Validates: Requirements 7.2**
    ///
    /// Feature: archive-search, Property 6: 取消安全性
    ///
    /// When the cancel flag is set before calling search_entries with 10,000+ entries,
    /// the result must be empty (no partial results leak).
    #[test]
    fn prop_cancellation_returns_empty(
        needle in "[a-z]{1,5}"
    ) {
        // Generate 10,000+ entries
        let entries: Vec<String> = (0..10_500)
            .map(|i| format!("entry_{i}_{needle}.txt"))
            .collect();

        // Set cancel flag BEFORE calling search
        let cancelled = AtomicBool::new(true);
        let result = search_entries(&needle, &entries, &cancelled);

        // Must return empty — cancelled search yields no results
        prop_assert!(
            result.is_empty(),
            "Cancelled search should return empty Vec, got {} results", result.len()
        );
    }
}

// ---------------------------------------------------------------------------
// Property 7 — FFI Panic 安全性
// Validates: Requirements 5.2
// ---------------------------------------------------------------------------

// These are deterministic edge-case tests for FFI safety, but placed in the PBT
// file as they validate Property 7.

/// Feature: archive-search, Property 7: FFI Panic 安全性
///
/// **Validates: Requirements 5.2**
#[cfg(test)]
mod ffi_panic_safety {
    use zipease_extract::ffi::search::zip_ease_search_entries;
    use std::ptr;

    /// Null pattern_ptr → returns -1 (parameter error)
    #[test]
    fn null_pattern_returns_error() {
        let mut out_indices: *mut i32 = ptr::null_mut();
        let mut out_count: i32 = 0;

        let result = zip_ease_search_entries(
            ptr::null(),       // null pattern
            ptr::null(),       // null entries (also invalid, but pattern checked first)
            0,
            ptr::null(),
            &mut out_indices,
            &mut out_count,
        );

        assert_eq!(result, -1, "Null pattern_ptr should return -1");
    }

    /// Null entries_ptr → returns -1 (parameter error)
    #[test]
    fn null_entries_returns_error() {
        // Create a valid UTF-16 pattern "test\0"
        let pattern: Vec<u16> = "test\0".encode_utf16().collect();

        let mut out_indices: *mut i32 = ptr::null_mut();
        let mut out_count: i32 = 0;

        let result = zip_ease_search_entries(
            pattern.as_ptr(),
            ptr::null(),       // null entries
            5,                 // non-zero count with null ptr
            ptr::null(),
            &mut out_indices,
            &mut out_count,
        );

        assert_eq!(result, -1, "Null entries_ptr should return -1");
    }

    /// Null out_indices_ptr → returns -1 (parameter error)
    #[test]
    fn null_out_indices_returns_error() {
        let pattern: Vec<u16> = "test\0".encode_utf16().collect();

        let mut out_count: i32 = 0;

        let result = zip_ease_search_entries(
            pattern.as_ptr(),
            ptr::null(),       // entries don't matter, out ptr checked
            0,
            ptr::null(),
            ptr::null_mut(),   // null out_indices
            &mut out_count,
        );

        assert_eq!(result, -1, "Null out_indices_ptr should return -1");
    }

    /// Null out_count → returns -1 (parameter error)
    #[test]
    fn null_out_count_returns_error() {
        let pattern: Vec<u16> = "test\0".encode_utf16().collect();
        let mut out_indices: *mut i32 = ptr::null_mut();

        let result = zip_ease_search_entries(
            pattern.as_ptr(),
            ptr::null(),       // entries don't matter
            0,
            ptr::null(),
            &mut out_indices,
            ptr::null_mut(),   // null out_count
        );

        assert_eq!(result, -1, "Null out_count should return -1");
    }

    /// count=0 with valid pointers → returns 0 with empty results
    #[test]
    fn zero_count_valid_pointers_returns_success() {
        use zipease_extract::ffi::list::ArchiveEntryFFI;

        // Valid UTF-16 pattern "*.txt\0"
        let pattern: Vec<u16> = "*.txt\0".encode_utf16().collect();

        let mut out_indices: *mut i32 = ptr::null_mut();
        let mut out_count: i32 = -1; // sentinel to verify it gets set

        // Use a properly aligned, non-null entries pointer with count=0
        // We create a valid ArchiveEntryFFI to get a properly typed pointer
        let dummy_entry = ArchiveEntryFFI {
            file_name_ptr: ptr::null_mut(),
            file_size: 0,
            is_directory: 0,
        };

        let result = zip_ease_search_entries(
            pattern.as_ptr(),
            &dummy_entry as *const ArchiveEntryFFI,
            0,                 // zero entries
            ptr::null(),
            &mut out_indices,
            &mut out_count,
        );

        assert_eq!(result, 0, "Zero count with valid pointers should return 0 (success)");
        assert_eq!(out_count, 0, "Result count should be 0");
        assert!(out_indices.is_null(), "Result pointer should be null for empty results");
    }
}
