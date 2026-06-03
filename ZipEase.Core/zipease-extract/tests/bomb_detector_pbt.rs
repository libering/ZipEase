//! Property-based tests for the zip-bomb-protection feature.
//!
//! Feature: zip-bomb-protection
//! Property 1: 壓縮比超限時回傳 ZipBomb

use proptest::prelude::*;
use zipease_extract::extract::bomb_detector::{BombThresholds, check_entries};
use zipease_extract::ArchiveEntryInfo;
use zipease_shared::LockError;

// ── Helpers / Strategies ─────────────────────────────────────────────────────

/// Generate a single `ArchiveEntryInfo` with a given size range.
fn arb_entry(size_range: impl Strategy<Value = i64>) -> impl Strategy<Value = ArchiveEntryInfo> {
    (any::<bool>(), size_range, "[a-zA-Z0-9_]{1,20}\\.txt").prop_map(
        |(is_directory, size, name)| ArchiveEntryInfo {
            name,
            is_directory,
            size,
        },
    )
}

/// Generate a non-empty vec of entries where all sizes are non-negative.
/// Returns entries whose total uncompressed size is in a controllable range.
fn arb_entries_with_positive_sizes(
    count: impl Into<prop::collection::SizeRange>,
) -> impl Strategy<Value = Vec<ArchiveEntryInfo>> {
    prop::collection::vec(arb_entry(0i64..=1_000_000i64), count)
}

/// A non-exempt archive extension (not "iso").
fn arb_non_exempt_ext() -> impl Strategy<Value = String> {
    prop_oneof!["zip", "7z", "rar", "tar", "gz"]
}

// ── Property 1 — Compression ratio threshold ─────────────────────────────────
// Tag: Feature: zip-bomb-protection, Property 1: 壓縮比超限時回傳 ZipBomb
// **Validates: Requirements 1.1, 1.2, 1.4**

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// When total_uncompressed / archive_file_size > max_compression_ratio,
    /// check_entries must return Err(LockError::ZipBomb(_)).
    #[test]
    fn prop_compression_ratio_exceeds_threshold(
        // Generate a ratio threshold between 2.0 and 200.0
        max_ratio in 2.0f64..200.0f64,
        // Generate a non-zero archive file size (1..10_000 bytes)
        archive_file_size in 1u64..10_000u64,
        // Generate a multiplier that guarantees the ratio is exceeded
        overshoot in 1.01f64..5.0f64,
    ) {
        // Compute a total uncompressed size that exceeds the ratio threshold.
        let target_total = ((max_ratio * overshoot) * archive_file_size as f64).ceil() as i64;
        prop_assume!(target_total > 0);

        // Create a single entry with that size.
        let entries = vec![ArchiveEntryInfo {
            name: "bomb.bin".to_string(),
            is_directory: false,
            size: target_total,
        }];

        let thresholds = BombThresholds {
            max_compression_ratio: max_ratio,
            // Set other thresholds very high so they don't trigger.
            max_total_uncompressed_bytes: u64::MAX,
            max_single_entry_bytes: u64::MAX,
            max_nesting_depth: 100,
            exempt_formats: vec!["iso".to_string()],
        };

        let result = check_entries(&entries, archive_file_size, "zip", 1, &thresholds);
        prop_assert!(
            matches!(result, Err(LockError::ZipBomb(_))),
            "Expected ZipBomb error when ratio {:.1} exceeds threshold {:.1}, got {:?}",
            target_total as f64 / archive_file_size as f64,
            max_ratio,
            result
        );
    }

    /// When total_uncompressed / archive_file_size <= max_compression_ratio,
    /// check_entries must return Ok(()) (assuming no other checks trigger).
    #[test]
    fn prop_compression_ratio_within_threshold(
        max_ratio in 2.0f64..200.0f64,
        archive_file_size in 1u64..10_000u64,
        // Generate a fraction that keeps the ratio safely below the threshold
        fraction in 0.01f64..0.99f64,
    ) {
        // Compute a total uncompressed size that stays within the ratio threshold.
        let target_total = ((max_ratio * fraction) * archive_file_size as f64).floor() as i64;
        prop_assume!(target_total >= 0);

        let entries = vec![ArchiveEntryInfo {
            name: "safe.bin".to_string(),
            is_directory: false,
            size: target_total,
        }];

        let thresholds = BombThresholds {
            max_compression_ratio: max_ratio,
            // Set other thresholds very high so they don't trigger.
            max_total_uncompressed_bytes: u64::MAX,
            max_single_entry_bytes: u64::MAX,
            max_nesting_depth: 100,
            exempt_formats: vec!["iso".to_string()],
        };

        let result = check_entries(&entries, archive_file_size, "zip", 1, &thresholds);
        prop_assert!(
            result.is_ok(),
            "Expected Ok when ratio {:.1} is within threshold {:.1}, got {:?}",
            target_total as f64 / archive_file_size as f64,
            max_ratio,
            result
        );
    }

    /// When archive_file_size == 0, check_entries must skip the ratio check
    /// and return Ok(()) (no divide-by-zero panic).
    /// **Validates: Requirement 1.4**
    #[test]
    fn prop_zero_archive_size_skips_ratio_check(
        entries in arb_entries_with_positive_sizes(1..=5usize),
        ext in arb_non_exempt_ext(),
    ) {
        let thresholds = BombThresholds {
            max_compression_ratio: 1.0, // Very strict — would trigger if ratio were checked
            // Set other thresholds very high so they don't trigger.
            max_total_uncompressed_bytes: u64::MAX,
            max_single_entry_bytes: u64::MAX,
            max_nesting_depth: 100,
            exempt_formats: vec!["iso".to_string()],
        };

        // archive_file_size == 0 → ratio check must be skipped entirely
        let result = check_entries(&entries, 0, &ext, 1, &thresholds);
        prop_assert!(
            result.is_ok(),
            "Expected Ok when archive_file_size == 0 (ratio check skipped), got {:?}",
            result
        );
    }
}

