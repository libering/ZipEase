//! FFI module — extern "C" functions for the image preview plugin.
//!
//! All functions are wrapped in `std::panic::catch_unwind` to prevent Rust panics
//! from crossing the FFI boundary into the C# host process.
//!
//! Path parameters are received as UTF-16 pointers with explicit length (no null
//! terminator required), matching the C# `string.Length` convention.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use crate::decoder::{decode_image, DecodeOptions, DecodeResult};
use crate::error::PreviewError;
use crate::magic_bytes::validate_magic_bytes;
use crate::thumbnail::{generate_thumbnail, ThumbnailOptions};

/// FFI-safe image result passed across the Rust / C# boundary.
///
/// C# receives this struct via P/Invoke output parameter. The `pixels` pointer
/// is Rust-allocated and MUST be freed by calling `free_image_buffer(pixels, pixels_len)`.
#[repr(C)]
pub struct ImageResultFFI {
    /// Pointer to RGBA pixel buffer (Rust-allocated).
    pub pixels: *mut u8,
    /// Length of the pixel buffer in bytes.
    pub pixels_len: usize,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
}

impl Default for ImageResultFFI {
    fn default() -> Self {
        Self {
            pixels: std::ptr::null_mut(),
            pixels_len: 0,
            width: 0,
            height: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Parse a UTF-16 pointer + length (in u16 units) into a `PathBuf`.
///
/// Returns `Err(PreviewError::DecodeFailed)` if the pointer is null or length is negative.
fn parse_wide_path(ptr: *const u16, len: i32) -> Result<PathBuf, PreviewError> {
    if ptr.is_null() || len < 0 {
        return Err(PreviewError::DecodeFailed(
            "Null pointer or negative length for path argument".to_string(),
        ));
    }
    if len == 0 {
        return Err(PreviewError::DecodeFailed(
            "Empty path argument".to_string(),
        ));
    }
    let slice = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let os_string = OsString::from_wide(slice);
    Ok(PathBuf::from(os_string))
}

/// Parse a UTF-16 pointer + length into a `String` (for extension parameter).
///
/// Returns `Err(PreviewError::DecodeFailed)` if the pointer is null or length is negative.
fn parse_wide_str(ptr: *const u16, len: i32) -> Result<String, PreviewError> {
    if ptr.is_null() || len < 0 {
        return Err(PreviewError::DecodeFailed(
            "Null pointer or negative length for string argument".to_string(),
        ));
    }
    if len == 0 {
        return Ok(String::new());
    }
    let slice = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    Ok(String::from_utf16_lossy(slice))
}

/// Write a successful `DecodeResult` into the output `ImageResultFFI` pointer.
///
/// Transfers ownership of the pixel buffer to the caller — the Vec is leaked
/// and must be freed via `free_image_buffer`.
unsafe fn write_result(result: DecodeResult, out: *mut ImageResultFFI) {
    let mut pixels = result.pixels;
    pixels.shrink_to_fit();
    let ptr = pixels.as_mut_ptr();
    let len = pixels.len();
    std::mem::forget(pixels);

    (*out).pixels = ptr;
    (*out).pixels_len = len;
    (*out).width = result.width;
    (*out).height = result.height;
}

/// Handle an error: set thread-local last_error and return the negative error code.
fn handle_error(err: PreviewError) -> i32 {
    let code = err.to_error_code();
    err.set_last_error();
    code
}

/// Use `safe_join` from zipease-extract for all temp path construction.
/// Wraps the result into a `PreviewError::PathTraversal` on failure.
pub(crate) fn safe_join(base: &std::path::Path, entry_name: &str) -> Result<PathBuf, PreviewError> {
    zipease_extract::extract::safe_join(base, entry_name)
        .map_err(|_| PreviewError::PathTraversal(entry_name.to_string()))
}

// ---------------------------------------------------------------------------
// Exported FFI functions
// ---------------------------------------------------------------------------

/// Decode an image file to RGBA pixel buffer.
///
/// # Parameters
/// - `file_path_ptr`: UTF-16 encoded file path pointer
/// - `file_path_len`: Length of the path in UTF-16 code units (i32)
/// - `out_result`: Pointer to caller-allocated `ImageResultFFI` struct
///
/// # Returns
/// - `0` on success (result written to `out_result`)
/// - Negative error code on failure (call `zip_ease_get_last_error` for message)
#[no_mangle]
pub extern "C" fn zip_ease_decode_image(
    file_path_ptr: *const u16,
    file_path_len: i32,
    out_result: *mut ImageResultFFI,
) -> i32 {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if out_result.is_null() {
            return handle_error(PreviewError::DecodeFailed(
                "Null output pointer".to_string(),
            ));
        }

        let file_path = match parse_wide_path(file_path_ptr, file_path_len) {
            Ok(p) => p,
            Err(e) => return handle_error(e),
        };

        let options = DecodeOptions::default();

        match decode_image(&file_path, &options) {
            Ok(decode_result) => {
                unsafe { write_result(decode_result, out_result) };
                0
            }
            Err(e) => handle_error(e),
        }
    }));

    match result {
        Ok(code) => code,
        Err(_) => {
            let err = PreviewError::InternalPanic;
            handle_error(err)
        }
    }
}

