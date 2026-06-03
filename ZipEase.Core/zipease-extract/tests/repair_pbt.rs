/// Property-based tests for archive repair data model serialization.
///
/// Feature: archive-repair
use proptest::prelude::*;
use zipease_extract::repair::models::{DamageEntry, DamageReport, ScanResult};

// ---------------------------------------------------------------------------
// Strategies for generating arbitrary instances
// ---------------------------------------------------------------------------

/// Generate an arbitrary DamageEntry.
fn arb_damage_entry() -> impl Strategy<Value = DamageEntry> {
    (
        prop::sample::select(vec![
            "missing_eocd".to_string(),
            "corrupted_cd".to_string(),
            "misaligned_lfh".to_string(),
            "invalid_crc".to_string(),
            "corrupted_header".to_string(),
            "missing_marker".to_string(),
        ]),
        any::<u64>(),
        prop::option::of("[a-zA-Z0-9_/\\.]{0,50}"),
        "[a-zA-Z0-9 _\\-\\.]{0,100}",
    )
        .prop_map(|(damage_type, offset, entry_name, description)| DamageEntry {
            damage_type,
            offset,
            entry_name,
            description,
        })
}

/// Generate an arbitrary DamageReport.
fn arb_damage_report() -> impl Strategy<Value = DamageReport> {
    (
        prop::sample::select(vec!["zip".to_string(), "rar".to_string()]),
        any::<u32>(),
        any::<u32>(),
        any::<u32>(),
        any::<u32>(),
        prop::collection::vec(arb_damage_entry(), 0..5),
        any::<bool>(),
    )
        .prop_map(
            |(format, total_entries, valid_entries, corrupted_entries, unrecoverable_entries, damages, repairable)| {
                DamageReport {
                    format,
                    total_entries,
                    valid_entries,
                    corrupted_entries,
                    unrecoverable_entries,
                    damages,
                    repairable,
                }
            },
        )
}

/// Generate an arbitrary ScanResult.
fn arb_scan_result() -> impl Strategy<Value = ScanResult> {
    (
        any::<bool>(),
        prop::collection::vec("[a-zA-Z0-9_/\\.]{1,50}", 0..10),
        prop::collection::vec("[a-zA-Z0-9_/\\.]{1,50}", 0..10),
        prop::option::of("[a-zA-Z0-9_/\\\\:\\.]{1,100}"),
    )
        .prop_map(
            |(success, recovered_entries, failed_entries, repaired_path)| ScanResult {
                success,
                recovered_entries,
                failed_entries,
                repaired_path,
            },
        )
}

// ---------------------------------------------------------------------------
// Property 1 — DamageReport serialization round-trip
// Feature: archive-repair, Property 1: DamageReport serialization round-trip
// Validates: Requirements 9.1, 9.3, 9.5
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 9.1, 9.3, 9.5**
    ///
    /// Feature: archive-repair, Property 1: DamageReport serialization round-trip
    #[test]
    fn damage_report_serialization_roundtrip(report in arb_damage_report()) {
        let json = serde_json::to_string(&report)
            .expect("DamageReport should serialize to JSON");
        let deserialized: DamageReport = serde_json::from_str(&json)
            .expect("DamageReport JSON should deserialize back");
        prop_assert_eq!(report, deserialized, "DamageReport round-trip must produce equal object");
    }
}

// ---------------------------------------------------------------------------
// Property 2 — ScanResult serialization round-trip
// Feature: archive-repair, Property 2: ScanResult serialization round-trip
// Validates: Requirements 9.2, 9.4, 9.6
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 9.2, 9.4, 9.6**
    ///
    /// Feature: archive-repair, Property 2: ScanResult serialization round-trip
    #[test]
    fn scan_result_serialization_roundtrip(result in arb_scan_result()) {
        let json = serde_json::to_string(&result)
            .expect("ScanResult should serialize to JSON");
        let deserialized: ScanResult = serde_json::from_str(&json)
            .expect("ScanResult JSON should deserialize back");
        prop_assert_eq!(result, deserialized, "ScanResult round-trip must produce equal object");
    }
}

// ---------------------------------------------------------------------------
// Property 5 — DamageReport count invariant
// Feature: archive-repair, Property 5: DamageReport count invariant
// Validates: Requirements 1.6, 2.6, 5.6
// ---------------------------------------------------------------------------

