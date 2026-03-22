use std::ffi::OsString;
use std::iter::once;
use std::os::windows::ffi::{OsStringExt, OsStrExt};
use std::path::PathBuf;
use chardetng::EncodingDetector;

/// Parse a null-terminated UTF-16 pointer into a PathBuf.
///
/// # Safety
/// `ptr` must point to a valid null-terminated UTF-16 string.
pub unsafe fn parse_wide_string(ptr: *const u16) -> PathBuf {
    if ptr.is_null() {
        return PathBuf::new();
    }
    let mut len = 0isize;
    while *ptr.offset(len) != 0 {
        len += 1;
    }
    let slice = std::slice::from_raw_parts(ptr, len as usize);
    PathBuf::from(OsString::from_wide(slice))
}

/// Convert a &str to a heap-allocated UTF-16 null-terminated *mut u16.
pub fn to_utf16_ptr(s: &str) -> *mut u16 {
    let wide: Vec<u16> = OsString::from(s)
        .encode_wide()
        .chain(once(0u16))
        .collect();
    Box::into_raw(wide.into_boxed_slice()) as *mut u16
}

/// Decode raw bytes to a UTF-8 String.
/// Tries UTF-8 first; falls back to chardetng CJK detection via encoding_rs.
pub fn decode_filename(raw_bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(raw_bytes) {
        return s.to_string();
    }
    let mut detector = EncodingDetector::new();
    detector.feed(raw_bytes, true);
    let encoding = detector.guess(None, true);
    let (decoded, _, _) = encoding.decode(raw_bytes);
    decoded.into_owned()
}
