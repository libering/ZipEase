//! Integration smoke tests for ZIP legacy encoding support.
//!
//! Feature: zip-encoding, tasks 7.10 and 7.11
//! Validates: Requirements 1.1, 1.2, 6.1, 6.2
//!
//! We build a minimal ZIP binary by hand so we can embed raw CP932 bytes as the
//! filename without going through ZipWriter (which requires valid UTF-8 names).

use tempfile::TempDir;
use zipease_extract::extract::zip::ZipBackend;
use zipease_extract::extract::ExtractionBackend;

/// CP932 bytes for "テスト.txt"
/// Verified: テ=0x83,0x65  ス=0x83,0x58  ト=0x83,0x67  .txt=0x2E,0x74,0x78,0x74
const CP932_NAME: &[u8] = &[0x83, 0x65, 0x83, 0x58, 0x83, 0x67, 0x2E, 0x74, 0x78, 0x74];
const FILE_CONTENT: &[u8] = b"hello";

/// Build a minimal valid ZIP archive with one stored file whose filename is raw CP932 bytes.
/// The UTF-8 flag (general-purpose bit 11) is intentionally NOT set, so decode_zip_filename
/// falls through to chardetng for CJK detection.
fn make_cp932_zip(dir: &std::path::Path) -> std::path::PathBuf {
    use std::io::Write;

    let name_len = CP932_NAME.len() as u16;
    let content_len = FILE_CONTENT.len() as u32;

    // CRC-32 of FILE_CONTENT ("hello")
    let crc = crc32fast::hash(FILE_CONTENT);

    // ── Local file header ────────────────────────────────────────────────────
    // Signature: PK\x03\x04
    let mut buf: Vec<u8> = Vec::new();
    buf.extend_from_slice(b"PK\x03\x04");
    buf.extend_from_slice(&20u16.to_le_bytes());   // version needed: 2.0
    buf.extend_from_slice(&0u16.to_le_bytes());    // general purpose bit flag: 0 (no UTF-8)
    buf.extend_from_slice(&0u16.to_le_bytes());    // compression: stored
    buf.extend_from_slice(&0u16.to_le_bytes());    // last mod time
    buf.extend_from_slice(&0u16.to_le_bytes());    // last mod date
    buf.extend_from_slice(&crc.to_le_bytes());     // crc-32
    buf.extend_from_slice(&content_len.to_le_bytes()); // compressed size
    buf.extend_from_slice(&content_len.to_le_bytes()); // uncompressed size
    buf.extend_from_slice(&name_len.to_le_bytes());    // file name length
    buf.extend_from_slice(&0u16.to_le_bytes());    // extra field length
    buf.extend_from_slice(CP932_NAME);             // file name (raw CP932)
    // no extra field
    let local_header_offset = 0u32;
    let _data_offset = buf.len();
    buf.extend_from_slice(FILE_CONTENT);           // file data

    // ── Central directory header ─────────────────────────────────────────────
    let central_dir_offset = buf.len() as u32;
    buf.extend_from_slice(b"PK\x01\x02");
    buf.extend_from_slice(&20u16.to_le_bytes());   // version made by
    buf.extend_from_slice(&20u16.to_le_bytes());   // version needed
    buf.extend_from_slice(&0u16.to_le_bytes());    // general purpose bit flag: 0
    buf.extend_from_slice(&0u16.to_le_bytes());    // compression: stored
    buf.extend_from_slice(&0u16.to_le_bytes());    // last mod time
    buf.extend_from_slice(&0u16.to_le_bytes());    // last mod date
    buf.extend_from_slice(&crc.to_le_bytes());     // crc-32
    buf.extend_from_slice(&content_len.to_le_bytes()); // compressed size
    buf.extend_from_slice(&content_len.to_le_bytes()); // uncompressed size
    buf.extend_from_slice(&name_len.to_le_bytes());    // file name length
    buf.extend_from_slice(&0u16.to_le_bytes());    // extra field length
    buf.extend_from_slice(&0u16.to_le_bytes());    // file comment length
    buf.extend_from_slice(&0u16.to_le_bytes());    // disk number start
    buf.extend_from_slice(&0u16.to_le_bytes());    // internal file attributes
    buf.extend_from_slice(&0u32.to_le_bytes());    // external file attributes
    buf.extend_from_slice(&local_header_offset.to_le_bytes()); // relative offset of local header
    buf.extend_from_slice(CP932_NAME);             // file name (raw CP932)

    // ── End of central directory record ─────────────────────────────────────
    let central_dir_size = (buf.len() as u32) - central_dir_offset;
    buf.extend_from_slice(b"PK\x05\x06");
    buf.extend_from_slice(&0u16.to_le_bytes());    // disk number
    buf.extend_from_slice(&0u16.to_le_bytes());    // disk with start of central dir
    buf.extend_from_slice(&1u16.to_le_bytes());    // entries on this disk
    buf.extend_from_slice(&1u16.to_le_bytes());    // total entries
    buf.extend_from_slice(&central_dir_size.to_le_bytes()); // size of central dir
    buf.extend_from_slice(&central_dir_offset.to_le_bytes()); // offset of central dir
    buf.extend_from_slice(&0u16.to_le_bytes());    // comment length

    let zip_path = dir.join("cp932.zip");
    let mut f = std::fs::File::create(&zip_path).unwrap();
    f.write_all(&buf).unwrap();
    zip_path
}

// Task 7.10: Integration smoke test — list_entries on a CP932 ZIP fixture
// Validates: Requirements 1.1, 1.2, 6.1
#[test]
fn test_list_entries_cp932_zip() {
    let dir = TempDir::new().unwrap();
    let zip_path = make_cp932_zip(dir.path());

    let entries = ZipBackend.list_entries(&zip_path).expect("list_entries must succeed");

    assert_eq!(entries.len(), 1, "must have exactly one entry");
    assert!(
        !entries[0].contains('\u{FFFD}'),
        "decoded filename must not contain replacement characters, got: {:?}",
        entries[0]
    );
    assert_eq!(
        entries[0], "テスト.txt",
        "CP932 filename must decode to correct Unicode"
    );
}

// Task 7.11: Integration smoke test — list_entries_info on the same CP932 fixture
// Validates: Requirements 6.2
#[test]
fn test_list_entries_info_cp932_zip() {
    let dir = TempDir::new().unwrap();
    let zip_path = make_cp932_zip(dir.path());

    let entries = ZipBackend.list_entries_info(&zip_path).expect("list_entries_info must succeed");

    assert_eq!(entries.len(), 1, "must have exactly one entry");
    assert_eq!(
        entries[0].name, "テスト.txt",
        "ArchiveEntryInfo.name must contain correctly decoded CP932 filename"
    );
    assert!(!entries[0].is_directory, "entry must be a file, not a directory");
    assert!(entries[0].size >= 0, "size must be non-negative");
}