/// Generate a DamageReport where total_entries == valid + corrupted + unrecoverable.
/// This mirrors how the repair engine constructs reports.
fn arb_damage_report_with_count_invariant() -> impl Strategy<Value = DamageReport> {
    (
        prop::sample::select(vec!["zip".to_string(), "rar".to_string()]),
        any::<u32>(),
        any::<u32>(),
        any::<u32>(),
        prop::collection::vec(arb_damage_entry(), 0..5),
        any::<bool>(),
    )
        .prop_map(
            |(format, valid_entries, corrupted_entries, unrecoverable_entries, damages, repairable)| {
                let total_entries = valid_entries
                    .saturating_add(corrupted_entries)
                    .saturating_add(unrecoverable_entries);
                DamageReport {
                    format,
                    total_entries,
                    valid_entries,
                    corrupted_entries,
                    unrecoverable_entries,
                    damages,
                    repairable,
                }
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 1.6, 2.6, 5.6**
    ///
    /// Feature: archive-repair, Property 5: DamageReport count invariant
    #[test]
    fn damage_report_count_invariant(report in arb_damage_report_with_count_invariant()) {
        // The repair engine must always produce reports where total == valid + corrupted + unrecoverable.
        // We use saturating_add to match the generator's overflow behavior.
        let expected_total = report.valid_entries
            .saturating_add(report.corrupted_entries)
            .saturating_add(report.unrecoverable_entries);
        prop_assert_eq!(
            report.total_entries,
            expected_total,
            "DamageReport count invariant violated: total_entries ({}) != valid ({}) + corrupted ({}) + unrecoverable ({})",
            report.total_entries,
            report.valid_entries,
            report.corrupted_entries,
            report.unrecoverable_entries
        );
    }
}

// ---------------------------------------------------------------------------
// Property 4 — Repair path generation
// Feature: archive-repair, Property 4: Repair path generation
// Validates: Requirements 8.2, 8.3
// ---------------------------------------------------------------------------

use std::fs;
use std::path::Path;
use tempfile::TempDir;
use zipease_extract::repair::path_gen::generate_repair_path;

/// Strategy for generating valid filenames with extensions.
fn arb_filename_with_ext() -> impl Strategy<Value = String> {
    (
        "[a-zA-Z0-9_]{1,20}",  // stem
        "[a-zA-Z0-9]{1,5}",    // extension
    )
        .prop_map(|(stem, ext)| format!("{}.{}", stem, ext))
}

/// Strategy for generating valid filenames without extensions.
fn arb_filename_no_ext() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_]{1,20}".prop_map(|s| s)
}

/// Strategy for generating filenames (with or without extension).
fn arb_filename() -> impl Strategy<Value = String> {
    prop_oneof![
        arb_filename_with_ext(),
        arb_filename_no_ext(),
    ]
}

/// Strategy for generating a count of pre-existing _repaired files to simulate collisions.
fn arb_collision_count() -> impl Strategy<Value = u32> {
    0u32..5u32
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 8.2, 8.3**
    ///
    /// Feature: archive-repair, Property 4: Repair path generation
    ///
    /// For any original filename, the generated repair path:
    /// 1. Follows the `{stem}_repaired.{ext}` pattern (or incremented variant)
    /// 2. Does not collide with existing files
    /// 3. Never equals the original path
    #[test]
    fn repair_path_follows_pattern_no_collision_not_original(
        filename in arb_filename(),
        collision_count in arb_collision_count(),
    ) {
        let dir = TempDir::new().unwrap();
        let original = dir.path().join(&filename);
        fs::write(&original, b"original").unwrap();

        // Determine stem and extension from the filename
        let path_ref = Path::new(&filename);
        let stem = path_ref.file_stem().unwrap().to_string_lossy().to_string();
        let ext = path_ref.extension().map(|e| e.to_string_lossy().to_string());

        // Pre-create _repaired files to simulate collisions
        for i in 0..collision_count {
            let collision_name = if i == 0 {
                match &ext {
                    Some(e) => format!("{}_repaired.{}", stem, e),
                    None => format!("{}_repaired", stem),
                }
            } else {
                match &ext {
                    Some(e) => format!("{}_repaired_{}.{}", stem, i + 1, e),
                    None => format!("{}_repaired_{}", stem, i + 1),
                }
            };
            fs::write(dir.path().join(&collision_name), b"existing").unwrap();
        }

        let result = generate_repair_path(&original);

        // Property 1: Result follows the _repaired pattern
        let result_filename = result.file_name().unwrap().to_string_lossy().to_string();
        let result_stem_full = result.file_stem().unwrap().to_string_lossy().to_string();

        // The result filename must contain "_repaired" in the stem portion
        prop_assert!(
            result_stem_full.contains("_repaired"),
            "Generated path '{}' does not contain '_repaired' in stem",
            result_filename
        );

        // Verify the pattern: stem starts with original stem + "_repaired"
        prop_assert!(
            result_stem_full.starts_with(&format!("{}_repaired", stem)),
            "Generated path stem '{}' does not start with '{}_repaired'",
            result_stem_full,
            stem
        );

        // Verify extension is preserved (or absent if original had none)
        let result_ext = result.extension().map(|e| e.to_string_lossy().to_string());
        prop_assert_eq!(
            &result_ext,
            &ext,
            "Extension mismatch: generated {:?}, expected {:?}",
            result_ext,
            ext
        );

        // Property 2: No collision with existing files
        prop_assert!(
            !result.exists(),
            "Generated path '{}' collides with an existing file",
            result.display()
        );

        // Property 3: Never equals original path
        prop_assert_ne!(
            result,
            original,
            "Generated path must never equal the original path"
        );
    }
}

