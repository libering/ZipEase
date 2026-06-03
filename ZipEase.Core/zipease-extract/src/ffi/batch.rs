use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use zipease_shared::parse_wide_string;

use crate::batch::{batch_extract, BatchProgress};

/// FFI callback signature: (archive_index, archive_count, file_percent, current_file_name_ptr)
type BatchProgressCallback = extern "C" fn(u32, u32, i32, *const u16);

/// Batch extraction FFI entry point.
///
/// # Parameters
/// - `paths_ptr`: pointer to array of UTF-16 null-terminated string pointers
/// - `path_count`: number of archives in the array
/// - `output_dir_ptr`: output directory as UTF-16 null-terminated pointer
/// - `progress_callback`: optional progress callback (archive_index, archive_count, file_percent, current_file_name_ptr)
/// - `cancel_flag_ptr`: pointer to i32 cancel flag (non-zero = cancel); may be null
///
/// # Returns
/// - `>= 0`: number of successfully extracted archives
/// - `< 0`: error code on overall failure
#[no_mangle]
pub extern "C" fn zip_ease_batch_extract(
    paths_ptr: *const *const u16,
    path_count: i32,
    output_dir_ptr: *const u16,
    progress_callback: Option<BatchProgressCallback>,
    cancel_flag_ptr: *const i32,
) -> i32 {
    std::panic::catch_unwind(|| {
        // Validate required parameters
        if paths_ptr.is_null() || output_dir_ptr.is_null() || path_count < 0 {
            return -1;
        }

        // Handle zero archives case
        if path_count == 0 {
            return 0;
        }

        let count = path_count as usize;

        // Parse UTF-16 pointer array into Vec<PathBuf>
        let archives: Vec<PathBuf> = unsafe {
            let path_ptrs = std::slice::from_raw_parts(paths_ptr, count);
            path_ptrs
                .iter()
                .map(|&ptr| {
                    if ptr.is_null() {
                        PathBuf::new()
                    } else {
                        parse_wide_string(ptr)
                    }
                })
                .collect()
        };

        // Parse output directory
        let output_dir = unsafe { parse_wide_string(output_dir_ptr) };

        // Map cancel_flag_ptr to AtomicBool
        // We use an AtomicBool that we update by reading the C# pinned int before each check
        let cancel_flag = AtomicBool::new(false);

        // If cancel_flag_ptr is provided, read its initial value
        if !cancel_flag_ptr.is_null() {
            let val = unsafe { *cancel_flag_ptr };
            if val != 0 {
                cancel_flag.store(true, Ordering::Relaxed);
            }
        }

        // Create progress callback wrapper
        let progress_wrapper = |progress: BatchProgress| {
            // Update cancel flag from the C# pinned int on each progress tick
            if !cancel_flag_ptr.is_null() {
                let val = unsafe { *cancel_flag_ptr };
                cancel_flag.store(val != 0, Ordering::Relaxed);
            }

            // Invoke the FFI callback if provided
            if let Some(callback) = progress_callback {
                // Allocate a temporary UTF-16 null-terminated string for the filename
                let wide_name: Vec<u16> = OsString::from(&progress.current_file_name)
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                callback(
                    progress.archive_index,
                    progress.archive_count,
                    progress.file_percent,
                    wide_name.as_ptr(),
                );
                // wide_name is dropped here — callback must copy if it needs the string longer
            }
        };

        // Execute batch extraction
        let result = batch_extract(&archives, &output_dir, &cancel_flag, progress_wrapper);

        result.success_count() as i32
    })
    .unwrap_or(-1)
}
