//! Unit tests for the ZIP scanner module.
//!
//! Feature: archive-repair
//! Validates: Requirements 1.1, 1.2, 1.3, 1.4, 1.5

use zipease_extract::repair::zip_scanner::ZipScanner;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Build a minimal valid Local File Header (30 bytes fixed + filename).
/// Uses STORED compression (method 0), no extra field.
fn build_lfh(filename: &[u8], crc32: u32, compressed_size: u32, uncompressed_size: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    // Signature: PK\x03\x04
    buf.extend_from_slice(&[0x50, 0x4B, 0x03, 0x04]);
    // Version needed: 20
    buf.extend_from_slice(&20u16.to_le_bytes());
    // Flags: 0
    buf.extend_from_slice(&0u16.to_le_bytes());
    // Compression method: 0 (stored)
    buf.extend_from_slice(&0u16.to_le_bytes());
    // Last mod time: 0
    buf.extend_from_slice(&0u16.to_le_bytes());
    // Last mod date: 0
    buf.extend_from_slice(&0u16.to_le_bytes());
    // CRC-32
    buf.extend_from_slice(&crc32.to_le_bytes());
    // Compressed size
    buf.extend_from_slice(&compressed_size.to_le_bytes());
    // Uncompressed size
    buf.extend_from_slice(&uncompressed_size.to_le_bytes());
    // Filename length
    buf.extend_from_slice(&(filename.len() as u16).to_le_bytes());
    // Extra field length: 0
    buf.extend_from_slice(&0u16.to_le_bytes());
    // Filename
    buf.extend_from_slice(filename);
    buf
}

/// Build a minimal Central Directory entry (46 bytes fixed + filename).
fn build_cd_entry(filename: &[u8], crc32: u32, compressed_size: u32, uncompressed_size: u32, local_header_offset: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    // Signature: PK\x01\x02
    buf.extend_from_slice(&[0x50, 0x4B, 0x01, 0x02]);
    // Version made by: 20
    buf.extend_from_slice(&20u16.to_le_bytes());
    // Version needed: 20
    buf.extend_from_slice(&20u16.to_le_bytes());
    // Flags: 0
    buf.extend_from_slice(&0u16.to_le_bytes());
    // Compression method: 0 (stored)
    buf.extend_from_slice(&0u16.to_le_bytes());
    // Last mod time: 0
    buf.extend_from_slice(&0u16.to_le_bytes());
    // Last mod date: 0
    buf.extend_from_slice(&0u16.to_le_bytes());
    // CRC-32
    buf.extend_from_slice(&crc32.to_le_bytes());
    // Compressed size
    buf.extend_from_slice(&compressed_size.to_le_bytes());
    // Uncompressed size
    buf.extend_from_slice(&uncompressed_size.to_le_bytes());
    // Filename length
    buf.extend_from_slice(&(filename.len() as u16).to_le_bytes());
    // Extra field length: 0
    buf.extend_from_slice(&0u16.to_le_bytes());
    // Comment length: 0
    buf.extend_from_slice(&0u16.to_le_bytes());
    // Disk number start: 0
    buf.extend_from_slice(&0u16.to_le_bytes());
    // Internal attrs: 0
    buf.extend_from_slice(&0u16.to_le_bytes());
    // External attrs: 0
    buf.extend_from_slice(&0u32.to_le_bytes());
    // Local header offset
    buf.extend_from_slice(&local_header_offset.to_le_bytes());
    // Filename
    buf.extend_from_slice(filename);
    buf
}

/// Build a minimal EOCD record (22 bytes + optional comment).
fn build_eocd(cd_entries: u16, cd_size: u32, cd_offset: u32, comment: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    // Signature: PK\x05\x06
    buf.extend_from_slice(&[0x50, 0x4B, 0x05, 0x06]);
    // Disk number: 0
    buf.extend_from_slice(&0u16.to_le_bytes());
    // CD start disk: 0
    buf.extend_from_slice(&0u16.to_le_bytes());
    // CD entries on this disk
    buf.extend_from_slice(&cd_entries.to_le_bytes());
    // CD entries total
    buf.extend_from_slice(&cd_entries.to_le_bytes());
    // CD size
    buf.extend_from_slice(&cd_size.to_le_bytes());
    // CD offset
    buf.extend_from_slice(&cd_offset.to_le_bytes());
    // Comment length
    buf.extend_from_slice(&(comment.len() as u16).to_le_bytes());
    // Comment
    buf.extend_from_slice(comment);
    buf
}

/// Build a complete minimal valid ZIP archive with one stored file entry.
fn build_valid_zip(filename: &str, file_data: &[u8], crc32: u32) -> Vec<u8> {
    let fname = filename.as_bytes();
    let size = file_data.len() as u32;

    // LFH + file data
    let lfh = build_lfh(fname, crc32, size, size);
    let lfh_len = lfh.len();

    // CD entry
    let cd = build_cd_entry(fname, crc32, size, size, 0);
    let cd_len = cd.len();
    let cd_offset = (lfh_len + file_data.len()) as u32;

    // EOCD
    let eocd = build_eocd(1, cd_len as u32, cd_offset, &[]);

    let mut archive = Vec::new();
    archive.extend_from_slice(&lfh);
    archive.extend_from_slice(file_data);
    archive.extend_from_slice(&cd);
    archive.extend_from_slice(&eocd);
    archive
}

// ══════════════════════════════════════════════════════════════════════════════
// 1. EOCD Detection Tests
// ══════════════════════════════════════════════════════════════════════════════

/// **Validates: Requirement 1.2**
#[test]
fn test_find_eocd_valid_at_end() {
    // Minimal EOCD at the end of data (no CD, no entries)
    let eocd = build_eocd(0, 0, 0, &[]);
    assert_eq!(eocd.len(), 22);

    let result = ZipScanner::find_eocd(&eocd);
    assert!(result.is_some(), "Should find EOCD at end of data");

    let (offset, record) = result.unwrap();
    assert_eq!(offset, 0);
    assert_eq!(record.cd_entries_total, 0);
    assert_eq!(record.cd_offset, 0);
    assert_eq!(record.comment.len(), 0);
}

/// **Validates: Requirement 1.2**
#[test]
fn test_find_eocd_with_comment() {
    let comment = b"This is a ZIP comment";
    let eocd = build_eocd(0, 0, 0, comment);

    let result = ZipScanner::find_eocd(&eocd);
    assert!(result.is_some(), "Should find EOCD with comment");

    let (offset, record) = result.unwrap();
    assert_eq!(offset, 0);
    assert_eq!(record.comment, comment.to_vec());
}

/// **Validates: Requirement 1.2**
#[test]
fn test_find_eocd_with_preceding_data() {
    // Some data before the EOCD (simulating LFH + CD before EOCD)
    let prefix = vec![0u8; 100];
    let eocd = build_eocd(0, 0, 0, &[]);

    let mut data = prefix.clone();
    data.extend_from_slice(&eocd);

    let result = ZipScanner::find_eocd(&data);
    assert!(result.is_some(), "Should find EOCD after preceding data");

    let (offset, _record) = result.unwrap();
    assert_eq!(offset, 100);
}

