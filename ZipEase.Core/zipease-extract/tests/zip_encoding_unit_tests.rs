//! Unit tests for `decode_zip_filename` covering specific encoding scenarios.
//! Migrated from workspace-level tests/zip_encoding_tests.rs.

use zipease_extract::extract::encoding::decode_zip_filename;

// 7.1 UTF-8 flag fast path
#[test]
fn test_utf8_flag_fast_path() {
    let s = "hello_日本語.txt";
    let result = decode_zip_filename(s.as_bytes(), true);
    assert_eq!(result, s);
}

// 7.2 UTF-8 flag set but bytes are invalid UTF-8 — must not return U+FFFD, must be non-empty
#[test]
fn test_utf8_flag_invalid_bytes_fallback() {
    // 0x82 0xA0 is valid CP932 for "あ"
    let raw = &[0x82u8, 0xA0u8];
    let result = decode_zip_filename(raw, true);
    assert!(!result.is_empty(), "result must be non-empty");
    assert!(!result.contains('\u{FFFD}'), "must not contain replacement char");
}

// 7.3 CP932 (Japanese Shift-JIS)
#[test]
fn test_cp932_decoding() {
    // "テスト" in CP932: 0x83 0x65 0x83 0x58 0x83 0x67
    let raw = &[0x83u8, 0x65, 0x83, 0x58, 0x83, 0x67];
    let result = decode_zip_filename(raw, false);
    assert_eq!(result, "テスト");
}

// 7.4 EUC-JP
#[test]
fn test_euc_jp_decoding() {
    // "テスト" in EUC-JP: 0xA5 0xC6 0xA5 0xB9 0xA5 0xC8
    let raw = &[0xA5u8, 0xC6, 0xA5, 0xB9, 0xA5, 0xC8];
    let result = decode_zip_filename(raw, false);
    assert_eq!(result, "テスト");
}

// 7.5 CP950 (Traditional Chinese Big5)
// Short sequences are ambiguous to chardetng; use "中文測試檔案" (12 bytes) which is
// reliably detected as Big5. Bytes verified via encoding_rs::BIG5.encode().
#[test]
fn test_cp950_decoding() {
    // "中文測試檔案" in Big5/CP950
    let raw = &[0xA4u8, 0xA4, 0xA4, 0xE5, 0xB4, 0xFA, 0xB8, 0xD5, 0xC0, 0xC9, 0xAE, 0xD7];
    let result = decode_zip_filename(raw, false);
    assert_eq!(result, "中文測試檔案");
}

// 7.6 CP936 (Simplified Chinese GBK)
#[test]
fn test_cp936_decoding() {
    // "测试" in GBK/CP936: 0xB2 0xE2 0xCA 0xD4
    let raw = &[0xB2u8, 0xE2, 0xCA, 0xD4];
    let result = decode_zip_filename(raw, false);
    assert_eq!(result, "测试");
}

// 7.7 CP949 (Korean EUC-KR)
#[test]
fn test_cp949_decoding() {
    // "테스트" in EUC-KR/CP949: 0xC5 0xD7 0xBD 0xBA 0xC6 0xAE
    let raw = &[0xC5u8, 0xD7, 0xBD, 0xBA, 0xC6, 0xAE];
    let result = decode_zip_filename(raw, false);
    assert_eq!(result, "테스트");
}

// 7.8 CP1252 (Western European) — "café"
#[test]
fn test_cp1252_decoding() {
    // "café" in CP1252: c=0x63, a=0x61, f=0x66, é=0xE9
    let raw = &[0x63u8, 0x61, 0x66, 0xE9];
    let result = decode_zip_filename(raw, false);
    assert_eq!(result, "café");
}

// 7.9 Pure ASCII
#[test]
fn test_ascii_input() {
    let result = decode_zip_filename(b"hello.txt", false);
    assert_eq!(result, "hello.txt");
}
