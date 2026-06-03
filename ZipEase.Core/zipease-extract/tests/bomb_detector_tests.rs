//! Example-based unit tests for the zip-bomb-protection feature.
//!
//! Feature: zip-bomb-protection
//! Validates: Requirements 1.2, 1.4, 2.2, 3.2, 4.2, 6.2, 6.3

use zipease_extract::extract::bomb_detector::{BombThresholds, check_entries};
use zipease_extract::ArchiveEntryInfo;
use zipease_shared::LockError;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Build a permissive `BombThresholds` that won't trigger any check,
/// then let the caller override specific fields.
fn permissive_thresholds() -> BombThresholds {
    BombThresholds {
        max_compression_ratio: f64::MAX,
        max_total_uncompressed_bytes: u64::MAX,
        max_single_entry_bytes: u64::MAX,
        max_nesting_depth: 100,
        exempt_formats: vec!["iso".to_string()],
    }
}

// ── 1. BombThresholds::default() field values ────────────────────────────────
// **Validates: Requirement 9.1**

#[test]
fn test_default_thresholds() {
    let t = BombThresholds::default();
    assert_eq!(t.max_compression_ratio, 100.0);
    assert_eq!(t.max_total_uncompressed_bytes, 16_106_127_360);
    assert_eq!(t.max_single_entry_bytes, 8_589_934_592);
    assert_eq!(t.max_nesting_depth, 3);
    assert_eq!(t.exempt_formats, vec!["iso".to_string()]);
}

// ── 2. LockError::ZipBomb error code ─────────────────────────────────────────
// **Validates: Requirement 6.2**

#[test]
fn test_error_code_0x2005() {
    let err = LockError::ZipBomb("x".to_string());
    assert_eq!(err.to_error_code(), 0x2005);
}

// ── 3. LockError::ZipBomb message passthrough ────────────────────────────────
// **Validates: Requirement 6.3**

#[test]
fn test_message_passthrough() {
    let err = LockError::ZipBomb("msg".to_string());
    assert_eq!(err.message(), "msg");
}

// ── 4. ISO exempt format skips size checks ───────────────────────────────────
// **Validates: Requirement 2.3, 3.3**

#[test]
fn test_iso_exempt_skips_size() {
    // 20 GiB entry — would trigger both total and single-entry checks for non-exempt formats
    let twenty_gib: i64 = 20 * 1_073_741_824;
    let entries = vec![ArchiveEntryInfo {
        name: "big_image.bin".to_string(),
        is_directory: false,
        size: twenty_gib,
    }];

    let thresholds = BombThresholds::default();
    // archive_file_size large enough to avoid ratio trigger if it weren't exempt
    let result = check_entries(&entries, twenty_gib as u64, "iso", 1, &thresholds);
    assert!(result.is_ok(), "ISO format should be exempt from size checks, got {:?}", result);
}

// ── 5. Unknown size entries (size == -1) are skipped ─────────────────────────
// **Validates: Requirement 3.4, 2.4**

#[test]
fn test_unknown_size_skipped() {
    let entries = vec![
        ArchiveEntryInfo {
            name: "unknown.bin".to_string(),
            is_directory: false,
            size: -1,
        },
        ArchiveEntryInfo {
            name: "small.txt".to_string(),
            is_directory: false,
            size: 100,
        },
    ];

    // Use very strict thresholds — if -1 were counted, it would overflow or trigger
    let thresholds = BombThresholds {
        max_single_entry_bytes: 1_000,
        max_total_uncompressed_bytes: 1_000,
        ..permissive_thresholds()
    };

    let result = check_entries(&entries, 1_000, "zip", 1, &thresholds);
    assert!(result.is_ok(), "Entries with size == -1 should be skipped, got {:?}", result);
}

// ── 6. Zero archive file size does not panic ─────────────────────────────────
// **Validates: Requirement 1.4**

#[test]
fn test_zero_archive_size_no_panic() {
    let entries = vec![ArchiveEntryInfo {
        name: "file.bin".to_string(),
        is_directory: false,
        size: 1_000_000,
    }];

    // Very strict ratio — would trigger if ratio were computed (division by zero avoided)
    let thresholds = BombThresholds {
        max_compression_ratio: 1.0,
        ..permissive_thresholds()
    };

    let result = check_entries(&entries, 0, "zip", 1, &thresholds);
    assert!(result.is_ok(), "archive_file_size == 0 should skip ratio check, got {:?}", result);
}

// ── 7. Nesting at depth 1 below limit → Ok ──────────────────────────────────
// **Validates: Requirement 4.2, 4.3**