/// **Validates: Requirement 1.2**
#[test]
fn test_find_eocd_no_eocd_present() {
    // Random data with no EOCD signature
    let data = vec![0xAA; 100];
    let result = ZipScanner::find_eocd(&data);
    assert!(result.is_none(), "Should not find EOCD in random data");
}

/// **Validates: Requirement 1.2**
#[test]
fn test_find_eocd_data_too_short() {
    // Data shorter than 22 bytes (minimum EOCD size)
    let data = vec![0x50, 0x4B, 0x05, 0x06]; // Just the signature, not enough
    let result = ZipScanner::find_eocd(&data);
    assert!(result.is_none(), "Should not find EOCD when data is too short");
}

/// **Validates: Requirement 1.2**
#[test]
fn test_find_eocd_invalid_comment_length() {
    // EOCD with comment_length that doesn't match actual remaining data
    let mut eocd = build_eocd(0, 0, 0, &[]);
    // Overwrite comment_length to 10 (but no comment bytes follow)
    eocd[20] = 10;
    eocd[21] = 0;

    let result = ZipScanner::find_eocd(&eocd);
    assert!(result.is_none(), "Should reject EOCD with mismatched comment_length");
}

// ══════════════════════════════════════════════════════════════════════════════
// 2. LFH Parsing Tests
// ══════════════════════════════════════════════════════════════════════════════

/// **Validates: Requirement 1.3, 1.4**
#[test]
fn test_parse_lfh_valid_with_filename() {
    let filename = b"hello.txt";
    let crc = 0xDEADBEEF_u32;
    let lfh_data = build_lfh(filename, crc, 42, 42);

    let result = ZipScanner::parse_lfh(&lfh_data, 0);
    assert!(result.is_some(), "Should parse valid LFH");

    let header = result.unwrap();
    assert_eq!(header.offset, 0);
    assert_eq!(header.filename, filename.to_vec());
    assert_eq!(header.crc32, crc);
    assert_eq!(header.compressed_size, 42);
    assert_eq!(header.uncompressed_size, 42);
    assert_eq!(header.compression_method, 0);
    assert_eq!(header.version_needed, 20);
    // data_offset = 30 (fixed) + 9 (filename) + 0 (extra) = 39
    assert_eq!(header.data_offset, 39);
}

/// **Validates: Requirement 1.3**
#[test]
fn test_parse_lfh_at_nonzero_offset() {
    let filename = b"test.bin";
    let lfh_data = build_lfh(filename, 0x12345678, 100, 100);

    // Place LFH at offset 50 in a larger buffer
    let mut data = vec![0u8; 50];
    data.extend_from_slice(&lfh_data);

    let result = ZipScanner::parse_lfh(&data, 50);
    assert!(result.is_some(), "Should parse LFH at non-zero offset");

    let header = result.unwrap();
    assert_eq!(header.offset, 50);
    assert_eq!(header.filename, filename.to_vec());
    assert_eq!(header.crc32, 0x12345678);
}

/// **Validates: Requirement 1.3**
#[test]
fn test_parse_lfh_no_signature_at_offset() {
    // Data that doesn't start with LFH signature
    let data = vec![0x00; 50];
    let result = ZipScanner::parse_lfh(&data, 0);
    assert!(result.is_none(), "Should return None when no LFH signature at offset");
}

/// **Validates: Requirement 1.3**
#[test]
fn test_parse_lfh_truncated_header() {
    // Only 20 bytes — less than the 30-byte fixed header
    let mut data = vec![0x50, 0x4B, 0x03, 0x04]; // signature
    data.extend_from_slice(&[0u8; 16]); // only 16 more bytes (total 20, need 30)

    let result = ZipScanner::parse_lfh(&data, 0);
    assert!(result.is_none(), "Should return None for truncated LFH");
}

/// **Validates: Requirement 1.3**
#[test]
fn test_parse_lfh_truncated_variable_fields() {
    // Fixed header says filename_length=10, but only 5 bytes follow
    let mut data = Vec::new();
    data.extend_from_slice(&[0x50, 0x4B, 0x03, 0x04]); // signature
    data.extend_from_slice(&20u16.to_le_bytes()); // version
    data.extend_from_slice(&0u16.to_le_bytes()); // flags
    data.extend_from_slice(&0u16.to_le_bytes()); // method
    data.extend_from_slice(&0u16.to_le_bytes()); // mod time
    data.extend_from_slice(&0u16.to_le_bytes()); // mod date
    data.extend_from_slice(&0u32.to_le_bytes()); // crc
    data.extend_from_slice(&0u32.to_le_bytes()); // compressed size
    data.extend_from_slice(&0u32.to_le_bytes()); // uncompressed size
    data.extend_from_slice(&10u16.to_le_bytes()); // filename_length = 10
    data.extend_from_slice(&0u16.to_le_bytes()); // extra_field_length = 0
    // Only 5 bytes of filename (need 10)
    data.extend_from_slice(&[0x41; 5]);

    let result = ZipScanner::parse_lfh(&data, 0);
    assert!(result.is_none(), "Should return None when variable fields are truncated");
}

// ══════════════════════════════════════════════════════════════════════════════
// 3. scan_lfh_signatures Tests
// ══════════════════════════════════════════════════════════════════════════════

/// **Validates: Requirement 1.4**
#[test]
fn test_scan_lfh_signatures_finds_all() {
    let filename = b"a.txt";
    let lfh1 = build_lfh(filename, 0x11111111, 5, 5);
    let file_data1 = b"hello";
    let lfh2 = build_lfh(b"b.txt", 0x22222222, 5, 5);
    let file_data2 = b"world";

    let mut data = Vec::new();
    data.extend_from_slice(&lfh1);
    data.extend_from_slice(file_data1);
    let second_offset = data.len() as u64;
    data.extend_from_slice(&lfh2);
    data.extend_from_slice(file_data2);

    let offsets = ZipScanner::scan_lfh_signatures(&data);
    assert_eq!(offsets.len(), 2);
    assert_eq!(offsets[0], 0);
    assert_eq!(offsets[1], second_offset);
}

/// **Validates: Requirement 1.4**
#[test]
fn test_scan_lfh_signatures_empty_data() {
    let offsets = ZipScanner::scan_lfh_signatures(&[]);
    assert!(offsets.is_empty(), "Empty data should return no signatures");
}

/// **Validates: Requirement 1.4**
#[test]
fn test_scan_lfh_signatures_no_signatures() {
    let data = vec![0xAA; 100];
    let offsets = ZipScanner::scan_lfh_signatures(&data);
    assert!(offsets.is_empty(), "Random data should return no signatures");
}

