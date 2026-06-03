use std::sync::atomic::{AtomicI32, Ordering};

use crate::ffi::list::ArchiveEntryFFI;
use crate::search::{detect_mode, SearchMode};
use crate::search::pattern;

/// Search archive entries by pattern, returning matched indices via out-params.
///
/// # Parameters
/// - `pattern_ptr`: UTF-16 null-terminated search string
/// - `entries_ptr`: Previously returned ArchiveEntryFFI array from zip_ease_list_archive_contents
/// - `entry_count`: Number of entries
/// - `cancel_flag_ptr`: Pointer to i32, non-zero means cancel search (may be null)
/// - `out_indices_ptr`: Output parameter, receives matched indices array
/// - `out_count`: Output parameter, receives match count
///
/// # Returns
/// - 0: Success
/// - -1: Parameter error
/// - -2: Search cancelled
#[no_mangle]
pub extern "C" fn zip_ease_search_entries(
    pattern_ptr: *const u16,
    entries_ptr: *const ArchiveEntryFFI,
    entry_count: i32,
    cancel_flag_ptr: *const i32,
    out_indices_ptr: *mut *mut i32,
    out_count: *mut i32,
) -> i32 {
    std::panic::catch_unwind(|| {
        // Validate required parameters
        if pattern_ptr.is_null()
            || entries_ptr.is_null()
            || out_indices_ptr.is_null()
            || out_count.is_null()
            || entry_count < 0
        {
            return -1;
        }

        // Convert UTF-16 pattern to Rust String
        let pattern_path = unsafe { zipease_shared::parse_wide_string(pattern_ptr) };
        let pattern = pattern_path.to_string_lossy().into_owned();

        // Empty pattern → return empty results (success)
        if pattern.is_empty() {
            unsafe {
                *out_indices_ptr = std::ptr::null_mut();
                *out_count = 0;
            }
            return 0;
        }

        // Map cancel_flag_ptr to an AtomicI32 reference for safe atomic reads.
        // AtomicI32 has the same size/alignment as i32 (guaranteed by Rust).
        let cancel_flag: Option<&AtomicI32> = if cancel_flag_ptr.is_null() {
            None
        } else {
            Some(unsafe { &*(cancel_flag_ptr as *const AtomicI32) })
        };

        // Helper: check if cancelled
        let is_cancelled = || -> bool {
            cancel_flag
                .map(|f| f.load(Ordering::Relaxed) != 0)
                .unwrap_or(false)
        };

        // Check cancellation before starting
        if is_cancelled() {
            unsafe {
                *out_indices_ptr = std::ptr::null_mut();
                *out_count = 0;
            }
            return -2;
        }

        // Extract filenames from ArchiveEntryFFI entries as Strings
        let count = entry_count as usize;
        let entries_slice = unsafe { std::slice::from_raw_parts(entries_ptr, count) };

        let mut filenames: Vec<String> = Vec::with_capacity(count);
        for entry in entries_slice.iter() {
            if entry.file_name_ptr.is_null() {
                filenames.push(String::new());
            } else {
                // Read UTF-16 null-terminated string from file_name_ptr
                let path = unsafe { zipease_shared::parse_wide_string(entry.file_name_ptr as *const u16) };
                filenames.push(path.to_string_lossy().into_owned());
            }
        }

        // Detect search mode and prepare matcher
        let mode = detect_mode(&pattern);

        enum Matcher {
            Glob(globset::GlobMatcher),
            Substring(String),
        }

        let matcher = match mode {
            SearchMode::Glob => match pattern::compile_glob(&pattern) {
                Ok(glob_matcher) => Matcher::Glob(glob_matcher),
                Err(_) => Matcher::Substring(pattern.to_lowercase()),
            },
            SearchMode::Substring => Matcher::Substring(pattern.to_lowercase()),
        };

        // Search loop with periodic cancellation check (every 1024 iterations)
        let mut results: Vec<i32> = Vec::new();

        for (i, filename) in filenames.iter().enumerate() {
            // Check cancellation every 1024 iterations
            if i & 0x3FF == 0 && i > 0 && is_cancelled() {
                unsafe {
                    *out_indices_ptr = std::ptr::null_mut();
                    *out_count = 0;
                }
                return -2;
            }

            let matched = match &matcher {
                Matcher::Glob(glob_matcher) => glob_matcher.is_match(filename),
                Matcher::Substring(needle_lower) => pattern::substring_match(filename, needle_lower),
            };

            if matched {
                results.push(i as i32);
            }
        }

        // Final cancellation check after loop
        if is_cancelled() {
            unsafe {
                *out_indices_ptr = std::ptr::null_mut();
                *out_count = 0;
            }
            return -2;
        }

        // Write results to out-params
        let result_count = results.len() as i32;
        if results.is_empty() {
            unsafe {
                *out_indices_ptr = std::ptr::null_mut();
                *out_count = 0;
            }
        } else {
            let boxed = results.into_boxed_slice();
            let ptr = Box::into_raw(boxed) as *mut i32;
            unsafe {
                *out_indices_ptr = ptr;
                *out_count = result_count;
            }
        }

        0 // Success
    })
    .unwrap_or(-1)
}

/// Free the search results index array previously returned by `zip_ease_search_entries`.
///
/// # Safety
/// `indices_ptr` must have been allocated by `zip_ease_search_entries` and not yet freed.
#[no_mangle]
pub extern "C" fn zip_ease_free_search_results(indices_ptr: *mut i32, count: i32) {
    let _ = std::panic::catch_unwind(|| {
        if indices_ptr.is_null() || count <= 0 {
            return;
        }
        unsafe {
            // Reconstruct the Vec from the raw pointer and let it drop
            let _ = Vec::from_raw_parts(indices_ptr, count as usize, count as usize);
        }
    });
}