/// Generate a thumbnail for an image file.
///
/// # Parameters
/// - `file_path_ptr`: UTF-16 encoded file path pointer
/// - `file_path_len`: Length of the path in UTF-16 code units (i32)
/// - `max_width`: Maximum thumbnail width in pixels
/// - `max_height`: Maximum thumbnail height in pixels
/// - `out_result`: Pointer to caller-allocated `ImageResultFFI` struct
///
/// # Returns
/// - `0` on success (result written to `out_result`)
/// - Negative error code on failure
#[no_mangle]
pub extern "C" fn zip_ease_generate_thumbnail(
    file_path_ptr: *const u16,
    file_path_len: i32,
    max_width: u32,
    max_height: u32,
    out_result: *mut ImageResultFFI,
) -> i32 {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if out_result.is_null() {
            return handle_error(PreviewError::DecodeFailed(
                "Null output pointer".to_string(),
            ));
        }

        let file_path = match parse_wide_path(file_path_ptr, file_path_len) {
            Ok(p) => p,
            Err(e) => return handle_error(e),
        };

        let options = ThumbnailOptions {
            max_width: if max_width == 0 { 64 } else { max_width },
            max_height: if max_height == 0 { 64 } else { max_height },
        };

        match generate_thumbnail(&file_path, &options) {
            Ok(decode_result) => {
                unsafe { write_result(decode_result, out_result) };
                0
            }
            Err(e) => handle_error(e),
        }
    }));

    match result {
        Ok(code) => code,
        Err(_) => {
            let err = PreviewError::InternalPanic;
            handle_error(err)
        }
    }
}

/// Free a pixel buffer previously allocated by `zip_ease_decode_image` or
/// `zip_ease_generate_thumbnail`.
///
/// # Safety
/// - `ptr` must have been returned in an `ImageResultFFI.pixels` field
/// - `len` must match the corresponding `ImageResultFFI.pixels_len`
/// - Must be called exactly once per successful decode/thumbnail call
/// - After calling, the pointer is invalid and must not be used
#[no_mangle]
pub extern "C" fn free_image_buffer(ptr: *mut u8, len: usize) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if ptr.is_null() || len == 0 {
            return;
        }
        // Reconstruct the Vec from the raw parts and let it drop, freeing the memory.
        unsafe {
            let _ = Vec::from_raw_parts(ptr, len, len);
        }
    }));
}

/// Performs startup cleanup of stale temp files from previous sessions.
///
/// Called by C# on application startup. Removes all files and subdirectories
/// within the temp directory that may have been left behind by a previous
/// abnormal termination.
///
/// # Safety
/// This function is safe to call at any time. Failures are logged but never propagated.
#[no_mangle]
pub extern "C" fn zip_ease_preview_startup_cleanup() {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::temp::startup_cleanup();
    }));
}

/// Cleans up all preview temp files.
///
/// Called by C# on application exit. Removes all files and subdirectories
/// within the temp directory.
///
/// # Safety
/// This function is safe to call at any time. Failures are logged but never propagated.
#[no_mangle]
pub extern "C" fn zip_ease_preview_cleanup_all_temps() {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::temp::cleanup_all_temps();
    }));
}

/// Cleans up all preview temp files for a specific archive.
///
/// Called by C# when an archive is closed. Removes all temp files associated
/// with the given archive ID (the subdirectory name within the temp directory).
///
/// # Parameters
/// - `archive_id_ptr`: UTF-16 encoded archive ID pointer
/// - `archive_id_len`: Length of the archive ID in UTF-16 code units (i32)
#[no_mangle]
pub extern "C" fn zip_ease_preview_cleanup_archive(
    archive_id_ptr: *const u16,
    archive_id_len: i32,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let archive_id = match parse_wide_str(archive_id_ptr, archive_id_len) {
            Ok(s) => s,
            Err(_) => return,
        };
        crate::temp::cleanup_archive_temps(&archive_id);
    }));
}

/// Validate whether an image file's magic bytes match its claimed extension.
///
/// This is used to verify that an archive entry is genuinely an image of the
/// claimed format before attempting full decode.
///
/// # Parameters
/// - `file_path_ptr`: UTF-16 encoded file path pointer
/// - `file_path_len`: Length of the path in UTF-16 code units (i32)
/// - `extension_ptr`: UTF-16 encoded extension string pointer (without dot)
/// - `extension_len`: Length of the extension in UTF-16 code units (i32)
///
/// # Returns
/// - `0` if magic bytes match the claimed extension
/// - Negative error code if validation fails
#[no_mangle]
pub extern "C" fn zip_ease_validate_image_entry(
    file_path_ptr: *const u16,
    file_path_len: i32,
    extension_ptr: *const u16,
    extension_len: i32,
) -> i32 {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let file_path = match parse_wide_path(file_path_ptr, file_path_len) {
            Ok(p) => p,
            Err(e) => return handle_error(e),
        };

        let extension = match parse_wide_str(extension_ptr, extension_len) {
            Ok(s) => s,
            Err(e) => return handle_error(e),
        };

        // Read the first 12 bytes of the file for magic byte validation
        let header = match std::fs::read(&file_path) {
            Ok(data) => data,
            Err(e) => {
                return handle_error(PreviewError::DecodeFailed(format!(
                    "Cannot read file: {}",
                    e
                )));
            }
        };

        let header_len = header.len().min(12);
        match validate_magic_bytes(&header[..header_len], &extension) {
            Ok(()) => 0,
            Err(e) => handle_error(e),
        }
    }));

    match result {
        Ok(code) => code,
        Err(_) => {
            let err = PreviewError::InternalPanic;
            handle_error(err)
        }
    }
}