// ── Property 2 — Total size threshold ────────────────────────────────────────
// Tag: Feature: zip-bomb-protection, Property 2: 總大小超限時回傳 ZipBomb
// **Validates: Requirements 2.1, 2.2, 2.5**

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// When the sum of all entry sizes exceeds `max_total_uncompressed_bytes`,
    /// check_entries must return Err(LockError::ZipBomb(_)).
    /// Uses an arbitrary threshold to validate custom-threshold support (Req 2.5).
    #[test]
    fn prop_total_size_exceeds_threshold(
        // Generate a total-size threshold between 1 KiB and 100 MiB
        max_total in 1_024u64..104_857_600u64,
        // Number of entries (1..=10)
        entry_count in 1usize..=10usize,
        // Overshoot factor to guarantee the sum exceeds the threshold
        overshoot in 1.01f64..3.0f64,
    ) {
        // Distribute the overshooting total evenly across entries.
        let target_total = (max_total as f64 * overshoot).ceil() as u64;
        let per_entry = (target_total / entry_count as u64).max(1);
        // Last entry absorbs the remainder so the sum is exactly target_total.
        let remainder = target_total - per_entry * (entry_count as u64 - 1);

        let mut entries: Vec<ArchiveEntryInfo> = (0..entry_count - 1)
            .map(|i| ArchiveEntryInfo {
                name: format!("file_{i}.bin"),
                is_directory: false,
                size: per_entry as i64,
            })
            .collect();
        entries.push(ArchiveEntryInfo {
            name: "file_last.bin".to_string(),
            is_directory: false,
            size: remainder as i64,
        });

        // Use a large archive_file_size so the compression ratio check doesn't trigger.
        let archive_file_size = target_total; // ratio = 1.0, well below any reasonable limit

        let thresholds = BombThresholds {
            max_total_uncompressed_bytes: max_total,
            // Set other thresholds very high so they don't trigger.
            max_compression_ratio: f64::MAX,
            max_single_entry_bytes: u64::MAX,
            max_nesting_depth: 100,
            exempt_formats: vec!["iso".to_string()],
        };

        let result = check_entries(&entries, archive_file_size, "zip", 1, &thresholds);
        prop_assert!(
            matches!(result, Err(LockError::ZipBomb(_))),
            "Expected ZipBomb error when total {} exceeds threshold {}, got {:?}",
            target_total,
            max_total,
            result
        );
    }

    /// When the sum of all entry sizes is within `max_total_uncompressed_bytes`,
    /// check_entries must return Ok(()) (assuming no other checks trigger).
    /// Uses an arbitrary threshold to validate custom-threshold support (Req 2.5).
    #[test]
    fn prop_total_size_within_threshold(
        // Generate a total-size threshold between 1 MiB and 100 MiB
        max_total in 1_048_576u64..104_857_600u64,
        // Number of entries (1..=10)
        entry_count in 1usize..=10usize,
        // Fraction to keep the sum safely below the threshold
        fraction in 0.01f64..0.99f64,
    ) {
        let target_total = (max_total as f64 * fraction).floor() as u64;
        let per_entry = (target_total / entry_count as u64).max(0);
        let remainder = target_total.saturating_sub(per_entry * (entry_count as u64 - 1));

        let mut entries: Vec<ArchiveEntryInfo> = (0..entry_count - 1)
            .map(|i| ArchiveEntryInfo {
                name: format!("safe_{i}.bin"),
                is_directory: false,
                size: per_entry as i64,
            })
            .collect();
        entries.push(ArchiveEntryInfo {
            name: "safe_last.bin".to_string(),
            is_directory: false,
            size: remainder as i64,
        });

        // Use a large archive_file_size so the compression ratio check doesn't trigger.
        let archive_file_size = max_total; // ratio ≤ 1.0

        let thresholds = BombThresholds {
            max_total_uncompressed_bytes: max_total,
            // Set other thresholds very high so they don't trigger.
            max_compression_ratio: f64::MAX,
            max_single_entry_bytes: u64::MAX,
            max_nesting_depth: 100,
            exempt_formats: vec!["iso".to_string()],
        };

        let result = check_entries(&entries, archive_file_size, "zip", 1, &thresholds);
        prop_assert!(
            result.is_ok(),
            "Expected Ok when total {} is within threshold {}, got {:?}",
            target_total,
            max_total,
            result
        );
    }
}

