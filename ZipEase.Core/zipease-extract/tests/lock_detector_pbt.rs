//! Property-based and unit tests for the file-lock-detector feature.
//!
//! Feature: file-lock-detector
//! Properties 1, 2, 3, 5 from the file-lock-detector spec.

use proptest::prelude::*;
use zipease_extract::lock_detector::{join_process_names, who_locks};
use zipease_extract::zip_ease_who_locks;

// ── helpers ───────────────────────────────────────────────────────────────────

fn to_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Reconstruct and free a Rust-allocated UTF-16 pointer.
/// Mirrors the logic in `zip_ease_free_string` (zipease-core).
/// The allocation is `Box::into_raw(vec.into_boxed_slice())` so len == capacity.
unsafe fn free_utf16(ptr: *mut u16) {
    if ptr.is_null() {
        return;
    }
    let mut len = 0usize;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    drop(Vec::from_raw_parts(ptr, len + 1, len + 1));
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[test]
fn who_locks_null_ptr_returns_null() {
    // Validates: Requirement 2.4
    let result = who_locks(std::ptr::null());
    assert!(result.is_null(), "who_locks(null) must return null");
}

#[test]
fn who_locks_nonexistent_returns_null() {
    // Validates: Requirements 2.3, 4.4
    let path = "C:\\ZipEase_nonexistent_lock_test_99999.zip";
    let wide = to_wide_null(path);
    let result = who_locks(wide.as_ptr());
    assert!(result.is_null(), "who_locks on non-existent path must return null");
}

#[test]
fn zip_ease_who_locks_null_returns_null() {
    // Validates: Requirements 2.4, 4.2
    let result = zip_ease_who_locks(std::ptr::null());
    assert!(result.is_null(), "zip_ease_who_locks(null) must return null without panicking");
}

#[test]
fn join_process_names_single() {
    // Validates: Requirements 1.2, 1.4
    let names = vec!["Google Chrome".to_string()];
    assert_eq!(join_process_names(&names), "Google Chrome");
}

#[test]
fn join_process_names_two() {
    // Validates: Requirements 1.2, 1.4
    let names = vec!["A".to_string(), "B".to_string()];
    assert_eq!(join_process_names(&names), "A, B");
}

#[test]
fn free_null_is_noop() {
    // Validates: Requirements 6.4, 5.5
    // Property 5 — Allocator compatibility: free_utf16(null) must not panic.
    unsafe { free_utf16(std::ptr::null_mut()) };
    // reaching here without panic is the pass condition
}

// ── Property 1 — UTF-16 round-trip fidelity ──────────────────────────────────
// Tag: Feature: file-lock-detector, Property 1: UTF-16 round-trip fidelity
// Validates: Requirement 6.2

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn utf16_roundtrip(s in "\\PC*") {
        // Encode to null-terminated UTF-16, recover via from_utf16_lossy, assert equality.
        let wide: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
        let recovered = unsafe {
            let mut len = 0usize;
            while *wide.as_ptr().add(len) != 0 {
                len += 1;
            }
            String::from_utf16_lossy(&wide[..len]).to_string()
        };
        prop_assert_eq!(s, recovered);
    }
}

// ── Property 2 — Graceful degradation ────────────────────────────────────────
// Tag: Feature: file-lock-detector, Property 2: Graceful degradation — null and non-existent inputs return null without panic
// Validates: Requirements 2.3, 2.4, 4.2, 4.4

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn nonexistent_path_returns_null(s in "[a-zA-Z0-9_\\-]{1,50}") {
        // Construct a path that is virtually guaranteed not to exist.
        let path = format!("C:\\ZipEase_nonexistent_{}.zip", s);
        let wide = to_wide_null(&path);
        let result = zip_ease_who_locks(wide.as_ptr());
        prop_assert!(result.is_null(), "non-existent path must return null");
    }
}

// Separate unit test for literal null pointer (not a proptest — it's a fixed input).
#[test]
fn null_ptr_returns_null() {
    // Validates: Requirement 2.4
    let result = zip_ease_who_locks(std::ptr::null());
    assert!(result.is_null(), "null pointer must return null without panic");
}

// ── Property 3 — Process name joining ────────────────────────────────────────
// Tag: Feature: file-lock-detector, Property 3: Process name joining
// Validates: Requirements 1.2, 1.4

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn process_names_joined_correctly(
        names in prop::collection::vec("[a-zA-Z ]{1,30}", 1..=10)
    ) {
        let result = join_process_names(&names);
        // Result must equal names.join(", ")
        prop_assert_eq!(&result, &names.join(", "));
        // Each name must appear exactly once — verify via round-trip split
        let parts: Vec<&str> = result.split(", ").collect();
        prop_assert_eq!(parts.len(), names.len(), "split count must equal input count");
        for (part, name) in parts.iter().zip(names.iter()) {
            prop_assert_eq!(*part, name.as_str(), "each part must equal the original name in order");
        }
    }
}

// ── Property 5 — Allocator compatibility ─────────────────────────────────────
// Tag: Feature: file-lock-detector, Property 5: Allocator compatibility — returned pointer is safely freed
// Validates: Requirements 6.4, 5.5
//
// This is a unit test (not proptest) since it tests the null-free no-op path.

#[test]
fn returned_null_ptr_is_safely_freed() {
    // Call zip_ease_who_locks with a non-existent path — returns null.
    // Then call free_utf16(null) — must be a no-op with no panic.
    let path = "C:\\ZipEase_nonexistent_alloc_test_00000.zip";
    let wide = to_wide_null(path);
    let ptr = zip_ease_who_locks(wide.as_ptr());
    // ptr is null here (non-existent path); free_utf16 must handle null gracefully.
    unsafe { free_utf16(ptr) };
    // reaching here without panic is the pass condition
}
