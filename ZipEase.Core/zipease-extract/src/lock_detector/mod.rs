// Lock detector module — queries which processes hold a lock on a given file.
// Implementation lives here; FFI shim is in src/ffi/lock_detector.rs.

/// Joins a slice of process names into a single comma-separated string.
///
/// Returns `names.join(", ")`. An empty slice returns an empty string.
pub fn join_process_names(names: &[String]) -> String {
    names.join(", ")
}

/// Query which processes hold a lock on the file at `path_ptr`.
///
/// Returns a heap-allocated null-terminated UTF-16 string of comma-separated
/// process names, or null if no lock is detected or any error occurs.
///
/// # Safety
/// `path_ptr` must be a valid null-terminated UTF-16 string, or null.
pub fn who_locks(path_ptr: *const u16) -> *mut u16 {
    // Requirement 2.4: null-pointer guard
    if path_ptr.is_null() {
        return std::ptr::null_mut();
    }

    // Requirement 6.1: convert null-terminated UTF-16 to String
    let path = unsafe {
        let mut len = 0usize;
        while *path_ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(path_ptr, len);
        String::from_utf16_lossy(slice).to_string()
    };

    // Requirements 2.2, 4.1: call wholock; return null on Err or empty result
    let processes = match wholock::who_locks_file(&path) {
        Ok(v) if !v.is_empty() => v,
        _ => return std::ptr::null_mut(),
    };

    // Requirements 1.2, 6.4: join names, encode to null-terminated UTF-16, leak
    let names: Vec<String> = processes.into_iter().map(|p| p.process_name).collect();
    let joined = join_process_names(&names);

    let mut wide: Vec<u16> = joined.encode_utf16().collect();
    wide.push(0); // null terminator

    // Box::into_raw(boxed_slice) ensures len == capacity, matching zip_ease_free_string
    Box::into_raw(wide.into_boxed_slice()) as *mut u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_name_returns_unchanged() {
        let names = vec!["Google Chrome".to_string()];
        assert_eq!(join_process_names(&names), "Google Chrome");
    }

    #[test]
    fn two_names_produce_comma_separated() {
        let names = vec!["A".to_string(), "B".to_string()];
        assert_eq!(join_process_names(&names), "A, B");
    }
}