// ── Property 3 — Single entry size threshold ─────────────────────────────────
// Tag: Feature: zip-bomb-protection, Property 3: 單一條目超限時回傳 ZipBomb
// **Validates: Requirements 3.1, 3.2**

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// When at least one entry's `size` exceeds `max_single_entry_bytes`,
    /// check_entries must return Err(LockError::ZipBomb(_)).
    #[test]
    fn prop_single_entry_exceeds_threshold(
        // Generate a single-entry threshold between 1 KiB and 100 MiB
        max_single in 1_024u64..104_857_600u64,
        // Overshoot factor to guarantee the entry exceeds the threshold
        overshoot in 1.01f64..5.0f64,
        // Number of safe entries before the oversized one (0..=5)
        safe_count in 0usize..=5usize,
    ) {
        let oversized = (max_single as f64 * overshoot).ceil() as i64;
        prop_assume!(oversized > max_single as i64);

        // Build safe entries (each well within the threshold).
        let safe_size = (max_single / 2).max(1) as i64;
        let mut entries: Vec<ArchiveEntryInfo> = (0..safe_count)
            .map(|i| ArchiveEntryInfo {
                name: format!("safe_{i}.bin"),
                is_directory: false,
                size: safe_size,
            })
            .collect();

        // Add the oversized entry.
        entries.push(ArchiveEntryInfo {
            name: "oversized.bin".to_string(),
            is_directory: false,
            size: oversized,
        });

        // Use a large archive_file_size so the compression ratio check doesn't trigger.
        let total: u64 = entries.iter().filter(|e| e.size >= 0).map(|e| e.size as u64).sum();
        let archive_file_size = total; // ratio = 1.0

        let thresholds = BombThresholds {
            max_single_entry_bytes: max_single,
            // Set other thresholds very high so they don't trigger.
            max_compression_ratio: f64::MAX,
            max_total_uncompressed_bytes: u64::MAX,
            max_nesting_depth: 100,
            exempt_formats: vec!["iso".to_string()],
        };

        let result = check_entries(&entries, archive_file_size, "zip", 1, &thresholds);
        prop_assert!(
            matches!(result, Err(LockError::ZipBomb(_))),
            "Expected ZipBomb error when entry size {} exceeds threshold {}, got {:?}",
            oversized,
            max_single,
            result
        );
    }

    /// When all entries' sizes are within `max_single_entry_bytes`,
    /// check_entries must return Ok(()) (assuming no other checks trigger).
    #[test]
    fn prop_single_entry_within_threshold(
        // Generate a single-entry threshold between 1 KiB and 100 MiB
        max_single in 1_024u64..104_857_600u64,
        // Number of entries (1..=10)
        entry_count in 1usize..=10usize,
        // Fraction to keep each entry safely below the threshold
        fraction in 0.01f64..0.99f64,
    ) {
        let per_entry = (max_single as f64 * fraction).floor() as i64;
        prop_assume!(per_entry >= 0);

        let entries: Vec<ArchiveEntryInfo> = (0..entry_count)
            .map(|i| ArchiveEntryInfo {
                name: format!("safe_{i}.bin"),
                is_directory: false,
                size: per_entry,
            })
            .collect();

        // Use a large archive_file_size so the compression ratio check doesn't trigger.
        let total: u64 = entries.iter().filter(|e| e.size >= 0).map(|e| e.size as u64).sum();
        let archive_file_size = total.max(1); // ratio ≤ 1.0, avoid 0

        let thresholds = BombThresholds {
            max_single_entry_bytes: max_single,
            // Set other thresholds very high so they don't trigger.
            max_compression_ratio: f64::MAX,
            max_total_uncompressed_bytes: u64::MAX,
            max_nesting_depth: 100,
            exempt_formats: vec!["iso".to_string()],
        };

        let result = check_entries(&entries, archive_file_size, "zip", 1, &thresholds);
        prop_assert!(
            result.is_ok(),
            "Expected Ok when all entry sizes ({}) are within threshold {}, got {:?}",
            per_entry,
            max_single,
            result
        );
    }
}

