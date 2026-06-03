pub mod pattern;

use std::sync::atomic::{AtomicBool, Ordering};

/// Search mode detected from the pattern string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    /// Plain case-insensitive substring matching (pattern contains no `*` or `?`).
    Substring,
    /// Glob pattern matching (pattern contains `*` or `?`).
    Glob,
}

/// Detect whether a pattern should be interpreted as a glob or a plain substring.
///
/// A pattern is treated as glob if it contains `*` or `?`.
pub fn detect_mode(pattern: &str) -> SearchMode {
    if pattern.contains('*') || pattern.contains('?') {
        SearchMode::Glob
    } else {
        SearchMode::Substring
    }
}

/// Search through `entries` using the given `pattern`, returning indices of matching entries.
///
/// Behaviour:
/// 1. Returns an empty `Vec` if `pattern` is empty.
/// 2. Detects mode (substring vs glob) via [`detect_mode`].
/// 3. For glob mode: compiles the glob pattern (case-insensitive). On compile failure,
///    falls back to substring matching.
/// 4. For substring mode: uses case-insensitive substring matching.
/// 5. Checks the `cancelled` flag every 1024 iterations — if set, returns an empty `Vec`.
/// 6. Returns indices of all matching entries.
pub fn search_entries(
    pattern: &str,
    entries: &[String],
    cancelled: &AtomicBool,
) -> Vec<usize> {
    if pattern.is_empty() {
        return Vec::new();
    }

    let mode = detect_mode(pattern);

    // Determine the effective matching strategy.
    // For glob mode, attempt to compile; on failure fall back to substring.
    enum Matcher {
        Glob(globset::GlobMatcher),
        Substring(String),
    }

    let matcher = match mode {
        SearchMode::Glob => match pattern::compile_glob(pattern) {
            Ok(glob_matcher) => Matcher::Glob(glob_matcher),
            Err(_) => {
                // Glob compile failed — fallback to substring (cognitive ease: no error shown)
                Matcher::Substring(pattern.to_lowercase())
            }
        },
        SearchMode::Substring => Matcher::Substring(pattern.to_lowercase()),
    };

    let mut results = Vec::new();

    for (i, entry) in entries.iter().enumerate() {
        // Check cancellation every 1024 iterations
        if i & 0x3FF == 0 && cancelled.load(Ordering::Relaxed) {
            return Vec::new();
        }

        let matched = match &matcher {
            Matcher::Glob(glob_matcher) => glob_matcher.is_match(entry),
            Matcher::Substring(needle_lower) => pattern::substring_match(entry, needle_lower),
        };

        if matched {
            results.push(i);
        }
    }

    results
}
