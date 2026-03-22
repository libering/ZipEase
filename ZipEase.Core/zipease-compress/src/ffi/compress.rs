use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use zipease_shared::{set_last_error, parse_wide_string};
use crate::compress::{CompressOptions, compress_with_progress as smart_compress};

/// Progress callback: fn(percentage: i32, current_file_utf16: *const u16)
type CompressProgressCallback = extern "C" fn(i32, *const u16);

/// Create an archive from a list of input paths.
///
/// # Parameters
/// - `input_paths_ptr`  : pointer to an array of `input_count` UTF-16 string pointers (C#-owned)
/// - `input_count`      : number of input paths
/// - `output_path_ptr`  : UTF-16 null-terminated path for the output archive (C#-owned)
/// - `level`            : compression level 0–9
/// - `progress_callback`: optional progress callback (may be null)
///
/// # Returns
/// 0 on success, negative error code on failure.
/// Rust never frees any pointer passed in from C#.
#[no_mangle]
pub extern "C" fn zip_ease_compress(
    input_paths_ptr: *const *const u16,
    input_count: i32,
    output_path_ptr: *const u16,
    level: i32,
    progress_callback: Option<CompressProgressCallback>,
) -> i32 {
    std::panic::catch_unwind(|| {
        // Guard: null pointer or invalid count
        if input_paths_ptr.is_null() || output_path_ptr.is_null() || input_count < 1 {
            return -1;
        }

        let level = level.clamp(0, 9) as u8;

        // Parse output path
        let output_path = unsafe { parse_wide_string(output_path_ptr) };

        // Parse input paths — Rust reads but never frees these C#-owned pointers
        let input_paths_owned: Vec<PathBuf> = unsafe {
            let slice = std::slice::from_raw_parts(input_paths_ptr, input_count as usize);
            slice.iter().map(|&ptr| parse_wide_string(ptr)).collect()
        };

        let input_refs: Vec<&std::path::Path> = input_paths_owned
            .iter()
            .map(|p| p.as_path())
            .collect();

        let options = CompressOptions {
            level,
            store_relative_paths: true,
        };

        let progress_fn = |current: usize, total: usize, file_name: &str| {
            if let Some(callback) = progress_callback {
                let percentage = if total > 0 {
                    ((current as f64 / total as f64) * 100.0) as i32
                } else {
                    0
                };
                let wide_name: Vec<u16> = OsString::from(file_name)
                    .encode_wide()
                    .chain(std::iter::once(0u16))
                    .collect();
                callback(percentage, wide_name.as_ptr());
            }
        };

        match smart_compress(&input_refs, &output_path, &options, progress_fn) {
            Ok(()) => 0,
            Err(e) => {
                let code = e.to_error_code();
                set_last_error(e);
                code
            }
        }
    })
    .unwrap_or(-1)
}