// ── Property 4 — Nesting detection ───────────────────────────────────────────
// Tag: Feature: zip-bomb-protection, Property 4: 嵌套壓縮包偵測的正確性
// **Validates: Requirements 4.1, 4.2**

/// Known archive extensions used for nesting detection.
const KNOWN_ARCHIVE_EXTS: &[&str] = &["zip", "7z", "rar", "tar", "gz", "bz2", "xz", "cab"];

/// Strategy that generates a file name ending with a known archive extension.
fn arb_nested_archive_name() -> impl Strategy<Value = String> {
    (
        "[a-zA-Z0-9_]{1,15}",
        prop::sample::select(KNOWN_ARCHIVE_EXTS),
    )
        .prop_map(|(base, ext)| format!("{base}.{ext}"))
}

/// Strategy that generates a file name with a safe (non-archive) extension.
fn arb_safe_name() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_]{1,15}\\.(txt|bin|dat|pdf|jpg|png|doc|xml|csv|log)"
}

/// Generate entries that contain at least one nested archive entry.
fn arb_entries_with_nested_archive(
    count: impl Into<prop::collection::SizeRange>,
) -> impl Strategy<Value = Vec<ArchiveEntryInfo>> {
    prop::collection::vec(
        prop_oneof![
            // Safe entry (non-archive extension)
            arb_safe_name().prop_map(|name| ArchiveEntryInfo {
                name,
                is_directory: false,
                size: 1_000,
            }),
            // Nested archive entry
            arb_nested_archive_name().prop_map(|name| ArchiveEntryInfo {
                name,
                is_directory: false,
                size: 1_000,
            }),
        ],
        count,
    )
    .prop_filter("must contain at least one archive entry", |entries| {
        entries.iter().any(|e| {
            let lower = e.name.to_lowercase();
            KNOWN_ARCHIVE_EXTS
                .iter()
                .any(|ext| lower.ends_with(&format!(".{ext}")))
        })
    })
}