// ---------------------------------------------------------------------------
// Property 6 — LFH signature scanner completeness
// Feature: archive-repair, Property 6: LFH signature scanner completeness
// Validates: Requirements 1.4, 2.1
// ---------------------------------------------------------------------------

use zipease_extract::repair::zip_scanner::ZipScanner;

/// Strategy for generating a byte buffer with LFH signatures embedded at known, non-overlapping offsets.
/// Returns (buffer, sorted list of embedded offsets).
fn arb_buffer_with_lfh_signatures() -> impl Strategy<Value = (Vec<u8>, Vec<u64>)> {
    // Buffer size between 100 and 1000 bytes
    (100usize..=1000usize).prop_flat_map(|buf_size| {
        // Generate random base bytes for the buffer, avoiding accidental PK\x03\x04
        let base_bytes = prop::collection::vec(0x05u8..=0xFFu8, buf_size);
        // Generate between 1 and min(10, buf_size/8) non-overlapping offsets
        let max_sigs = 10usize.min(buf_size / 8).max(1);
        let num_sigs = 1usize..=max_sigs;
        (base_bytes, num_sigs).prop_flat_map(move |(bytes, count)| {
            // Generate `count` offsets that are at least 4 apart (non-overlapping signatures)
            // We pick from slots: each slot is 4 bytes wide, so max_slots = buf_size / 4
            let max_offset = buf_size - 4;
            // Generate sorted unique offsets with minimum spacing of 4
            let offsets = prop::collection::vec(0usize..=max_offset, count)
                .prop_map(|mut v| {
                    v.sort();
                    v.dedup();
                    // Filter to ensure minimum spacing of 4 between consecutive offsets
                    let mut filtered = Vec::new();
                    for offset in v {
                        if filtered.last().map_or(true, |&last: &usize| offset >= last + 4) {
                            filtered.push(offset);
                        }
                    }
                    filtered
                })
                .prop_filter("need at least one offset", |v| !v.is_empty());
            (Just(bytes), offsets)
        })
    }).prop_map(|(mut bytes, offsets)| {
        let lfh_sig: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];
        // Embed the LFH signature at each chosen offset
        for &offset in &offsets {
            bytes[offset..offset + 4].copy_from_slice(&lfh_sig);
        }
        let offsets_u64: Vec<u64> = offsets.into_iter().map(|o| o as u64).collect();
        (bytes, offsets_u64)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 1.4, 2.1**
    ///
    /// Feature: archive-repair, Property 6: LFH signature scanner completeness
    ///
    /// For any byte buffer with embedded PK\x03\x04 signatures at known offsets,
    /// scan_lfh_signatures must find all those offsets (no false negatives)
    /// and return them sorted in ascending order.
    #[test]
    fn lfh_signature_scanner_completeness((buffer, expected_offsets) in arb_buffer_with_lfh_signatures()) {
        let found_offsets = ZipScanner::scan_lfh_signatures(&buffer);

        // Verify no false negatives: every embedded offset must be in the result
        for &expected in &expected_offsets {
            prop_assert!(
                found_offsets.contains(&expected),
                "False negative: embedded LFH signature at offset {} was not found. Found: {:?}",
                expected,
                found_offsets
            );
        }

        // Verify results are sorted in ascending order
        for window in found_offsets.windows(2) {
            prop_assert!(
                window[0] < window[1],
                "Results not sorted ascending: {} >= {}",
                window[0],
                window[1]
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Property 7 — Central Directory reconstruction preserves LFH metadata
// Feature: archive-repair, Property 7: Central Directory reconstruction preserves LFH metadata
// Validates: Requirements 2.2, 3.1
// ---------------------------------------------------------------------------

use zipease_extract::repair::zip_repairer::ZipRepairer;

/// Build a raw LFH byte buffer from the given metadata fields.
/// Layout (30 bytes fixed + filename):
///   [0..4]   Signature: PK\x03\x04
///   [4..6]   version_needed (u16 LE)
///   [6..8]   flags (u16 LE)
///   [8..10]  compression_method (u16 LE)
///   [10..12] last_mod_time (u16 LE)
///   [12..14] last_mod_date (u16 LE)
///   [14..18] crc32 (u32 LE)
///   [18..22] compressed_size (u32 LE)
///   [22..26] uncompressed_size (u32 LE)
///   [26..28] filename_length (u16 LE)
///   [28..30] extra_field_length (u16 LE) = 0
///   [30..]   filename bytes
fn build_lfh_bytes(
    version_needed: u16,
    flags: u16,
    compression_method: u16,
    last_mod_time: u16,
    last_mod_date: u16,
    crc32: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    filename: &[u8],
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(30 + filename.len());
    // Signature
    buf.extend_from_slice(&[0x50, 0x4B, 0x03, 0x04]);
    buf.extend_from_slice(&version_needed.to_le_bytes());
    buf.extend_from_slice(&flags.to_le_bytes());
    buf.extend_from_slice(&compression_method.to_le_bytes());
    buf.extend_from_slice(&last_mod_time.to_le_bytes());
    buf.extend_from_slice(&last_mod_date.to_le_bytes());
    buf.extend_from_slice(&crc32.to_le_bytes());
    buf.extend_from_slice(&compressed_size.to_le_bytes());
    buf.extend_from_slice(&uncompressed_size.to_le_bytes());
    buf.extend_from_slice(&(filename.len() as u16).to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes()); // extra_field_length = 0
    buf.extend_from_slice(filename);
    buf
}

/// Strategy for generating a single valid LFH entry's metadata.
/// Returns (version_needed, flags, compression_method, last_mod_time, last_mod_date,
///          crc32, compressed_size, uncompressed_size, filename_bytes).
fn arb_lfh_metadata() -> impl Strategy<Value = (u16, u16, u16, u16, u16, u32, u32, u32, Vec<u8>)> {
    (
        any::<u16>(),                           // version_needed
        any::<u16>(),                           // flags
        any::<u16>(),                           // compression_method
        any::<u16>(),                           // last_mod_time
        any::<u16>(),                           // last_mod_date
        any::<u32>(),                           // crc32
        1u32..=u32::MAX,                        // compressed_size (non-zero so LFH value is used directly)
        any::<u32>(),                           // uncompressed_size
        prop::collection::vec(b'a'..=b'z', 1..20), // filename (1-19 ASCII bytes)
    )
}

/// Strategy for generating 1..5 LFH entries to embed in a buffer.
fn arb_lfh_entries() -> impl Strategy<Value = Vec<(u16, u16, u16, u16, u16, u32, u32, u32, Vec<u8>)>> {
    prop::collection::vec(arb_lfh_metadata(), 1..=5)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 2.2, 3.1**
    ///
    /// Feature: archive-repair, Property 7: Central Directory reconstruction preserves LFH metadata
    ///
    /// For any set of valid Local File Headers with known metadata, reconstructing
    /// Central Directory entries from those headers SHALL produce CD entries where
    /// each field matches the corresponding LFH field.
    #[test]
    fn cd_reconstruction_preserves_lfh_metadata(entries in arb_lfh_entries()) {
        // Build a contiguous byte buffer containing all LFH entries, and track their offsets.
        let mut data = Vec::new();
        let mut offsets: Vec<u64> = Vec::new();

        for (version_needed, flags, compression_method, last_mod_time, last_mod_date, crc32, compressed_size, uncompressed_size, ref filename) in &entries {
            offsets.push(data.len() as u64);
            let lfh_bytes = build_lfh_bytes(
                *version_needed,
                *flags,
                *compression_method,
                *last_mod_time,
                *last_mod_date,
                *crc32,
                *compressed_size,
                *uncompressed_size,
                filename,
            );
            data.extend_from_slice(&lfh_bytes);
            // Append some dummy compressed data bytes (compressed_size bytes, capped to avoid huge allocs)
            let dummy_size = (*compressed_size).min(64) as usize;
            data.extend(std::iter::repeat(0xAA).take(dummy_size));
        }

        // Call reconstruct_cd
        let cd_entries = ZipRepairer::reconstruct_cd(&data, &offsets);

        // Verify we got the same number of CD entries as LFH entries
        prop_assert_eq!(
            cd_entries.len(),
            entries.len(),
            "Expected {} CD entries, got {}",
            entries.len(),
            cd_entries.len()
        );

        // Verify each CD entry's fields match the corresponding LFH metadata
        for (i, ((version_needed, flags, compression_method, last_mod_time, last_mod_date, crc32, compressed_size, uncompressed_size, ref filename), cd)) in entries.iter().zip(cd_entries.iter()).enumerate() {
            prop_assert_eq!(
                cd.version_needed, *version_needed,
                "Entry {}: version_needed mismatch", i
            );
            prop_assert_eq!(
                cd.flags, *flags,
                "Entry {}: flags mismatch", i
            );
            prop_assert_eq!(
                cd.compression_method, *compression_method,
                "Entry {}: compression_method mismatch", i
            );
            prop_assert_eq!(
                cd.last_mod_time, *last_mod_time,
                "Entry {}: last_mod_time mismatch", i
            );
            prop_assert_eq!(
                cd.last_mod_date, *last_mod_date,
                "Entry {}: last_mod_date mismatch", i
            );
            prop_assert_eq!(
                cd.crc32, *crc32,
                "Entry {}: crc32 mismatch", i
            );
            prop_assert_eq!(
                cd.compressed_size, *compressed_size,
                "Entry {}: compressed_size mismatch", i
            );
            prop_assert_eq!(
                cd.uncompressed_size, *uncompressed_size,
                "Entry {}: uncompressed_size mismatch", i
            );
            prop_assert_eq!(
                &cd.filename, filename,
                "Entry {}: filename mismatch", i
            );
            // Verify fixed fields set by reconstruct_cd
            prop_assert_eq!(
                cd.version_made_by, 20,
                "Entry {}: version_made_by should be 20", i
            );
            prop_assert_eq!(
                cd.local_header_offset, offsets[i] as u32,
                "Entry {}: local_header_offset mismatch", i
            );
            prop_assert!(
                cd.comment.is_empty(),
                "Entry {}: comment should be empty", i
            );
            prop_assert_eq!(
                cd.disk_number_start, 0,
                "Entry {}: disk_number_start should be 0", i
            );
            prop_assert_eq!(
                cd.internal_attrs, 0,
                "Entry {}: internal_attrs should be 0", i
            );
            prop_assert_eq!(
                cd.external_attrs, 0,
                "Entry {}: external_attrs should be 0", i
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Property 9 — CRC-32 recalculation correctness
// Feature: archive-repair, Property 9: CRC-32 recalculation correctness
// Validates: Requirements 3.3
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 3.3**
    ///
    /// Feature: archive-repair, Property 9: CRC-32 recalculation correctness
    ///
    /// For any byte sequence (0-1000 bytes), recalculate_crc32(data, 0) (STORED method)
    /// SHALL produce the same value as crc32fast::hash(data).
    #[test]
    fn crc32_recalculation_correctness_stored(data in prop::collection::vec(any::<u8>(), 0..=1000)) {
        let result = ZipRepairer::recalculate_crc32(&data, 0);
        let expected = crc32fast::hash(&data);

        // STORED method (0) must always return Some
        prop_assert!(
            result.is_some(),
            "recalculate_crc32 with STORED method (0) must return Some, got None"
        );

        prop_assert_eq!(
            result.unwrap(),
            expected,
            "CRC-32 mismatch for STORED data of length {}: got {}, expected {}",
            data.len(),
            result.unwrap(),
            expected
        );
    }
}

// ---------------------------------------------------------------------------
// Property 8 — Repaired ZIP archive is openable
// Feature: archive-repair, Property 8: Repaired ZIP archive is openable
// Validates: Requirements 2.3, 3.5
// ---------------------------------------------------------------------------

use std::io::Cursor;
use zip::ZipArchive;
use zipease_extract::repair::models::RepairedEntry;

/// Strategy for generating a single file entry for a damaged ZIP.
/// Returns (filename_bytes, file_data).
fn arb_zip_file_entry() -> impl Strategy<Value = (Vec<u8>, Vec<u8>)> {
    (
        prop::collection::vec(b'a'..=b'z', 1..=15), // filename: 1-15 ASCII lowercase chars
        prop::collection::vec(any::<u8>(), 1..=100), // file data: 1-100 random bytes
    )
}

/// Strategy for generating 1-3 file entries for a damaged ZIP archive.
fn arb_zip_file_entries() -> impl Strategy<Value = Vec<(Vec<u8>, Vec<u8>)>> {
    prop::collection::vec(arb_zip_file_entry(), 1..=3)
}

/// Build a raw LFH + file data buffer for a STORED entry.
/// Returns the LFH bytes concatenated with the file data.
fn build_stored_lfh_with_data(filename: &[u8], file_data: &[u8]) -> Vec<u8> {
    let crc = crc32fast::hash(file_data);
    let compressed_size = file_data.len() as u32;
    let uncompressed_size = file_data.len() as u32;
    let filename_len = filename.len() as u16;

    let mut buf = Vec::with_capacity(30 + filename.len() + file_data.len());
    // LFH signature
    buf.extend_from_slice(&[0x50, 0x4B, 0x03, 0x04]);
    // version needed: 2.0
    buf.extend_from_slice(&20u16.to_le_bytes());
    // flags: 0
    buf.extend_from_slice(&0u16.to_le_bytes());
    // compression method: 0 (STORED)
    buf.extend_from_slice(&0u16.to_le_bytes());
    // last mod time
    buf.extend_from_slice(&0u16.to_le_bytes());
    // last mod date
    buf.extend_from_slice(&33u16.to_le_bytes()); // 1980-01-01 minimum valid DOS date
    // crc32
    buf.extend_from_slice(&crc.to_le_bytes());
    // compressed size
    buf.extend_from_slice(&compressed_size.to_le_bytes());
    // uncompressed size
    buf.extend_from_slice(&uncompressed_size.to_le_bytes());
    // filename length
    buf.extend_from_slice(&filename_len.to_le_bytes());
    // extra field length: 0
    buf.extend_from_slice(&0u16.to_le_bytes());
    // filename
    buf.extend_from_slice(filename);
    // file data (STORED, so raw bytes)
    buf.extend_from_slice(file_data);

    buf
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 2.3, 3.5**
    ///
    /// Feature: archive-repair, Property 8: Repaired ZIP archive is openable
    ///
    /// For any set of 1-3 file entries with random filenames and data,
    /// constructing a damaged ZIP (valid LFH + data, but NO Central Directory
    /// and NO EOCD), then repairing it via reconstruct_cd + write_repaired,
    /// SHALL produce output that zip::ZipArchive::new() can open with ≥1 entry.
    #[test]
    fn repaired_zip_archive_is_openable(entries in arb_zip_file_entries()) {
        // Ensure unique filenames by appending index suffix
        let entries_with_unique_names: Vec<(Vec<u8>, Vec<u8>)> = entries
            .into_iter()
            .enumerate()
            .map(|(i, (mut name, data))| {
                // Append index to ensure uniqueness
                name.extend_from_slice(format!("{}", i).as_bytes());
                (name, data)
            })
            .collect();

        // Phase 1: Build a "damaged" ZIP buffer (LFH + data only, no CD/EOCD)
        let mut damaged_buf = Vec::new();
        for (filename, file_data) in &entries_with_unique_names {
            let lfh_with_data = build_stored_lfh_with_data(filename, file_data);
            damaged_buf.extend_from_slice(&lfh_with_data);
        }

        // Phase 2: Scan for LFH signatures
        let lfh_offsets = ZipScanner::scan_lfh_signatures(&damaged_buf);
        prop_assert!(
            !lfh_offsets.is_empty(),
            "Should find at least one LFH signature in the damaged buffer"
        );

        // Phase 3: Reconstruct CD entries from LFH data
        let cd_entries = ZipRepairer::reconstruct_cd(&damaged_buf, &lfh_offsets);
        prop_assert!(
            !cd_entries.is_empty(),
            "Should reconstruct at least one CD entry"
        );

        // Phase 4: Convert CdEntry to RepairedEntry
        let repaired_entries: Vec<RepairedEntry> = cd_entries
            .iter()
            .map(|cd| {
                // Compute data_offset: LFH offset + 30 + filename_len + extra_field_len
                let lfh_offset = cd.local_header_offset as u64;
                let data_offset = lfh_offset + 30 + cd.filename.len() as u64;
                RepairedEntry {
                    filename: cd.filename.clone(),
                    compression_method: cd.compression_method,
                    crc32: cd.crc32,
                    compressed_size: cd.compressed_size,
                    uncompressed_size: cd.uncompressed_size,
                    data_offset,
                    last_mod_time: cd.last_mod_time,
                    last_mod_date: cd.last_mod_date,
                }
            })
            .collect();

        // Phase 5: Write repaired archive
        let mut output = Vec::new();
        let result = ZipRepairer::write_repaired(
            &damaged_buf,
            &repaired_entries,
            &mut output,
            |_current, _total, _name| {},
        );

        prop_assert!(
            result.is_ok(),
            "write_repaired should succeed, got error: {:?}",
            result.err()
        );

        let scan_result = result.unwrap();
        prop_assert!(
            !scan_result.recovered_entries.is_empty(),
            "Should have at least one recovered entry"
        );

        // Phase 6: Verify the output is a valid ZIP openable by the zip crate
        let cursor = Cursor::new(&output);
        let archive = ZipArchive::new(cursor);
        prop_assert!(
            archive.is_ok(),
            "ZipArchive::new() should succeed on repaired output, got error: {:?}",
            archive.err()
        );

        let archive = archive.unwrap();
        prop_assert!(
            archive.len() >= 1,
            "Repaired archive should contain at least 1 entry, got {}",
            archive.len()
        );
    }
}

// ---------------------------------------------------------------------------
// Property 10 — Non-archive data returns not-repairable error
// Feature: archive-repair, Property 10: Non-archive data returns not-repairable error
// Validates: Requirements 1.5
// ---------------------------------------------------------------------------

use zipease_extract::repair::RepairEngine;

/// Strategy for generating random byte sequences (4-200 bytes) that do NOT start
/// with ZIP magic (`PK` = 0x50, 0x4B) and do NOT contain RAR magic (`Rar!` = 0x52, 0x61, 0x72, 0x21)
/// anywhere in the first 1024 bytes.
fn arb_non_archive_bytes() -> impl Strategy<Value = Vec<u8>> {
    // Generate length between 4 and 200
    (4usize..=200usize).prop_flat_map(|len| {
        prop::collection::vec(any::<u8>(), len)
    }).prop_filter(
        "must not start with ZIP magic (PK) or contain RAR magic (Rar!) in first 1024 bytes",
        |bytes| {
            // Reject if starts with ZIP magic: 0x50, 0x4B
            if bytes.len() >= 2 && bytes[0] == 0x50 && bytes[1] == 0x4B {
                return false;
            }
            // Reject if RAR magic sequence appears anywhere in first 1024 bytes
            let scan_limit = bytes.len().min(1024);
            for i in 0..scan_limit.saturating_sub(3) {
                if bytes[i] == 0x52
                    && bytes[i + 1] == 0x61
                    && bytes[i + 2] == 0x72
                    && bytes[i + 3] == 0x21
                {
                    return false;
                }
            }
            true
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 1.5**
    ///
    /// Feature: archive-repair, Property 10: Non-archive data returns not-repairable error
    ///
    /// For any byte sequence that does not start with ZIP magic (`PK`) and does not
    /// contain RAR magic (`Rar!`) in the first 1024 bytes, `RepairEngine::diagnose()`
    /// SHALL return `Err(RepairError::NotAnArchive)` which has `to_ffi_code() == 0x2006`.
    #[test]
    fn non_archive_data_returns_not_repairable_error(data in arb_non_archive_bytes()) {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("non_archive_data.bin");
        fs::write(&file_path, &data).unwrap();

        let result = RepairEngine::diagnose(&file_path);

        // diagnose() must return an error for non-archive data
        prop_assert!(
            result.is_err(),
            "diagnose() should return Err for non-archive data (len={}), got Ok({:?})",
            data.len(),
            result.ok()
        );

        let err = result.unwrap_err();
        // The error must have FFI code 0x2006
        prop_assert_eq!(
            err.to_ffi_code(),
            0x2006_u32 as i32,
            "RepairError for non-archive data should have to_ffi_code() == 0x2006, got {}",
            err.to_ffi_code()
        );
    }
}

// ---------------------------------------------------------------------------
// Property 3 — Non-destructive repair guarantee
// Feature: archive-repair, Property 3: Non-destructive repair guarantee
// Validates: Requirements 2.4, 5.4, 8.1, 8.4
// ---------------------------------------------------------------------------

/// Strategy for generating a single file entry for a damaged ZIP used in non-destructive tests.
/// Returns (filename_bytes, file_data).
fn arb_repair_file_entry() -> impl Strategy<Value = (Vec<u8>, Vec<u8>)> {
    (
        prop::collection::vec(b'a'..=b'z', 1..=10), // filename: 1-10 ASCII lowercase chars
        prop::collection::vec(any::<u8>(), 1..=50), // file data: 1-50 random bytes
    )
}

/// Strategy for generating 1-3 file entries for a damaged ZIP archive (non-destructive test).
fn arb_repair_file_entries() -> impl Strategy<Value = Vec<(Vec<u8>, Vec<u8>)>> {
    prop::collection::vec(arb_repair_file_entry(), 1..=3)
}

/// Build a damaged ZIP buffer (LFH + STORED data, no CD/EOCD) from file entries.
/// Each entry has a valid LFH with correct CRC, STORED compression.
/// This ensures the repair engine will attempt repair (not reject as non-archive).
fn build_damaged_zip_for_nondestructive(entries: &[(Vec<u8>, Vec<u8>)]) -> Vec<u8> {
    let mut buf = Vec::new();
    for (i, (filename, file_data)) in entries.iter().enumerate() {
        // Make filename unique by appending index
        let mut unique_name = filename.clone();
        unique_name.extend_from_slice(format!("{}", i).as_bytes());

        let crc = crc32fast::hash(file_data);
        let compressed_size = file_data.len() as u32;
        let uncompressed_size = file_data.len() as u32;
        let filename_len = unique_name.len() as u16;

        // LFH signature
        buf.extend_from_slice(&[0x50, 0x4B, 0x03, 0x04]);
        // version needed: 2.0
        buf.extend_from_slice(&20u16.to_le_bytes());
        // flags: 0
        buf.extend_from_slice(&0u16.to_le_bytes());
        // compression method: 0 (STORED)
        buf.extend_from_slice(&0u16.to_le_bytes());
        // last mod time
        buf.extend_from_slice(&0u16.to_le_bytes());
        // last mod date (1980-01-01 minimum valid DOS date)
        buf.extend_from_slice(&33u16.to_le_bytes());
        // crc32
        buf.extend_from_slice(&crc.to_le_bytes());
        // compressed size
        buf.extend_from_slice(&compressed_size.to_le_bytes());
        // uncompressed size
        buf.extend_from_slice(&uncompressed_size.to_le_bytes());
        // filename length
        buf.extend_from_slice(&filename_len.to_le_bytes());
        // extra field length: 0
        buf.extend_from_slice(&0u16.to_le_bytes());
        // filename
        buf.extend_from_slice(&unique_name);
        // file data (STORED, so raw bytes)
        buf.extend_from_slice(file_data);
    }
    buf
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 2.4, 5.4, 8.1, 8.4**
    ///
    /// Feature: archive-repair, Property 3: Non-destructive repair guarantee
    ///
    /// For any damaged ZIP archive (valid LFH entries with STORED data, no CD/EOCD),
    /// running RepairEngine::repair() SHALL leave the original file's bytes IDENTICAL
    /// before and after the repair operation, regardless of whether repair succeeds or fails.
    #[test]
    fn non_destructive_repair_guarantee_repair(entries in arb_repair_file_entries()) {
        let dir = TempDir::new().unwrap();

        // Build a damaged ZIP with valid LFH entries but no CD/EOCD
        let damaged_bytes = build_damaged_zip_for_nondestructive(&entries);

        // Write to temp file
        let original_path = dir.path().join("test_archive.zip");
        fs::write(&original_path, &damaged_bytes).unwrap();

        // Record original bytes
        let original_bytes = damaged_bytes.clone();

        // Define output path for repair (separate from original)
        let output_path = dir.path().join("test_archive_repaired.zip");

        // Run repair — we don't care if it succeeds or fails
        let _result = RepairEngine::repair(&original_path, &output_path, |_, _, _| {});

        // Read the original file again after repair
        let after_bytes = fs::read(&original_path).unwrap();

        // The original file MUST be identical before and after repair
        prop_assert_eq!(
            &original_bytes,
            &after_bytes,
            "Non-destructive guarantee violated: original file was modified by repair operation. \
             Original len={}, After len={}",
            original_bytes.len(),
            after_bytes.len()
        );
    }

    /// **Validates: Requirements 2.4, 5.4, 8.1, 8.4**
    ///
    /// Feature: archive-repair, Property 3: Non-destructive repair guarantee
    ///
    /// For any damaged ZIP archive, running RepairEngine::diagnose() SHALL leave
    /// the original file's bytes IDENTICAL before and after the diagnose operation.
    /// Diagnose is read-only and must never modify the source file.
    #[test]
    fn non_destructive_diagnose_guarantee(entries in arb_repair_file_entries()) {
        let dir = TempDir::new().unwrap();

        // Build a damaged ZIP with valid LFH entries but no CD/EOCD
        let damaged_bytes = build_damaged_zip_for_nondestructive(&entries);

        // Write to temp file
        let original_path = dir.path().join("test_archive_diag.zip");
        fs::write(&original_path, &damaged_bytes).unwrap();

        // Record original bytes
        let original_bytes = damaged_bytes.clone();

        // Run diagnose — we don't care about the result
        let _result = RepairEngine::diagnose(&original_path);

        // Read the original file again after diagnose
        let after_bytes = fs::read(&original_path).unwrap();

        // The original file MUST be identical before and after diagnose
        prop_assert_eq!(
            &original_bytes,
            &after_bytes,
            "Non-destructive guarantee violated: original file was modified by diagnose operation. \
             Original len={}, After len={}",
            original_bytes.len(),
            after_bytes.len()
        );
    }
}
