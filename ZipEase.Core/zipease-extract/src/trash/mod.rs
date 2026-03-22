/// Converts a null-terminated UTF-16 pointer to a Rust String and moves the file
/// to the Windows Recycle Bin via the `trash` crate.
///
/// Returns `Ok(())` on success, or `Err(String)` with a plain-language message on failure.
/// No error codes or internal type names are exposed in the error string.
pub fn trash_file(path_ptr: *const u16) -> Result<(), String> {
    if path_ptr.is_null() {
        return Err("Invalid path".into());
    }

    // SAFETY: caller guarantees path_ptr is a valid, null-terminated UTF-16 sequence.
    // We read only until the null terminator and do not write through the pointer.
    let path = unsafe {
        let mut len = 0usize;
        while *path_ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(path_ptr, len);
        String::from_utf16_lossy(slice)
    };

    trash::delete(&path).map_err(|e| map_trash_error(e))
}

fn map_trash_error(e: trash::Error) -> String {
    match e {
        trash::Error::CouldNotAccess { .. } => {
            "The file could not be found. It may have already been moved or deleted.".into()
        }
        trash::Error::Os { code, .. } => match code {
            5 => "ZipEase doesn't have permission to move this file. Try running as administrator.".into(),
            32 | 33 => "The file is in use by another program. Close it and try again.".into(),
            _ => "The file could not be moved to the Recycle Bin. Close any programs using it and try again.".into(),
        },
        trash::Error::TargetedRoot => {
            "ZipEase cannot move a root folder to the Recycle Bin.".into()
        }
        trash::Error::CanonicalizePath { .. } => {
            "The file path is invalid or the file could not be found.".into()
        }
        trash::Error::ConvertOsString { .. } => {
            "The file path contains characters that cannot be processed. Rename the file and try again.".into()
        }
        trash::Error::Unknown { description } => {
            let _ = description; // don't expose internal description to the user
            "The file could not be moved to the Recycle Bin. Close any programs using it and try again.".into()
        }
        _ => "The file could not be moved to the Recycle Bin. Close any programs using it and try again.".into(),
    }
}