#[test]
fn test_nesting_at_depth_1_below_limit() {
    let entries = vec![ArchiveEntryInfo {
        name: "inner.zip".to_string(),
        is_directory: false,
        size: 1_000,
    }];

    let thresholds = BombThresholds {
        max_nesting_depth: 3,
        ..permissive_thresholds()
    };

    // depth 1 < max_nesting_depth 3 → should be Ok
    let result = check_entries(&entries, 10_000, "zip", 1, &thresholds);
    assert!(result.is_ok(), "Depth 1 with max_nesting_depth 3 should be Ok, got {:?}", result);
}

// ── 8. Nesting at depth 3 triggers error ─────────────────────────────────────
// **Validates: Requirement 4.2**

#[test]
fn test_nesting_at_depth_3_triggers() {
    let entries = vec![ArchiveEntryInfo {
        name: "inner.zip".to_string(),
        is_directory: false,
        size: 1_000,
    }];

    let thresholds = BombThresholds {
        max_nesting_depth: 3,
        ..permissive_thresholds()
    };

    // depth 3 >= max_nesting_depth 3 → should trigger
    let result = check_entries(&entries, 10_000, "zip", 3, &thresholds);
    assert!(
        matches!(result, Err(LockError::ZipBomb(_))),
        "Depth 3 with max_nesting_depth 3 and nested .zip should trigger ZipBomb, got {:?}",
        result
    );
}

// ── 9. Error message format: compression ratio ──────────────────────────────
// **Validates: Requirement 1.2**

#[test]
fn test_error_message_format_ratio() {
    // Create entries with total uncompressed = 10_000, archive_file_size = 10
    // → ratio = 1000, which exceeds default 100
    let entries = vec![ArchiveEntryInfo {
        name: "bomb.bin".to_string(),
        is_directory: false,
        size: 10_000,
    }];

    let thresholds = BombThresholds {
        max_compression_ratio: 100.0,
        ..permissive_thresholds()
    };

    let result = check_entries(&entries, 10, "zip", 1, &thresholds);
    match result {
        Err(LockError::ZipBomb(msg)) => {
            // ratio = 10_000 / 10 = 1000
            assert!(msg.contains("1000"), "Message should contain ratio value '1000', got: {}", msg);
            assert!(msg.contains("100"), "Message should contain limit '100', got: {}", msg);
        }
        other => panic!("Expected ZipBomb error, got {:?}", other),
    }
}

// ── 10. Error message format: total size ─────────────────────────────────────
// **Validates: Requirement 2.2**

#[test]
fn test_error_message_format_total() {
    // 20 GiB total, limit 15 GiB
    let twenty_gib: i64 = 20 * 1_073_741_824;
    let entries = vec![ArchiveEntryInfo {
        name: "big.bin".to_string(),
        is_directory: false,
        size: twenty_gib,
    }];

    let thresholds = BombThresholds {
        max_total_uncompressed_bytes: 16_106_127_360, // 15 GiB
        ..permissive_thresholds()
    };

    // Use large archive_file_size to avoid ratio trigger
    let result = check_entries(&entries, twenty_gib as u64, "zip", 1, &thresholds);
    match result {
        Err(LockError::ZipBomb(msg)) => {
            // 20 GiB = 20.0 GB
            assert!(msg.contains("20.0"), "Message should contain size '20.0' GB, got: {}", msg);
            // limit = 15 GiB = 15.0 GB
            assert!(msg.contains("15"), "Message should contain limit '15' GB, got: {}", msg);
        }
        other => panic!("Expected ZipBomb error for total size, got {:?}", other),
    }
}

// ── 11. Error message format: single entry ───────────────────────────────────
// **Validates: Requirement 3.2**

#[test]
fn test_error_message_format_entry() {
    // 10 GiB entry, limit 8 GiB
    let ten_gib: i64 = 10 * 1_073_741_824;
    let entries = vec![ArchiveEntryInfo {
        name: "huge_video.mkv".to_string(),
        is_directory: false,
        size: ten_gib,
    }];

    let thresholds = BombThresholds {
        max_single_entry_bytes: 8_589_934_592, // 8 GiB
        ..permissive_thresholds()
    };

    // Use large archive_file_size to avoid ratio trigger
    let result = check_entries(&entries, ten_gib as u64, "zip", 1, &thresholds);
    match result {
        Err(LockError::ZipBomb(msg)) => {
            assert!(msg.contains("huge_video.mkv"), "Message should contain entry name, got: {}", msg);
            // 10 GiB = 10.0 GB
            assert!(msg.contains("10.0"), "Message should contain size '10.0' GB, got: {}", msg);
            // limit = 8 GiB = 8.0 GB
            assert!(msg.contains("8"), "Message should contain limit '8' GB, got: {}", msg);
        }
        other => panic!("Expected ZipBomb error for single entry, got {:?}", other),
    }
}
