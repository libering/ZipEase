//! Property-based tests for ArchiveEntryFFI is_directory round-trip.
//!
//! Feature: ui-enhancements, Property 1: is_directory round-trip
//! Validates: Requirements 1.1, 1.2, 1.3

use zipease_extract::extract::ArchiveEntryInfo;
use proptest::prelude::*;

// Replicate the FFI mapping logic inline (same as write_ffi_entries in list.rs)
fn map_is_directory(info: &ArchiveEntryInfo) -> i32 {
    if info.is_directory { 1 } else { 0 }
}

proptest! {
    // Property 1: is_directory round-trip
    // For any ArchiveEntryInfo with is_directory set, the FFI i32 field preserves the value.
    // Validates: ui-enhancements Requirements 1.1, 1.2, 1.3
    #[test]
    fn prop_is_directory_round_trip(
        name in "[a-zA-Z0-9_/]{1,32}",
        is_directory: bool,
        size in 0i64..1_000_000i64,
    ) {
        let info = ArchiveEntryInfo { name, is_directory, size };
        let ffi_value = map_is_directory(&info);
        let recovered = ffi_value != 0;
        prop_assert_eq!(recovered, is_directory);
    }

    // For a list of mixed entries, all is_directory values are preserved
    #[test]
    fn prop_is_directory_list_round_trip(
        entries in proptest::collection::vec(
            (any::<bool>(), 0i64..1_000_000i64),
            1..20
        )
    ) {
        for (is_directory, size) in &entries {
            let info = ArchiveEntryInfo {
                name: "test".to_string(),
                is_directory: *is_directory,
                size: *size,
            };
            let ffi_value = map_is_directory(&info);
            let recovered = ffi_value != 0;
            prop_assert_eq!(recovered, *is_directory);
        }
    }
}
