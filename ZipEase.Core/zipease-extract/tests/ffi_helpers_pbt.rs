//! Property-based tests for FFI helper functions.
//!
//! Feature: ui-integration, Property 15: String marshaling to UTF-16
//! Feature: ui-overhaul, Property 6: CJK encoding round-trip

use zipease_shared::{parse_wide_string, to_utf16_ptr, decode_filename};
use proptest::prelude::*;

proptest! {
    // Property 15 (ui-integration): UTF-16 round-trip
    // For any String, converting to UTF-16 via to_utf16_ptr and back via parse_wide_string
    // produces the original string.
    // Validates: ui-integration Requirements 9.1
    #[test]
    fn prop_utf16_round_trip(s: String) {
        let ptr = to_utf16_ptr(&s);
        let result = unsafe { parse_wide_string(ptr as *const u16) };
        // Free the allocated memory
        unsafe {
            let mut len = 0usize;
            while *ptr.add(len) != 0 { len += 1; }
            let _ = Box::from_raw(std::slice::from_raw_parts_mut(ptr, len + 1));
        }
        prop_assert_eq!(result.to_string_lossy().into_owned(), s);
    }

    // Property 6 (ui-overhaul): CJK encoding round-trip
    // For any Unicode string, decode_filename(s.as_bytes()) returns the original string
    // (since valid UTF-8 is always returned unchanged).
    // Validates: ui-overhaul Requirements 5.5
    #[test]
    fn prop_cjk_utf8_round_trip(s: String) {
        // decode_filename tries UTF-8 first — any valid UTF-8 string is returned unchanged
        let result = decode_filename(s.as_bytes());
        prop_assert_eq!(result, s);
    }
}