/// Generate entries with NO nested archive entries (only safe extensions).
fn arb_entries_without_nested_archive(
    count: impl Into<prop::collection::SizeRange>,
) -> impl Strategy<Value = Vec<ArchiveEntryInfo>> {
    prop::collection::vec(
        arb_safe_name().prop_map(|name| ArchiveEntryInfo {
            name,
            is_directory: false,
            size: 1_000,
        }),
        count,
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// When entries contain at least one nested archive and current_depth >= max_nesting_depth,
    /// check_entries must return Err(LockError::ZipBomb(_)).
    #[test]
    fn prop_nesting_detected_at_or_above_depth_limit(
        entries in arb_entries_with_nested_archive(1..=10usize),
        // max_nesting_depth between 1 and 5
        max_nesting in 1u32..=5u32,
        // depth_offset: 0 means exactly at limit, >0 means above limit
        depth_offset in 0u32..=3u32,
    ) {
        let current_depth = max_nesting + depth_offset; // >= max_nesting_depth

        // Use a large archive_file_size to avoid triggering the compression ratio check.
        let total: u64 = entries.iter().filter(|e| e.size >= 0).map(|e| e.size as u64).sum();
        let archive_file_size = total.max(1);

        let thresholds = BombThresholds {
            max_nesting_depth: max_nesting,
            // Set other thresholds very high so they don't trigger.
            max_compression_ratio: f64::MAX,
            max_total_uncompressed_bytes: u64::MAX,
            max_single_entry_bytes: u64::MAX,
            exempt_formats: vec!["iso".to_string()],
        };

        let result = check_entries(&entries, archive_file_size, "zip", current_depth, &thresholds);
        prop_assert!(
            matches!(result, Err(LockError::ZipBomb(_))),
            "Expected ZipBomb error when nested archive exists at depth {} (limit {}), got {:?}",
            current_depth,
            max_nesting,
            result
        );
    }

    /// When entries contain NO nested archive entries,
    /// check_entries must return Ok(()) regardless of depth.
    #[test]
    fn prop_no_nesting_detected_without_archive_entries(
        entries in arb_entries_without_nested_archive(1..=10usize),
        // Any depth, even very high
        current_depth in 1u32..=100u32,
        max_nesting in 1u32..=5u32,
    ) {
        // Use a large archive_file_size to avoid triggering the compression ratio check.
        let total: u64 = entries.iter().filter(|e| e.size >= 0).map(|e| e.size as u64).sum();
        let archive_file_size = total.max(1);

        let thresholds = BombThresholds {
            max_nesting_depth: max_nesting,
            // Set other thresholds very high so they don't trigger.
            max_compression_ratio: f64::MAX,
            max_total_uncompressed_bytes: u64::MAX,
            max_single_entry_bytes: u64::MAX,
            exempt_formats: vec!["iso".to_string()],
        };

        let result = check_entries(&entries, archive_file_size, "zip", current_depth, &thresholds);
        prop_assert!(
            result.is_ok(),
            "Expected Ok when no nested archive entries exist (depth {}, limit {}), got {:?}",
            current_depth,
            max_nesting,
            result
        );
    }
}

// ── Property 5 — Detection determinism ───────────────────────────────────────
// Tag: Feature: zip-bomb-protection, Property 5: 偵測結果的確定性（冪等性）
// **Validates: Requirements 10.1, 10.2**

/// Strategy that generates an arbitrary `BombThresholds`.
fn arb_thresholds() -> impl Strategy<Value = BombThresholds> {
    (
        2.0f64..1000.0f64,                    // max_compression_ratio
        1_024u64..107_374_182_400u64,          // max_total_uncompressed_bytes (1 KiB .. 100 GiB)
        1_024u64..107_374_182_400u64,          // max_single_entry_bytes
        1u32..=10u32,                          // max_nesting_depth
    )
        .prop_map(|(ratio, total, single, depth)| BombThresholds {
            max_compression_ratio: ratio,
            max_total_uncompressed_bytes: total,
            max_single_entry_bytes: single,
            max_nesting_depth: depth,
            exempt_formats: vec!["iso".to_string()],
        })
}

/// Strategy that generates an arbitrary entry — may have archive or non-archive extension.
fn arb_any_entry() -> impl Strategy<Value = ArchiveEntryInfo> {
    (
        any::<bool>(),
        -1i64..=50_000_000_000i64,
        prop_oneof![
            "[a-zA-Z0-9_]{1,15}\\.(txt|bin|dat|pdf|jpg|png|doc|xml|csv|log)",
            arb_nested_archive_name(),
        ],
    )
        .prop_map(|(is_directory, size, name)| ArchiveEntryInfo {
            name,
            is_directory,
            size,
        })
}

/// Strategy that generates a non-empty vec of arbitrary entries.
fn arb_any_entries(
    count: impl Into<prop::collection::SizeRange>,
) -> impl Strategy<Value = Vec<ArchiveEntryInfo>> {
    prop::collection::vec(arb_any_entry(), count)
}

/// Strategy that generates an arbitrary archive extension (may or may not be exempt).
fn arb_archive_ext() -> impl Strategy<Value = String> {
    prop_oneof!["zip", "7z", "rar", "tar", "gz", "iso", "cab", "bz2", "xz"]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// For any valid combination of inputs, calling check_entries() twice with
    /// the same arguments must return the same variant (both Ok or both Err).
    /// This validates that the detection logic is deterministic and idempotent.
    #[test]
    fn prop_detection_determinism(
        thresholds in arb_thresholds(),
        entries in arb_any_entries(1..=10usize),
        archive_file_size in 0u64..10_000_000u64,
        archive_ext in arb_archive_ext(),
        current_depth in 1u32..=10u32,
    ) {
        let result1 = check_entries(&entries, archive_file_size, &archive_ext, current_depth, &thresholds);
        let result2 = check_entries(&entries, archive_file_size, &archive_ext, current_depth, &thresholds);

        let both_ok = result1.is_ok() && result2.is_ok();
        let both_err = result1.is_err() && result2.is_err();

        prop_assert!(
            both_ok || both_err,
            "Detection must be deterministic: first call returned {:?}, second call returned {:?}",
            result1,
            result2
        );
    }
}

// ── Property 6 — BombThresholds JSON round-trip ──────────────────────────────
// Tag: Feature: zip-bomb-protection, Property 6: AppSettings ZipBombThresholds 序列化 Round-trip
// **Validates: Requirements 9.4**

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// For any valid `BombThresholds`, serializing to JSON with `serde_json`
    /// and deserializing back must produce a struct with all fields equal to
    /// the original.
    #[test]
    fn prop_bomb_thresholds_json_roundtrip(
        thresholds in arb_thresholds(),
    ) {
        let json = serde_json::to_string(&thresholds)
            .expect("BombThresholds should serialize to JSON");
        let deserialized: BombThresholds = serde_json::from_str(&json)
            .expect("BombThresholds should deserialize from JSON");

        // Compare each field individually (BombThresholds does not derive PartialEq).
        // f64: JSON serialization may lose the least-significant bit of the mantissa,
        // so we use a relative epsilon comparison instead of exact bit equality.
        let ratio_diff = (thresholds.max_compression_ratio - deserialized.max_compression_ratio).abs();
        let ratio_eps = thresholds.max_compression_ratio.abs() * f64::EPSILON * 4.0;
        prop_assert!(
            ratio_diff <= ratio_eps,
            "max_compression_ratio mismatch: {} vs {} (diff={}, eps={})",
            thresholds.max_compression_ratio,
            deserialized.max_compression_ratio,
            ratio_diff,
            ratio_eps
        );
        prop_assert_eq!(
            thresholds.max_total_uncompressed_bytes,
            deserialized.max_total_uncompressed_bytes,
            "max_total_uncompressed_bytes mismatch"
        );
        prop_assert_eq!(
            thresholds.max_single_entry_bytes,
            deserialized.max_single_entry_bytes,
            "max_single_entry_bytes mismatch"
        );
        prop_assert_eq!(
            thresholds.max_nesting_depth,
            deserialized.max_nesting_depth,
            "max_nesting_depth mismatch"
        );
        prop_assert_eq!(
            thresholds.exempt_formats,
            deserialized.exempt_formats,
            "exempt_formats mismatch"
        );
    }
}
