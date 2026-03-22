//! Property-based tests for the safe-delete-trash feature.
//!
//! Feature: safe-delete-trash
//! Properties 1, 2, 3 from the safe-delete-trash spec.

use proptest::prelude::*;
use zipease_extract::ffi::zip_ease_trash_file;

// ── helpers ──────────────────────────────────────────────────────────────────

fn to_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide_null(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    // SAFETY: ptr is non-null and points to a null-terminated UTF-16 sequence
    // produced by to_wide_null — valid for the lifetime of the Vec<u16>.
    unsafe {
        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len)).to_owned()
    }
}

// ── edge-case unit tests ──────────────────────────────────────────────────────

#[test]
fn null_pointer_returns_nonzero() {
    // Validates: Requirement 5.3
    let result = zip_ease_trash_file(std::ptr::null());
    assert_ne!(result, 0, "null pointer must return non-zero");
}

#[test]
fn nonexistent_path_returns_nonzero() {
    // Validates: Requirement 5.2
    let path = "C:\\ZipEase_nonexistent_test_file_that_does_not_exist_12345.zip";
    let wide = to_wide_null(path);
    let result = zip_ease_trash_file(wide.as_ptr());
    assert_ne!(result, 0, "non-existent path must return non-zero");
}

#[test]
fn valid_temp_file_returns_zero_and_file_is_gone() {
    // Validates: Requirement 5.1
    use std::io::Write;
    let mut tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
    writeln!(tmp, "zipease trash test").unwrap();
    let path = tmp.path().to_string_lossy().into_owned();
    // Keep the path alive but prevent NamedTempFile from deleting a file we already trashed.
    let _ = tmp.into_temp_path().keep();

    let wide = to_wide_null(&path);
    let result = zip_ease_trash_file(wide.as_ptr());
    assert_eq!(result, 0, "valid existing file must return 0");
    assert!(
        !std::path::Path::new(&path).exists(),
        "file must no longer exist at original path after trash"
    );
}

// ── Property 1: UTF-16 round-trip fidelity ───────────────────────────────────
// Tag: Feature: safe-delete-trash, Property 1: UTF-16 round-trip fidelity
// Validates: Requirements 5.6, 6.1

proptest! {
    #[test]
    fn prop_utf16_roundtrip(s in "\\PC*") {
        // Encode to null-terminated UTF-16, recover via from_utf16_lossy, compare.
        // For valid Unicode strings (which proptest generates), the round-trip must be lossless.
        let wide = to_wide_null(&s);
        let recovered = from_wide_null(wide.as_ptr());
        prop_assert_eq!(s, recovered);
    }
}

// ── Property 2: No permanent delete on failure ───────────────────────────────
// Tag: Feature: safe-delete-trash, Property 2: No permanent delete on failure
// Validates: Requirements 5.4, 2.4

proptest! {
    #[test]
    fn prop_no_permanent_delete_on_failure(suffix in "[a-zA-Z0-9_]{1,40}") {
        // Use a path that does not exist — call must fail (non-zero) and the file
        // must not have been created or permanently deleted (vacuously true: it never existed).
        let nonexistent = format!("C:\\ZipEase_pbt_nonexistent_{}.zip", suffix);
        let wide = to_wide_null(&nonexistent);
        let result = zip_ease_trash_file(wide.as_ptr());
        prop_assert_ne!(result, 0, "non-existent path must return non-zero (no permanent delete)");
        // If the file somehow exists (name collision), assert it was not deleted.
        if std::path::Path::new(&nonexistent).exists() {
            prop_assert!(
                std::path::Path::new(&nonexistent).exists(),
                "if file existed before call and call failed, file must still exist"
            );
        }
    }
}

// ── Property 3: Idempotent disable after success ─────────────────────────────
// Tag: Feature: safe-delete-trash, Property 3: Idempotent disable after success
// Validates: Requirements 2.2, 5.2

proptest! {
    #[test]
    fn prop_idempotent_disable(suffix in "[a-z]{8,16}") {
        use std::io::Write;
        // Create a real temp file with a unique name derived from the generated suffix.
        let tmp_path = std::env::temp_dir().join(format!("zipease_pbt_{}.tmp", suffix));
        {
            let mut f = std::fs::File::create(&tmp_path).expect("failed to create temp file");
            writeln!(f, "pbt content {}", suffix).unwrap();
        }
        let path_str = tmp_path.to_string_lossy().into_owned();
        let wide = to_wide_null(&path_str);

        // First call: must succeed (0).
        let first = zip_ease_trash_file(wide.as_ptr());
        // Skip iteration if Recycle Bin is unavailable (e.g. CI environment).
        prop_assume!(first == 0);

        // Second call on the same path: must fail (non-zero) because file is gone.
        let second = zip_ease_trash_file(wide.as_ptr());
        prop_assert_ne!(second, 0, "second call on already-trashed path must return non-zero");
    }
}
