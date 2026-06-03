use globset::{GlobBuilder, GlobMatcher};

/// Compile a glob pattern for case-insensitive matching.
///
/// If the pattern does not contain a path separator (`/` or `\`), it is automatically
/// prefixed with `**/` so it matches filenames at any directory depth.
pub fn compile_glob(pattern: &str) -> Result<GlobMatcher, String> {
    let effective = if pattern.contains('/') || pattern.contains('\\') {
        pattern.to_string()
    } else {
        format!("**/{pattern}")
    };

    GlobBuilder::new(&effective)
        .case_insensitive(true)
        .build()
        .map_err(|e| e.to_string())
        .map(|g| g.compile_matcher())
}

/// Case-insensitive substring match.
///
/// `needle_lower` must already be lowercased by the caller for efficiency
/// (avoids re-lowering the needle on every entry).
pub fn substring_match(haystack: &str, needle_lower: &str) -> bool {
    haystack.to_lowercase().contains(needle_lower)
}