/// **Validates: Requirement 1.4**
#[test]
fn test_scan_lfh_signatures_data_too_short() {
    // Less than 4 bytes
    let data = vec![0x50, 0x4B, 0x03];
    let offsets = ZipScanner::scan_lfh_signatures(&data);
    assert!(offsets.is_empty(), "Data shorter than signature should return empty");
}

// ══════════════════════════════════════════════════════════════════════════════
// 4. diagnose Tests
// ══════════════════════════════════════════════════════════════════════════════

/// **Validates: Requirement 1.5**
#[test]
fn test_diagnose_empty_data() {
    let report = ZipScanner::diagnose(&[]);
    assert_eq!(report.format, "zip");
    assert_eq!(report.total_entries, 0);
    assert!(!report.repairable, "Empty data should not be repairable");
}

/// **Validates: Requirement 1.5**
#[test]
fn test_diagnose_non_archive_data() {
    // Random bytes that don't contain any ZIP signatures
    let data = vec![0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE];
    let report = ZipScanner::diagnose(&data);
    assert_eq!(report.total_entries, 0);
    assert!(!report.repairable, "Non-archive data should not be repairable");
}

/// **Validates: Requirement 1.5**
#[test]
fn test_diagnose_data_less_than_4_bytes() {
    let data = vec![0x50, 0x4B]; // Only 2 bytes
    let report = ZipScanner::diagnose(&data);
    assert!(!report.repairable, "Data < 4 bytes should not be repairable");
    assert_eq!(report.total_entries, 0);
}

/// **Validates: Requirements 1.1, 1.2, 1.3, 1.4**
#[test]
fn test_diagnose_valid_zip_all_entries_intact() {
    let file_data = b"Hello, World!";
    let crc = crc32fast::hash(file_data);
    let archive = build_valid_zip("hello.txt", file_data, crc);

    let report = ZipScanner::diagnose(&archive);
    assert_eq!(report.format, "zip");
    assert_eq!(report.total_entries, 1);
    assert_eq!(report.valid_entries, 1);
    assert_eq!(report.corrupted_entries, 0);
    assert_eq!(report.unrecoverable_entries, 0);
    assert!(report.repairable, "Valid ZIP should be repairable");
    assert!(report.damages.is_empty(), "Valid ZIP should have no damages");
}

/// **Validates: Requirement 1.2**
#[test]
fn test_diagnose_missing_eocd() {
    // Build a ZIP with LFH + file data but NO CD and NO EOCD
    let filename = b"test.txt";
    let file_data = b"some content";
    let crc = crc32fast::hash(file_data);
    let size = file_data.len() as u32;

    let mut archive = build_lfh(filename, crc, size, size);
    archive.extend_from_slice(file_data);
    // No CD, no EOCD appended

    let report = ZipScanner::diagnose(&archive);
    assert_eq!(report.total_entries, 1);
    assert!(report.repairable, "ZIP with LFH but missing EOCD should be repairable");

    // Should report missing_eocd damage
    let has_missing_eocd = report.damages.iter().any(|d| d.damage_type == "missing_eocd");
    assert!(has_missing_eocd, "Should detect missing EOCD, damages: {:?}", report.damages);
}

/// **Validates: Requirement 1.3**
#[test]
fn test_diagnose_zeroed_crc() {
    // Build a valid ZIP but with CRC-32 set to 0 in the LFH
    let filename = b"zeroed.txt";
    let file_data = b"data with zeroed crc";
    let size = file_data.len() as u32;

    // Build LFH with crc32 = 0
    let lfh = build_lfh(filename, 0, size, size);
    let lfh_len = lfh.len();

    // Build CD entry also with crc32 = 0 (matching LFH)
    let cd = build_cd_entry(filename, 0, size, size, 0);
    let cd_len = cd.len();
    let cd_offset = (lfh_len + file_data.len()) as u32;

    let eocd = build_eocd(1, cd_len as u32, cd_offset, &[]);

    let mut archive = Vec::new();
    archive.extend_from_slice(&lfh);
    archive.extend_from_slice(file_data);
    archive.extend_from_slice(&cd);
    archive.extend_from_slice(&eocd);

    let report = ZipScanner::diagnose(&archive);
    assert_eq!(report.total_entries, 1);
    assert_eq!(report.corrupted_entries, 1);

    // Should report invalid_crc damage
    let has_invalid_crc = report.damages.iter().any(|d| d.damage_type == "invalid_crc");
    assert!(has_invalid_crc, "Should detect zeroed CRC, damages: {:?}", report.damages);
}

