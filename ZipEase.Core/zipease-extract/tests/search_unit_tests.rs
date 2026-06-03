/// Unit tests for archive search edge cases.
///
/// Feature: archive-search
/// Validates: Requirements 1.1, 2.1, 2.2, 2.4, 5.3
use std::sync::atomic::AtomicBool;
use zipease_extract::search::search_entries;

/// Empty pattern returns empty results.
#[test]
fn empty_pattern_returns_empty() {
    let entries = vec![
        "file.txt".to_string(),
        "photo.jpg".to_string(),
        "README.md".to_string(),
    ];
    let cancelled = AtomicBool::new(false);
    let result = search_entries("", &entries, &cancelled);
    assert!(result.is_empty(), "Empty pattern should return no results");
}

/// `*.jpg` matches `photo.jpg` but not `photo.png`.
#[test]
fn glob_star_extension_matching() {
    let entries = vec![
        "photo.jpg".to_string(),
        "photo.png".to_string(),
        "image.jpg".to_string(),
    ];
    let cancelled = AtomicBool::new(false);
    let result = search_entries("*.jpg", &entries, &cancelled);
    assert_eq!(result, vec![0, 2], "*.jpg should match .jpg files only");
}

/// `??.txt` matches `ab.txt` but not `abc.txt`.
#[test]
fn glob_question_mark_matching() {
    let entries = vec![
        "ab.txt".to_string(),
        "abc.txt".to_string(),
        "xy.txt".to_string(),
        "a.txt".to_string(),
    ];
    let cancelled = AtomicBool::new(false);
    let result = search_entries("??.txt", &entries, &cancelled);
    assert_eq!(result, vec![0, 2], "??.txt should match exactly 2-char filenames");
}

/// CJK substring: `報告` matches `2024年度報告.pdf`.
#[test]
fn cjk_substring_matching() {
    let entries = vec![
        "2024年度報告.pdf".to_string(),
        "readme.txt".to_string(),
        "月報告書.docx".to_string(),
    ];
    let cancelled = AtomicBool::new(false);
    let result = search_entries("報告", &entries, &cancelled);
    assert_eq!(result, vec![0, 2], "CJK substring should match entries containing 報告");
}

/// Case-insensitive: `readme` matches `README.md`.
#[test]
fn case_insensitive_substring() {
    let entries = vec![
        "README.md".to_string(),
        "Readme.txt".to_string(),
        "other.rs".to_string(),
        "readme_backup.md".to_string(),
    ];
    let cancelled = AtomicBool::new(false);
    let result = search_entries("readme", &entries, &cancelled);
    assert_eq!(result, vec![0, 1, 3], "Case-insensitive search should match all readme variants");
}

/// Path separator handling: `src/*.rs` matches `src/main.rs` and nested paths.
/// Note: globset's `*` matches path separators by default (no literal_separator),
/// so `src/*.rs` also matches `src/utils/helper.rs`.
#[test]
fn path_separator_glob_matching() {
    let entries = vec![
        "src/main.rs".to_string(),
        "src/lib.rs".to_string(),
        "tests/test.rs".to_string(),
        "src/utils/helper.rs".to_string(),
    ];
    let cancelled = AtomicBool::new(false);
    let result = search_entries("src/*.rs", &entries, &cancelled);
    // src/*.rs should match .rs files under src/ (globset * crosses path separators)
    assert!(result.contains(&0), "src/*.rs should match src/main.rs");
    assert!(result.contains(&1), "src/*.rs should match src/lib.rs");
    assert!(!result.contains(&2), "src/*.rs should not match tests/test.rs");
    // globset's * matches path separators, so nested paths also match
    assert!(result.contains(&3), "src/*.rs should match src/utils/helper.rs (globset * crosses separators)");
}

/// Glob fallback: invalid glob `[unclosed` falls back to substring match.
/// It should match entries that contain the literal text "[unclosed".
#[test]
fn invalid_glob_falls_back_to_substring() {
    let entries = vec![
        "normal_file.txt".to_string(),
        "file_with_[unclosed_bracket.txt".to_string(),
        "[unclosed_pattern_test.rs".to_string(),
        "other.rs".to_string(),
    ];
    let cancelled = AtomicBool::new(false);
    // "[unclosed" is not a valid glob (unclosed bracket), so it should
    // fall back to substring matching
    let result = search_entries("[unclosed", &entries, &cancelled);
    assert_eq!(
        result,
        vec![1, 2],
        "Invalid glob should fall back to substring match for entries containing '[unclosed'"
    );
}
