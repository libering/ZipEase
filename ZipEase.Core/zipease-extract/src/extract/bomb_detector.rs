use zipease_shared::LockError;
use crate::extract::ArchiveEntryInfo;

/// Configurable thresholds for Zip Bomb detection.
/// All fields have safe defaults. Can be persisted via AppSettings.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BombThresholds {
    /// Maximum allowed compression ratio (uncompressed total / archive file size).
    /// Default: 100.0
    pub max_compression_ratio: f64,
    /// Maximum total uncompressed size in bytes.
    /// Default: 16_106_127_360 (15 GiB)
    pub max_total_uncompressed_bytes: u64,
    /// Maximum single entry uncompressed size in bytes.
    /// Default: 8_589_934_592 (8 GiB)
    pub max_single_entry_bytes: u64,
    /// Maximum nesting depth for nested archives.
    /// Default: 3
    pub max_nesting_depth: u32,
    /// Archive formats exempt from size and ratio checks (e.g. ISO disc images).
    /// Default: ["iso"]
    pub exempt_formats: Vec<String>,
}

impl Default for BombThresholds {
    fn default() -> Self {
        Self {
            max_compression_ratio: 100.0,
            max_total_uncompressed_bytes: 16_106_127_360, // 15 GiB
            max_single_entry_bytes: 8_589_934_592,        // 8 GiB
            max_nesting_depth: 3,
            exempt_formats: vec!["iso".to_string()],
        }
    }
}

/// Known archive extensions used for nested bomb detection.
const ARCHIVE_EXTENSIONS: &[&str] = &[
    "zip", "7z", "rar", "tar", "gz", "bz2", "xz", "cab",
];

/// Check if the archive format is exempt from size/ratio checks.
fn is_exempt(archive_ext: &str, thresholds: &BombThresholds) -> bool {
    let ext_lower = archive_ext.to_lowercase();
    thresholds.exempt_formats.iter().any(|e| e.to_lowercase() == ext_lower)
}

/// Check compression ratio: total uncompressed size / archive file size.
/// Skips if archive_file_size == 0 or format is exempt.
fn check_compression_ratio(
    entries: &[ArchiveEntryInfo],
    archive_file_size: u64,
    archive_ext: &str,
    thresholds: &BombThresholds,
) -> Result<(), LockError> {
    if archive_file_size == 0 {
        return Ok(());
    }
    if is_exempt(archive_ext, thresholds) {
        return Ok(());
    }

    let total_uncompressed: u64 = entries
        .iter()
        .filter(|e| e.size >= 0)
        .map(|e| e.size as u64)
        .sum();

    if total_uncompressed == 0 {
        return Ok(());
    }

    let ratio = total_uncompressed as f64 / archive_file_size as f64;
    if ratio > thresholds.max_compression_ratio {
        let limit = thresholds.max_compression_ratio;
        return Err(LockError::ZipBomb(format!(
            "此壓縮包的壓縮比為 {ratio:.0}x，遠超安全上限（{limit:.0}x），可能是壓縮炸彈，已拒絕開啟。"
        )));
    }

    Ok(())
}

/// Check total uncompressed size of all entries.
/// Skips entries with size == -1 and exempt formats.
fn check_total_size(
    entries: &[ArchiveEntryInfo],
    archive_ext: &str,
    thresholds: &BombThresholds,
) -> Result<(), LockError> {
    if is_exempt(archive_ext, thresholds) {
        return Ok(());
    }

    let total: u64 = entries
        .iter()
        .filter(|e| e.size >= 0)
        .map(|e| e.size as u64)
        .sum();

    let limit = thresholds.max_total_uncompressed_bytes;
    if total > limit {
        let size_gb = total as f64 / 1_073_741_824.0;
        let limit_gb = limit as f64 / 1_073_741_824.0;
        return Err(LockError::ZipBomb(format!(
            "此壓縮包解壓後總大小約為 {size_gb:.1} GB，超過安全上限（{limit_gb:.0} GB），已拒絕開啟。"
        )));
    }

    Ok(())
}

/// Check if any single entry exceeds the size limit.
/// Skips entries with size == -1 and exempt formats.
fn check_single_entry_size(
    entries: &[ArchiveEntryInfo],
    archive_ext: &str,
    thresholds: &BombThresholds,
) -> Result<(), LockError> {
    if is_exempt(archive_ext, thresholds) {
        return Ok(());
    }

    let limit = thresholds.max_single_entry_bytes;
    for entry in entries {
        if entry.size < 0 {
            continue;
        }
        let size = entry.size as u64;
        if size > limit {
            let size_gb = size as f64 / 1_073_741_824.0;
            let limit_gb = limit as f64 / 1_073_741_824.0;
            let entry_name = &entry.name;
            return Err(LockError::ZipBomb(format!(
                "壓縮包內的檔案「{entry_name}」解壓後大小約為 {size_gb:.1} GB，超過安全上限（{limit_gb:.0} GB），已拒絕開啟。"
            )));
        }
    }

    Ok(())
}

/// Check for nested archive entries that exceed the nesting depth limit.
/// Only inspects entry name extensions — does not recursively open inner archives.
fn check_nesting_depth(
    entries: &[ArchiveEntryInfo],
    current_depth: u32,
    thresholds: &BombThresholds,
) -> Result<(), LockError> {
    let has_nested = entries.iter().any(|e| {
        let name_lower = e.name.to_lowercase();
        ARCHIVE_EXTENSIONS.iter().any(|ext| name_lower.ends_with(&format!(".{ext}")))
    });

    if has_nested && current_depth >= thresholds.max_nesting_depth {
        let depth = current_depth;
        let limit = thresholds.max_nesting_depth;
        return Err(LockError::ZipBomb(format!(
            "此壓縮包包含 {depth} 層嵌套壓縮包，超過安全上限（{limit} 層），可能是遞迴型壓縮炸彈，已拒絕開啟。"
        )));
    }

    Ok(())
}

/// Run all Zip Bomb detection checks against the listed entries.
///
/// # Arguments
/// - `entries`: entries returned by `smart_list_entries()`
/// - `archive_file_size`: size of the archive file itself (bytes), used for ratio check
/// - `archive_ext`: lowercase file extension of the archive (e.g. "zip"), used for exemption
/// - `current_depth`: current nesting depth (1 = outermost archive)
/// - `thresholds`: detection thresholds
///
/// # Returns
/// `Ok(())` if safe; `Err(LockError::ZipBomb(msg))` if a threat is detected.
pub fn check_entries(
    entries: &[ArchiveEntryInfo],
    archive_file_size: u64,
    archive_ext: &str,
    current_depth: u32,
    thresholds: &BombThresholds,
) -> Result<(), LockError> {
    check_compression_ratio(entries, archive_file_size, archive_ext, thresholds)?;
    check_total_size(entries, archive_ext, thresholds)?;
    check_single_entry_size(entries, archive_ext, thresholds)?;
    check_nesting_depth(entries, current_depth, thresholds)?;
    Ok(())
}
