// Feature: image-preview-plugin, Property 11: safe_join rejects all path traversal
//
// **Validates: Requirements 9.5, 9.6**
//
// For any base directory and entry name containing `..` components, absolute path
// prefixes, or null bytes, `safe_join` returns an error. For any entry name without
// traversal components, the resulting path starts with the base directory.

use proptest::prelude::*;
use tempfile::TempDir;

/// Strategy: generate entry names containing `..` path traversal components.
fn traversal_entry_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Simple parent traversal
        Just("../escape".to_string()),
        Just("..\\escape".to_string()),
        Just("a/../../b".to_string()),
        Just("a\\..\\..\\b".to_string()),
        Just("foo/../../../etc/passwd".to_string()),
        Just("..".to_string()),
        // Randomized: prefix/../../suffix
        "[a-z]{1,8}".prop_map(|prefix| format!("{}/../../../etc/passwd", prefix)),
        "[a-z]{1,8}".prop_map(|prefix| format!("{}/../../secret.txt", prefix)),
        "[a-z]{1,4}/[a-z]{1,4}".prop_map(|mid| format!("{}/../../../root", mid)),
    ]
}

/// Strategy: generate entry names with absolute path prefixes.
fn absolute_path_entry_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Unix absolute paths
        Just("/etc/passwd".to_string()),
        Just("/tmp/evil".to_string()),
        // Windows absolute paths
        Just("C:\\Windows\\System32\\evil.dll".to_string()),
        Just("D:\\secret\\data.txt".to_string()),
        Just("C:/Windows/evil.dll".to_string()),
        // UNC paths
        Just("\\\\server\\share\\file.txt".to_string()),
        // Windows device paths
        Just("\\\\.\\COM1".to_string()),
        // Randomized absolute paths
        "[A-Z]".prop_map(|drive| format!("{}:\\malicious\\file.txt", drive)),
        "[a-z]{1,8}".prop_map(|name| format!("/tmp/{}", name)),
    ]
}

/// Strategy: generate entry names containing null bytes.
fn null_byte_entry_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("file\0.txt".to_string()),
        Just("\0hidden".to_string()),
        Just("path/to\0/file.txt".to_string()),
        // Randomized: inject null byte at random position
        "[a-z]{1,8}".prop_map(|name| format!("{}\0.exe", name)),
        "[a-z]{1,4}/[a-z]{1,4}".prop_map(|path| format!("{}\0", path)),
    ]
}

/// Strategy: generate valid entry names (no traversal, no absolute, no null bytes).
/// These should always succeed with safe_join.
fn valid_entry_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Simple filenames
        "[a-zA-Z0-9_]{1,16}\\.[a-z]{1,4}".prop_map(|s| s),
        // Nested paths (no traversal)
        "[a-z]{1,8}/[a-z]{1,8}\\.[a-z]{1,3}".prop_map(|s| s),
        "[a-z]{1,6}/[a-z]{1,6}/[a-z]{1,8}\\.[a-z]{1,3}".prop_map(|s| s),
        // Filenames with spaces and dashes
        "[a-zA-Z0-9_ -]{1,20}\\.[a-z]{1,4}".prop_map(|s| {
            // Ensure it doesn't accidentally produce ".." or start with "/" or contain null
            let cleaned = s.replace("..", "xx").replace('\0', "x");
            if cleaned.starts_with('/') || cleaned.starts_with('\\') {
                format!("a{}", cleaned)
            } else if cleaned.is_empty() {
                "file.txt".to_string()
            } else {
                cleaned
            }
        }),
    ]
}

proptest! {
    /// Property 11a: Entry names with `..` traversal components are always rejected.
    #[test]
    fn traversal_entries_rejected(entry_name in traversal_entry_strategy()) {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let result = zipease_extract::extract::safe_join(base, &entry_name);
        prop_assert!(
            result.is_err(),
            "Expected safe_join to reject traversal entry {:?}, but got Ok({:?})",
            entry_name,
            result.unwrap()
        );
    }

    /// Property 11b: Entry names with absolute path prefixes are always rejected.
    #[test]
    fn absolute_path_entries_rejected(entry_name in absolute_path_entry_strategy()) {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let result = zipease_extract::extract::safe_join(base, &entry_name);
        prop_assert!(
            result.is_err(),
            "Expected safe_join to reject absolute path entry {:?}, but got Ok({:?})",
            entry_name,
            result.unwrap()
        );
    }

    /// Property 11c: Entry names containing null bytes are always rejected.
    #[test]
    fn null_byte_entries_rejected(entry_name in null_byte_entry_strategy()) {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let result = zipease_extract::extract::safe_join(base, &entry_name);
        prop_assert!(
            result.is_err(),
            "Expected safe_join to reject null-byte entry {:?}, but got Ok({:?})",
            entry_name,
            result.unwrap()
        );
    }

    /// Property 11d: Valid entry names (no traversal, no absolute, no null bytes)
    /// produce a path that starts with the base directory.
    #[test]
    fn valid_entries_resolve_within_base(entry_name in valid_entry_strategy()) {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let result = zipease_extract::extract::safe_join(base, &entry_name);
        prop_assert!(
            result.is_ok(),
            "Expected safe_join to accept valid entry {:?}, but got Err({:?})",
            entry_name,
            result.unwrap_err()
        );

        let resolved = result.unwrap();
        prop_assert!(
            resolved.starts_with(base),
            "Expected resolved path {:?} to start with base {:?} for entry {:?}",
            resolved,
            base,
            entry_name
        );
    }
}
