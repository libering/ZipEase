use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use zipease_shared::LockError;

/// Generate a unique temp directory name using SystemTime nanoseconds + thread ID length.
pub fn unique_temp_name() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let tid = format!("{:?}", std::thread::current().id());
    format!("ZipEase_preview_{:x}_{}", nanos, tid.len())
}

/// Recursively search `root` for a file whose `file_name()` matches `target`
/// (case-insensitive). Returns the first match found.
pub fn find_file_recursive(root: &Path, target: &str) -> Option<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return None;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file_recursive(&path, target) {
                return Some(found);
            }
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.eq_ignore_ascii_case(target))
            .unwrap_or(false)
        {
            return Some(path);
        }
    }
    None
}

/// Extract a single entry from an archive by name, placing it in `output_dir`.
///
/// Strategy:
/// 1. Generate a unique temp dir and extract the entire archive into it.
/// 2. Recursively search the temp dir for a file whose `file_name()` matches
///    the `file_name()` component of `entry_name` (case-insensitive).
/// 3. Copy the found file to `output_dir` using `safe_join()`.
/// 4. Best-effort cleanup of the temp dir.
/// 5. Return the file name (not the full path) on success.
pub fn extract_entry_by_name(
    archive_path: &Path,
    entry_name: &str,
    output_dir: &Path,
) -> Result<String, LockError> {
    crate::zlog(&format!("[preview] extract_entry_by_name: entry={:?} archive={:?}", entry_name, archive_path));

    // Derive the target file_name component from entry_name
    let file_name_os = Path::new(entry_name)
        .file_name()
        .ok_or_else(|| LockError::ExtractionFailed("Invalid entry name".to_string()))?;

    let file_name_str = file_name_os
        .to_str()
        .ok_or_else(|| LockError::ExtractionFailed("Entry name is not valid UTF-8".to_string()))?
        .to_string();

    // Create a unique temp directory
    let temp_dir = env::temp_dir().join(unique_temp_name());
    fs::create_dir_all(&temp_dir).map_err(|e| {
        LockError::ExtractionFailed(format!("Failed to create temp dir: {}", e))
    })?;

    // Extract the entire archive into the temp dir
    let extract_result = crate::extract::extract_direct(archive_path, &temp_dir, |_, _, _| {});

    if let Err(e) = extract_result {
        let _ = fs::remove_dir_all(&temp_dir);
        return Err(e);
    }

    // Recursively locate the target file by file_name (case-insensitive)
    let src = match find_file_recursive(&temp_dir, &file_name_str) {
        Some(p) => {
            crate::zlog(&format!("[preview] found {:?} at {:?}", file_name_str, p));
            p
        }
        None => {
            crate::zlog(&format!("[preview] NOT FOUND: {:?} in {:?}", file_name_str, temp_dir));
            let _ = fs::remove_dir_all(&temp_dir);
            return Err(LockError::ExtractionFailed(format!(
                "File '{}' not found in extracted archive",
                file_name_str
            )));
        }
    };

    // Build the destination path using safe_join to prevent path traversal
    let dst = crate::extract::safe_join(output_dir, &file_name_str)?;

    // If destination already exists and is non-empty, reuse it (cache hit)
    if dst.exists() {
        if let Ok(meta) = std::fs::metadata(&dst) {
            if meta.len() > 0 {
                crate::zlog(&format!("[preview] cache hit: {:?}", file_name_str));
                let _ = fs::remove_dir_all(&temp_dir);
                return Ok(file_name_str);
            }
        }
        // Exists but empty or unreadable — remove read-only so we can overwrite
        #[allow(clippy::permissions_set_readonly_false)]
        if let Ok(meta) = std::fs::metadata(&dst) {
            let mut perms = meta.permissions();
            perms.set_readonly(false);
            let _ = std::fs::set_permissions(&dst, perms);
        }
    }

    // Copy the file to the output directory
    if let Err(e) = fs::copy(&src, &dst) {
        crate::zlog(&format!("[preview] copy FAILED: {src:?} -> {dst:?}: {e}"));
        let _ = fs::remove_dir_all(&temp_dir);
        return Err(LockError::ExtractionFailed(format!(
            "Failed to copy '{file_name_str}' to output dir: {e}"
        )));
    }

    crate::zlog(&format!("[preview] copy OK: {src:?} -> {dst:?}"));

    // Best-effort cleanup — ignore errors
    let _ = fs::remove_dir_all(&temp_dir);

    Ok(file_name_str)
}
