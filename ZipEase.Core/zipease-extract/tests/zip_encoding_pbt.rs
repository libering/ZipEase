//! Property-based tests for `decode_zip_filename`.
//!
//! Feature: zip-encoding
//! Properties 1, 2, 4, 5 from the zip-encoding spec.

use zipease_extract::extract::encoding::decode_zip_filename;
use proptest::prelude::*;

proptest! {
    // Property 1: No panic for arbitrary byte input
    // Validates: Requirements 3.5
    #[test]
    fn prop_no_panic(raw: Vec<u8>, flag: bool) {
        let _ = decode_zip_filename(&raw, flag);
    }

    // Property 2: UTF-8 identity
    // For any valid UTF-8 string, decode_zip_filename returns it unchanged.
    // Validates: Requirements 2.1, 3.1, 4.7, 8.1, 8.4
    #[test]
    fn prop_utf8_identity(s: String, flag: bool) {
        prop_assert_eq!(decode_zip_filename(s.as_bytes(), flag), s);
    }

    // Property 4: Non-empty output for non-empty input
    // Validates: Requirements 8.3
    #[test]
    fn prop_nonempty(raw in proptest::collection::vec(any::<u8>(), 1..256), flag: bool) {
        prop_assert!(!decode_zip_filename(&raw, flag).is_empty());
    }

    // Property 5: Idempotence
    // Decoding the output of decode_zip_filename again (as UTF-8 bytes, flag=false)
    // produces the same string.
    // Validates: Requirements 8.5
    #[test]
    fn prop_idempotent(raw: Vec<u8>, flag: bool) {
        let first = decode_zip_filename(&raw, flag);
        let second = decode_zip_filename(first.as_bytes(), false);
        prop_assert_eq!(first, second);
    }
}