/// **Validates: Requirement 1.6**
#[test]
fn test_diagnose_count_invariant() {
    // Build a ZIP with 2 entries: one valid, one with zeroed CRC
    let file1 = b"good file";
    let crc1 = crc32fast::hash(file1);
    let file2 = b"bad file";

    let fname1 = b"good.txt";
    let fname2 = b"bad.txt";
    let size1 = file1.len() as u32;
    let size2 = file2.len() as u32;

    // LFH 1 (valid CRC)
    let lfh1 = build_lfh(fname1, crc1, size1, size1);
    // LFH 2 (zeroed CRC)
    let lfh2 = build_lfh(fname2, 0, size2, size2);

    let lfh1_offset = 0u32;
    let lfh2_offset = (lfh1.len() + file1.len()) as u32;

    // CD entries
    let cd1 = build_cd_entry(fname1, crc1, size1, size1, lfh1_offset);
    let cd2 = build_cd_entry(fname2, 0, size2, size2, lfh2_offset);
    let cd_offset = (lfh1.len() + file1.len() + lfh2.len() + file2.len()) as u32;
    let cd_size = (cd1.len() + cd2.len()) as u32;

    let eocd = build_eocd(2, cd_size, cd_offset, &[]);

    let mut archive = Vec::new();
    archive.extend_from_slice(&lfh1);
    archive.extend_from_slice(file1);
    archive.extend_from_slice(&lfh2);
    archive.extend_from_slice(file2);
    archive.extend_from_slice(&cd1);
    archive.extend_from_slice(&cd2);
    archive.extend_from_slice(&eocd);

    let report = ZipScanner::diagnose(&archive);
    // Invariant: total == valid + corrupted + unrecoverable
    assert_eq!(
        report.total_entries,
        report.valid_entries + report.corrupted_entries + report.unrecoverable_entries,
        "Count invariant violated: total={}, valid={}, corrupted={}, unrecoverable={}",
        report.total_entries, report.valid_entries, report.corrupted_entries, report.unrecoverable_entries
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// 5. RAR Scanner — Marker Detection Tests
// ══════════════════════════════════════════════════════════════════════════════

use zipease_extract::repair::rar_scanner::RarScanner;
use zipease_extract::repair::models::{RarVersion, RarHeaderInfo};

/// **Validates: Requirement 4.1**
#[test]
fn test_rar4_marker_at_start() {
    let mut data = Vec::new();
    data.extend_from_slice(b"Rar!\x1a\x07\x00");
    data.extend_from_slice(&[0u8; 50]); // padding

    let result = RarScanner::check_marker(&data);
    assert_eq!(result, Some(RarVersion::Rar4), "Should detect RAR4 marker at start");
}

/// **Validates: Requirement 4.1**
#[test]
fn test_rar5_marker_at_start() {
    let mut data = Vec::new();
    data.extend_from_slice(b"Rar!\x1a\x07\x01\x00");
    data.extend_from_slice(&[0u8; 50]); // padding

    let result = RarScanner::check_marker(&data);
    assert_eq!(result, Some(RarVersion::Rar5), "Should detect RAR5 marker at start");
}

/// **Validates: Requirement 4.1**
#[test]
fn test_no_marker_in_random_data() {
    let data = vec![0xAA; 200];
    let result = RarScanner::check_marker(&data);
    assert_eq!(result, None, "Should return None for data without RAR marker");
}

/// **Validates: Requirement 4.2**
#[test]
fn test_rar4_marker_with_garbage_before() {
    // Garbage bytes before the marker, but within 1024-byte scan limit
    let mut data = vec![0xDE; 100];
    data.extend_from_slice(b"Rar!\x1a\x07\x00");
    data.extend_from_slice(&[0u8; 50]);

    let result = RarScanner::check_marker(&data);
    assert_eq!(result, Some(RarVersion::Rar4), "Should detect RAR4 marker after garbage within 1024 bytes");
}

/// **Validates: Requirement 4.2**
#[test]
fn test_rar5_marker_with_garbage_before() {
    let mut data = vec![0xBE; 200];
    data.extend_from_slice(b"Rar!\x1a\x07\x01\x00");
    data.extend_from_slice(&[0u8; 50]);

    let result = RarScanner::check_marker(&data);
    assert_eq!(result, Some(RarVersion::Rar5), "Should detect RAR5 marker after garbage within 1024 bytes");
}

/// **Validates: Requirement 4.1**
#[test]
fn test_marker_data_too_short() {
    // Data shorter than either marker
    let data = b"Rar!";
    let result = RarScanner::check_marker(data);
    assert_eq!(result, None, "Should return None when data is shorter than marker");
}

/// **Validates: Requirement 4.1**
#[test]
fn test_marker_empty_data() {
    let result = RarScanner::check_marker(&[]);
    assert_eq!(result, None, "Should return None for empty data");
}

/// **Validates: Requirement 4.1**
/// RAR4 marker is a prefix of RAR5 marker — ensure RAR5 is detected correctly
#[test]
fn test_rar5_not_confused_with_rar4() {
    // RAR5 marker: Rar!\x1a\x07\x01\x00 (8 bytes)
    // RAR4 marker: Rar!\x1a\x07\x00 (7 bytes) — note the difference at byte 6
    let mut data = Vec::new();
    data.extend_from_slice(b"Rar!\x1a\x07\x01\x00");
    data.extend_from_slice(&[0u8; 100]);

    let result = RarScanner::check_marker(&data);
    assert_eq!(result, Some(RarVersion::Rar5), "RAR5 marker should not be confused with RAR4");
}

// ══════════════════════════════════════════════════════════════════════════════
// 6. RAR Scanner — Header CRC Validation Tests
// ══════════════════════════════════════════════════════════════════════════════

/// Build a valid RAR4 header with correct CRC.
///
/// RAR4 header structure (minimum 7 bytes):
/// - 2 bytes: header CRC (CRC32 of bytes from type field to end, lower 16 bits)
/// - 1 byte: header type
/// - 2 bytes: flags
/// - 2 bytes: header size (total including CRC and type fields)
fn build_rar4_header(header_type: u8, flags: u16) -> Vec<u8> {
    let header_size: u16 = 7; // minimum header: 2 CRC + 1 type + 2 flags + 2 size

    // Build the header body (type + flags + size) — this is what CRC covers
    let mut body = Vec::new();
    body.push(header_type);
    body.extend_from_slice(&flags.to_le_bytes());
    body.extend_from_slice(&header_size.to_le_bytes());

    // Compute CRC32 of body, take lower 16 bits
    let crc = (crc32fast::hash(&body) & 0xFFFF) as u16;

    // Assemble full header: CRC + body
    let mut header = Vec::new();
    header.extend_from_slice(&crc.to_le_bytes());
    header.extend_from_slice(&body);
    header
}

/// Build a complete RAR4 archive with marker + archive header + file header.
fn build_rar4_archive_with_headers() -> Vec<u8> {
    let mut data = Vec::new();

    // RAR4 marker
    data.extend_from_slice(b"Rar!\x1a\x07\x00");

    // Archive header (type 0x73)
    let archive_header = build_rar4_header(0x73, 0x0000);
    data.extend_from_slice(&archive_header);

    // File header (type 0x74) — needs to be at least 32 bytes for filename extraction
    // Build a more complete file header with filename
    let filename = b"test.txt";
    let filename_len = filename.len() as u16;
    // Header size = 7 (base) + 25 (file header fields before filename) + filename_len
    let header_size: u16 = 32 + filename_len;

    // Build body (everything after the 2-byte CRC)
    let mut body = Vec::new();
    body.push(0x74); // type: file header
    body.extend_from_slice(&0u16.to_le_bytes()); // flags
    body.extend_from_slice(&header_size.to_le_bytes()); // header size
    // File header specific fields (offsets +7 to +31 from header start):
    body.extend_from_slice(&100u32.to_le_bytes()); // compressed size (4 bytes)
    body.extend_from_slice(&100u32.to_le_bytes()); // uncompressed size (4 bytes)
    body.push(0x00); // host OS (1 byte)
    body.extend_from_slice(&0xAABBCCDDu32.to_le_bytes()); // file CRC (4 bytes)
    body.extend_from_slice(&0u32.to_le_bytes()); // date/time (4 bytes)
    body.push(29); // unpack version (1 byte)
    body.push(0x30); // method (1 byte)
    body.extend_from_slice(&filename_len.to_le_bytes()); // filename length (2 bytes)
    body.extend_from_slice(&0u32.to_le_bytes()); // file attributes (4 bytes)
    body.extend_from_slice(filename); // filename bytes

    // Compute CRC32 of body, take lower 16 bits
    let crc = (crc32fast::hash(&body) & 0xFFFF) as u16;

    // Assemble full header
    data.extend_from_slice(&crc.to_le_bytes());
    data.extend_from_slice(&body);

    data
}

/// **Validates: Requirement 4.3**
#[test]
fn test_validate_header_crc_valid_rar4() {
    let data = build_rar4_archive_with_headers();

    // The archive header starts right after the 7-byte marker
    let archive_header_offset = 7u64;
    let archive_header = RarHeaderInfo {
        offset: archive_header_offset,
        header_type: 0x73,
        header_size: 7,
        header_crc: u16::from_le_bytes([data[7], data[8]]),
        filename: None,
        data_size: 0,
        crc_valid: true,
    };

    let valid = RarScanner::validate_header_crc(&data, &archive_header);
    assert!(valid, "Valid RAR4 archive header should pass CRC validation");
}

/// **Validates: Requirement 4.3**
#[test]
fn test_validate_header_crc_invalid_rar4() {
    let mut data = build_rar4_archive_with_headers();

    // Corrupt the archive header CRC (at offset 7, 8)
    let archive_header_offset = 7usize;
    data[archive_header_offset] = 0xFF;
    data[archive_header_offset + 1] = 0xFF;

    let archive_header = RarHeaderInfo {
        offset: archive_header_offset as u64,
        header_type: 0x73,
        header_size: 7,
        header_crc: 0xFFFF, // corrupted CRC
        filename: None,
        data_size: 0,
        crc_valid: true,
    };

    let valid = RarScanner::validate_header_crc(&data, &archive_header);
    assert!(!valid, "Corrupted RAR4 header CRC should fail validation");
}

/// **Validates: Requirement 4.3**
#[test]
fn test_validate_header_crc_constructed_rar4() {
    // Construct a standalone RAR4 archive with just marker + one header
    // and verify CRC validation works
    let mut data = Vec::new();
    data.extend_from_slice(b"Rar!\x1a\x07\x00"); // RAR4 marker

    // Build a simple archive header (type 0x73, no flags)
    let header_type: u8 = 0x73;
    let flags: u16 = 0;
    let header_size: u16 = 7;

    let mut body = Vec::new();
    body.push(header_type);
    body.extend_from_slice(&flags.to_le_bytes());
    body.extend_from_slice(&header_size.to_le_bytes());

    let crc = (crc32fast::hash(&body) & 0xFFFF) as u16;
    data.extend_from_slice(&crc.to_le_bytes());
    data.extend_from_slice(&body);

    let header_info = RarHeaderInfo {
        offset: 7, // after marker
        header_type: 0x73,
        header_size: 7,
        header_crc: crc,
        filename: None,
        data_size: 0,
        crc_valid: true,
    };

    assert!(
        RarScanner::validate_header_crc(&data, &header_info),
        "Freshly constructed header with correct CRC should validate"
    );

    // Now corrupt one byte in the body and verify CRC fails
    let mut corrupted_data = data.clone();
    corrupted_data[9] = corrupted_data[9].wrapping_add(1); // corrupt the type byte

    assert!(
        !RarScanner::validate_header_crc(&corrupted_data, &header_info),
        "Header with corrupted body byte should fail CRC validation"
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// 7. RAR Scanner — diagnose Tests
// ══════════════════════════════════════════════════════════════════════════════

/// **Validates: Requirement 4.1, 4.2**
#[test]
fn test_rar_diagnose_no_marker() {
    // Random data with no RAR marker
    let data = vec![0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE, 0x00, 0x11, 0x22, 0x33];

    let report = RarScanner::diagnose(&data);
    assert_eq!(report.format, "rar");
    assert_eq!(report.total_entries, 0);
    assert!(!report.repairable, "Data without RAR marker should not be repairable");

    // Should have a "missing_marker" damage entry
    let has_missing_marker = report.damages.iter().any(|d| d.damage_type == "missing_marker");
    assert!(has_missing_marker, "Should report missing_marker damage, got: {:?}", report.damages);
}

/// **Validates: Requirement 4.1, 4.4**
#[test]
fn test_rar_diagnose_valid_rar4_with_headers() {
    let data = build_rar4_archive_with_headers();

    let report = RarScanner::diagnose(&data);
    assert_eq!(report.format, "rar");
    assert!(report.repairable, "Valid RAR4 archive should be repairable");
    // Should have at least one file header detected
    assert!(report.total_entries >= 1, "Should detect at least 1 file entry, got {}", report.total_entries);
    // Valid headers should have no corrupted entries
    assert_eq!(report.corrupted_entries, 0, "Valid RAR4 should have 0 corrupted entries");
}

/// **Validates: Requirement 4.3, 4.4**
#[test]
fn test_rar_diagnose_corrupted_header_crc() {
    let mut data = build_rar4_archive_with_headers();

    // Find the file header (starts after marker + archive header)
    // Marker = 7 bytes, archive header = 7 bytes, file header starts at offset 14
    let file_header_offset = 14usize;

    // Corrupt the file header CRC bytes (first 2 bytes of the file header)
    data[file_header_offset] = 0xFF;
    data[file_header_offset + 1] = 0xFF;

    let report = RarScanner::diagnose(&data);
    assert_eq!(report.format, "rar");
    assert!(report.repairable, "RAR with corrupted header CRC should still be repairable");

    // Should detect at least one corrupted header
    let has_corrupted = report.damages.iter().any(|d| d.damage_type == "corrupted_header");
    assert!(has_corrupted, "Should report corrupted_header damage, got: {:?}", report.damages);
}

/// **Validates: Requirement 4.1**
#[test]
fn test_rar_diagnose_empty_data() {
    let report = RarScanner::diagnose(&[]);
    assert_eq!(report.format, "rar");
    assert!(!report.repairable, "Empty data should not be repairable");
    assert_eq!(report.total_entries, 0);

    let has_missing_marker = report.damages.iter().any(|d| d.damage_type == "missing_marker");
    assert!(has_missing_marker, "Empty data should report missing_marker");
}

/// **Validates: Requirement 4.1, 4.4**
#[test]
fn test_rar_diagnose_marker_only_no_headers() {
    // Just the RAR4 marker with no headers following
    let data = b"Rar!\x1a\x07\x00".to_vec();

    let report = RarScanner::diagnose(&data);
    assert_eq!(report.format, "rar");
    assert_eq!(report.total_entries, 0);
    // No headers found means not repairable (nothing to repair)
    assert!(!report.repairable, "RAR with only marker and no headers should not be repairable");
}

// ══════════════════════════════════════════════════════════════════════════════
// 8. RAR Repairer — Unit Tests
// ══════════════════════════════════════════════════════════════════════════════

use zipease_extract::repair::rar_repairer::RarRepairer;

// ── 8.1 prepend_marker Tests ─────────────────────────────────────────────────

/// **Validates: Requirement 5.1**
#[test]
fn test_prepend_marker_rar4_writes_correct_bytes() {
    let mut output = Vec::new();
    RarRepairer::prepend_marker(&mut output, RarVersion::Rar4).unwrap();

    assert_eq!(output.len(), 7, "RAR4 marker should be 7 bytes");
    assert_eq!(output, b"Rar!\x1a\x07\x00", "RAR4 marker bytes should match");
}

/// **Validates: Requirement 5.1**
#[test]
fn test_prepend_marker_rar5_writes_correct_bytes() {
    let mut output = Vec::new();
    RarRepairer::prepend_marker(&mut output, RarVersion::Rar5).unwrap();

    assert_eq!(output.len(), 8, "RAR5 marker should be 8 bytes");
    assert_eq!(output, b"Rar!\x1a\x07\x01\x00", "RAR5 marker bytes should match");
}

/// **Validates: Requirement 5.1**
#[test]
fn test_prepend_marker_appends_to_existing_data() {
    let mut output = vec![0xAA, 0xBB, 0xCC];
    RarRepairer::prepend_marker(&mut output, RarVersion::Rar4).unwrap();

    assert_eq!(output.len(), 10, "Should append 7 bytes to existing 3 bytes");
    assert_eq!(&output[0..3], &[0xAA, 0xBB, 0xCC]);
    assert_eq!(&output[3..10], b"Rar!\x1a\x07\x00");
}

// ── 8.2 fix_header_crc Tests ─────────────────────────────────────────────────

/// **Validates: Requirement 5.2**
#[test]
fn test_fix_header_crc_rar4_corrects_corrupted_crc() {
    // Build a RAR4 header with a known body, then corrupt the CRC and fix it
    let header_type: u8 = 0x73; // archive header
    let flags: u16 = 0;
    let header_size: u16 = 7;

    // Build body (type + flags + size)
    let mut body = Vec::new();
    body.push(header_type);
    body.extend_from_slice(&flags.to_le_bytes());
    body.extend_from_slice(&header_size.to_le_bytes());

    // Compute correct CRC
    let correct_crc = (crc32fast::hash(&body) & 0xFFFF) as u16;

    // Assemble header with CORRUPTED CRC (0xFFFF)
    let mut data = Vec::new();
    data.extend_from_slice(&0xFFFFu16.to_le_bytes()); // corrupted CRC
    data.extend_from_slice(&body);

    let header_info = RarHeaderInfo {
        offset: 0,
        header_type: 0x73,
        header_size: 7,
        header_crc: 0xFFFF,
        filename: None,
        data_size: 0,
        crc_valid: false,
    };

    // Fix the CRC
    RarRepairer::fix_header_crc(&mut data, &header_info);

    // Verify the CRC is now correct
    let fixed_crc = u16::from_le_bytes([data[0], data[1]]);
    assert_eq!(fixed_crc, correct_crc, "Fixed CRC should match computed CRC");
}

/// **Validates: Requirement 5.2**
#[test]
fn test_fix_header_crc_rar4_at_nonzero_offset() {
    // Place a RAR4 header at a non-zero offset in the data buffer
    let header_type: u8 = 0x74; // file header
    let flags: u16 = 0x0001;
    let header_size: u16 = 7;

    let mut body = Vec::new();
    body.push(header_type);
    body.extend_from_slice(&flags.to_le_bytes());
    body.extend_from_slice(&header_size.to_le_bytes());

    let correct_crc = (crc32fast::hash(&body) & 0xFFFF) as u16;

    // Prefix with 20 bytes of garbage, then the header with corrupted CRC
    let mut data = vec![0xDE; 20];
    data.extend_from_slice(&0x0000u16.to_le_bytes()); // corrupted CRC
    data.extend_from_slice(&body);

    let header_info = RarHeaderInfo {
        offset: 20,
        header_type: 0x74,
        header_size: 7,
        header_crc: 0x0000,
        filename: None,
        data_size: 0,
        crc_valid: false,
    };

    RarRepairer::fix_header_crc(&mut data, &header_info);

    let fixed_crc = u16::from_le_bytes([data[20], data[21]]);
    assert_eq!(fixed_crc, correct_crc, "Fixed CRC at offset 20 should be correct");
}

/// **Validates: Requirement 5.2**
#[test]
fn test_fix_header_crc_rar5_corrects_corrupted_crc() {
    // Build a RAR5 header: 4 bytes CRC32 + body
    // RAR5 archive header body: size_vint=2, type_vint=1, flags_vint=0
    let body: &[u8] = &[2, 1, 0];
    let correct_crc = crc32fast::hash(body);

    // Assemble header with corrupted CRC (all zeros)
    let mut data = Vec::new();
    data.extend_from_slice(&0u32.to_le_bytes()); // corrupted CRC32
    data.extend_from_slice(body);

    let header_info = RarHeaderInfo {
        offset: 0,
        header_type: 1, // RAR5 archive header type (< 0x70, so it's RAR5)
        header_size: 7, // 4 CRC + 3 body
        header_crc: 0,
        filename: None,
        data_size: 0,
        crc_valid: false,
    };

    RarRepairer::fix_header_crc(&mut data, &header_info);

    let fixed_crc = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    assert_eq!(fixed_crc, correct_crc, "Fixed RAR5 CRC32 should match computed CRC32");
}

/// **Validates: Requirement 5.2**
#[test]
fn test_fix_header_crc_rar4_idempotent() {
    // If CRC is already correct, fix_header_crc should not change it
    let header_type: u8 = 0x73;
    let flags: u16 = 0;
    let header_size: u16 = 7;

    let mut body = Vec::new();
    body.push(header_type);
    body.extend_from_slice(&flags.to_le_bytes());
    body.extend_from_slice(&header_size.to_le_bytes());

    let correct_crc = (crc32fast::hash(&body) & 0xFFFF) as u16;

    let mut data = Vec::new();
    data.extend_from_slice(&correct_crc.to_le_bytes()); // already correct
    data.extend_from_slice(&body);

    let header_info = RarHeaderInfo {
        offset: 0,
        header_type: 0x73,
        header_size: 7,
        header_crc: correct_crc,
        filename: None,
        data_size: 0,
        crc_valid: true,
    };

    RarRepairer::fix_header_crc(&mut data, &header_info);

    let fixed_crc = u16::from_le_bytes([data[0], data[1]]);
    assert_eq!(fixed_crc, correct_crc, "Already-correct CRC should remain unchanged");
}

// ── 8.3 reconstruct_archive_header Tests ─────────────────────────────────────

/// **Validates: Requirement 5.3**
#[test]
fn test_reconstruct_archive_header_rar4_size_and_structure() {
    let header = RarRepairer::reconstruct_archive_header(RarVersion::Rar4);

    assert_eq!(header.len(), 13, "RAR4 archive header should be 13 bytes");

    // Verify structure: CRC(2) + type(1) + flags(2) + size(2) + reserved(6)
    let header_type = header[2];
    assert_eq!(header_type, 0x73, "Header type should be 0x73 (archive header)");

    let flags = u16::from_le_bytes([header[3], header[4]]);
    assert_eq!(flags, 0, "Flags should be 0");

    let size = u16::from_le_bytes([header[5], header[6]]);
    assert_eq!(size, 13, "Header size field should be 13");

    // Reserved bytes should be zero
    assert_eq!(&header[7..13], &[0u8; 6], "Reserved bytes should be zero");
}

/// **Validates: Requirement 5.3**
#[test]
fn test_reconstruct_archive_header_rar4_valid_crc() {
    let header = RarRepairer::reconstruct_archive_header(RarVersion::Rar4);

    // Verify CRC: CRC32 of bytes [2..13] (body), lower 16 bits
    let body = &header[2..13];
    let expected_crc = (crc32fast::hash(body) & 0xFFFF) as u16;
    let actual_crc = u16::from_le_bytes([header[0], header[1]]);

    assert_eq!(actual_crc, expected_crc, "RAR4 archive header CRC should be valid");
}

/// **Validates: Requirement 5.3**
#[test]
fn test_reconstruct_archive_header_rar5_size_and_structure() {
    let header = RarRepairer::reconstruct_archive_header(RarVersion::Rar5);

    assert_eq!(header.len(), 7, "RAR5 archive header should be 7 bytes");

    // Verify structure: CRC32(4) + size_vint(1) + type_vint(1) + flags_vint(1)
    let size_vint = header[4];
    assert_eq!(size_vint, 2, "Size vint should be 2 (type + flags)");

    let type_vint = header[5];
    assert_eq!(type_vint, 1, "Type vint should be 1 (archive header)");

    let flags_vint = header[6];
    assert_eq!(flags_vint, 0, "Flags vint should be 0");
}

/// **Validates: Requirement 5.3**
#[test]
fn test_reconstruct_archive_header_rar5_valid_crc() {
    let header = RarRepairer::reconstruct_archive_header(RarVersion::Rar5);

    // Verify CRC: CRC32 of bytes [4..7] (header data after CRC field)
    let body = &header[4..7];
    let expected_crc = crc32fast::hash(body);
    let actual_crc = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);

    assert_eq!(actual_crc, expected_crc, "RAR5 archive header CRC32 should be valid");
}

// ── 8.4 write_repaired Tests ─────────────────────────────────────────────────

/// **Validates: Requirement 5.4**
#[test]
fn test_write_repaired_rar4_output_starts_with_marker() {
    // Build a minimal RAR4 data buffer with one file header
    let header_type: u8 = 0x74; // file header
    let flags: u16 = 0;
    let header_size: u16 = 7;

    let mut body = Vec::new();
    body.push(header_type);
    body.extend_from_slice(&flags.to_le_bytes());
    body.extend_from_slice(&header_size.to_le_bytes());

    let crc = (crc32fast::hash(&body) & 0xFFFF) as u16;

    let mut data = Vec::new();
    data.extend_from_slice(&crc.to_le_bytes());
    data.extend_from_slice(&body);

    let headers = vec![RarHeaderInfo {
        offset: 0,
        header_type: 0x74,
        header_size: 7,
        header_crc: crc,
        filename: Some("test.txt".to_string()),
        data_size: 0,
        crc_valid: true,
    }];

    let mut output = Vec::new();
    let result = RarRepairer::write_repaired(&data, &headers, &mut output, |_, _, _| {});

    assert!(result.is_ok(), "write_repaired should succeed");

    // Output should start with RAR4 marker (7 bytes)
    assert!(output.len() >= 7, "Output should be at least 7 bytes");
    assert_eq!(&output[0..7], b"Rar!\x1a\x07\x00", "Output should start with RAR4 marker");
}

/// **Validates: Requirement 5.3, 5.4**
#[test]
fn test_write_repaired_rar4_inserts_archive_header_when_missing() {
    // Provide only a file header (type 0x74), no archive header (type 0x73)
    // write_repaired should insert a reconstructed archive header
    let header_type: u8 = 0x74;
    let flags: u16 = 0;
    let header_size: u16 = 7;

    let mut body = Vec::new();
    body.push(header_type);
    body.extend_from_slice(&flags.to_le_bytes());
    body.extend_from_slice(&header_size.to_le_bytes());

    let crc = (crc32fast::hash(&body) & 0xFFFF) as u16;

    let mut data = Vec::new();
    data.extend_from_slice(&crc.to_le_bytes());
    data.extend_from_slice(&body);

    let headers = vec![RarHeaderInfo {
        offset: 0,
        header_type: 0x74,
        header_size: 7,
        header_crc: crc,
        filename: Some("file.dat".to_string()),
        data_size: 0,
        crc_valid: true,
    }];

    let mut output = Vec::new();
    let result = RarRepairer::write_repaired(&data, &headers, &mut output, |_, _, _| {});

    assert!(result.is_ok());

    // Expected structure: marker(7) + archive_header(13) + file_header(7) = 27 bytes
    assert_eq!(output.len(), 27, "Output should be marker(7) + archive_header(13) + file_header(7) = 27 bytes");

    // Verify marker
    assert_eq!(&output[0..7], b"Rar!\x1a\x07\x00");

    // Verify archive header type at offset 7+2 = 9 (after marker + 2 CRC bytes)
    assert_eq!(output[9], 0x73, "Inserted archive header should have type 0x73");
}

/// **Validates: Requirement 5.4**
#[test]
fn test_write_repaired_rar4_with_data_area() {
    // Build a file header with data_size > 0 and verify data is written
    let header_type: u8 = 0x74;
    let flags: u16 = 0;
    let header_size: u16 = 7;

    let mut body = Vec::new();
    body.push(header_type);
    body.extend_from_slice(&flags.to_le_bytes());
    body.extend_from_slice(&header_size.to_le_bytes());

    let crc = (crc32fast::hash(&body) & 0xFFFF) as u16;

    // Data buffer: header(7) + file_data(10)
    let file_data = b"0123456789";
    let mut data = Vec::new();
    data.extend_from_slice(&crc.to_le_bytes());
    data.extend_from_slice(&body);
    data.extend_from_slice(file_data);

    let headers = vec![RarHeaderInfo {
        offset: 0,
        header_type: 0x74,
        header_size: 7,
        header_crc: crc,
        filename: Some("data.bin".to_string()),
        data_size: 10,
        crc_valid: true,
    }];

    let mut output = Vec::new();
    let result = RarRepairer::write_repaired(&data, &headers, &mut output, |_, _, _| {});

    assert!(result.is_ok());

    // Output: marker(7) + archive_header(13) + file_header(7) + file_data(10) = 37
    assert_eq!(output.len(), 37);

    // Verify file data is at the end
    assert_eq!(&output[27..37], file_data, "File data should be written after the header");
}

/// **Validates: Requirement 5.4**
#[test]
fn test_write_repaired_returns_recovered_entries() {
    let header_type: u8 = 0x74;
    let flags: u16 = 0;
    let header_size: u16 = 7;

    let mut body = Vec::new();
    body.push(header_type);
    body.extend_from_slice(&flags.to_le_bytes());
    body.extend_from_slice(&header_size.to_le_bytes());

    let crc = (crc32fast::hash(&body) & 0xFFFF) as u16;

    let mut data = Vec::new();
    data.extend_from_slice(&crc.to_le_bytes());
    data.extend_from_slice(&body);

    let headers = vec![RarHeaderInfo {
        offset: 0,
        header_type: 0x74,
        header_size: 7,
        header_crc: crc,
        filename: Some("recovered.txt".to_string()),
        data_size: 0,
        crc_valid: true,
    }];

    let mut output = Vec::new();
    let result = RarRepairer::write_repaired(&data, &headers, &mut output, |_, _, _| {}).unwrap();

    assert!(result.success);
    assert_eq!(result.recovered_entries, vec!["recovered.txt".to_string()]);
    assert!(result.failed_entries.is_empty());
}

/// **Validates: Requirement 5.4**
#[test]
fn test_write_repaired_empty_headers_returns_error() {
    let data = vec![0u8; 100];
    let headers: Vec<RarHeaderInfo> = vec![];

    let mut output = Vec::new();
    let result = RarRepairer::write_repaired(&data, &headers, &mut output, |_, _, _| {});

    assert!(result.is_err(), "Empty headers should return an error");
}

/// **Validates: Requirement 5.4**
#[test]
fn test_write_repaired_progress_callback_invoked() {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    let header_type: u8 = 0x74;
    let flags: u16 = 0;
    let header_size: u16 = 7;

    let mut body = Vec::new();
    body.push(header_type);
    body.extend_from_slice(&flags.to_le_bytes());
    body.extend_from_slice(&header_size.to_le_bytes());

    let crc = (crc32fast::hash(&body) & 0xFFFF) as u16;

    // Build data with two file headers back-to-back
    let mut data = Vec::new();
    data.extend_from_slice(&crc.to_le_bytes());
    data.extend_from_slice(&body);
    let second_offset = data.len() as u64;
    data.extend_from_slice(&crc.to_le_bytes());
    data.extend_from_slice(&body);

    let headers = vec![
        RarHeaderInfo {
            offset: 0,
            header_type: 0x74,
            header_size: 7,
            header_crc: crc,
            filename: Some("file1.txt".to_string()),
            data_size: 0,
            crc_valid: true,
        },
        RarHeaderInfo {
            offset: second_offset,
            header_type: 0x74,
            header_size: 7,
            header_crc: crc,
            filename: Some("file2.txt".to_string()),
            data_size: 0,
            crc_valid: true,
        },
    ];

    let call_count = Arc::new(AtomicU32::new(0));
    let call_count_clone = call_count.clone();

    let mut output = Vec::new();
    let result = RarRepairer::write_repaired(&data, &headers, &mut output, move |current, total, _name| {
        call_count_clone.fetch_add(1, Ordering::SeqCst);
        assert_eq!(total, 2, "Total should be 2 file headers");
        assert!(current >= 1 && current <= 2, "Current step should be 1 or 2");
    });

    assert!(result.is_ok());
    assert_eq!(call_count.load(Ordering::SeqCst), 2, "Progress callback should be called twice");
}


// ══════════════════════════════════════════════════════════════════════════════
// 9. FFI Null-Pointer Handling and Error Code Tests
// ══════════════════════════════════════════════════════════════════════════════

use zipease_extract::ffi::repair::{
    zip_ease_diagnose_archive, zip_ease_repair_archive,
};
use zipease_extract::repair::models::RepairError;

/// **Validates: Requirement 6.1, 6.5**
/// `zip_ease_diagnose_archive` with null pointer should return null.
#[test]
fn test_ffi_diagnose_null_pointer_returns_null() {
    let result = zip_ease_diagnose_archive(std::ptr::null());
    assert!(result.is_null(), "zip_ease_diagnose_archive(null) should return null");
}

/// **Validates: Requirement 6.5, 6.6**
/// `zip_ease_repair_archive` with null archive_path should return -1.
#[test]
fn test_ffi_repair_null_archive_path_returns_negative_one() {
    let result = zip_ease_repair_archive(std::ptr::null(), std::ptr::null(), None);
    assert_eq!(result, -1, "zip_ease_repair_archive(null, ...) should return -1");
}

/// **Validates: Requirement 6.5**
/// `zip_ease_free_diagnosis` with null pointer should not crash.
#[test]
fn test_ffi_free_diagnosis_null_pointer_no_crash() {
    // This should simply return without crashing
    unsafe {
        zipease_extract::ffi::repair::zip_ease_free_diagnosis(std::ptr::null_mut());
    }
}

/// **Validates: Requirement 6.6**
/// RepairError::NotAnArchive should map to FFI code 0x2006.
#[test]
fn test_repair_error_not_an_archive_ffi_code() {
    let err = RepairError::NotAnArchive;
    assert_eq!(err.to_ffi_code(), 0x2006_u32 as i32, "NotAnArchive should map to 0x2006");
}

/// **Validates: Requirement 6.6**
/// RepairError::NotRepairable should map to FFI code 0x2006.
#[test]
fn test_repair_error_not_repairable_ffi_code() {
    let err = RepairError::NotRepairable("too damaged".to_string());
    assert_eq!(err.to_ffi_code(), 0x2006_u32 as i32, "NotRepairable should map to 0x2006");
}

/// **Validates: Requirement 6.6**
/// RepairError::PartialRepair should map to FFI code 0x2007.
#[test]
fn test_repair_error_partial_repair_ffi_code() {
    use zipease_extract::repair::models::ScanResult;
    let err = RepairError::PartialRepair(ScanResult {
        success: false,
        recovered_entries: vec!["file1.txt".to_string()],
        failed_entries: vec!["file2.txt".to_string()],
        repaired_path: Some("/tmp/test_repaired.zip".to_string()),
    });
    assert_eq!(err.to_ffi_code(), 0x2007_u32 as i32, "PartialRepair should map to 0x2007");
}

/// **Validates: Requirement 6.6**
/// RepairError::IoError should map to FFI code -1.
#[test]
fn test_repair_error_io_error_ffi_code() {
    let err = RepairError::IoError("permission denied".to_string());
    assert_eq!(err.to_ffi_code(), -1, "IoError should map to -1");
}

/// **Validates: Requirement 6.1**
/// `zip_ease_diagnose_archive` with a valid path to a non-archive file should return null
/// (because diagnose returns Err for non-archive data).
#[test]
fn test_ffi_diagnose_non_archive_file_returns_null() {
    use std::io::Write;

    // Create a temp file with non-archive content
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("not_an_archive.txt");
    {
        let mut f = std::fs::File::create(&file_path).unwrap();
        f.write_all(b"This is not an archive file at all.").unwrap();
    }

    // Convert path to null-terminated UTF-16
    use std::os::windows::ffi::OsStrExt;
    let wide_path: Vec<u16> = file_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let result = zip_ease_diagnose_archive(wide_path.as_ptr());
    assert!(
        result.is_null(),
        "zip_ease_diagnose_archive on a non-archive file should return null"
    );
}
