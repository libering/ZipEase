use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use zipease_shared::LockError;

use crate::extract::smart::smart_extract_with_progress;

/// 支援的壓縮檔副檔名集合（與 `smart::detect_backend` 一致）
const SUPPORTED_EXTENSIONS: &[&str] = &[
    // Zip family
    "zip", "apk", "ipa", "jar", "war", "ear",
    // Tar family (includes compound: .tar.gz, .tar.bz2, .tar.xz, .tar.zst)
    "tar", "gz", "bz2", "xz", "zst",
    // 7-Zip
    "7z",
    // RAR
    "rar",
    // Cabinet
    "cab",
    // ISO
    "iso",
];

/// 判斷路徑是否為支援的壓縮檔格式
///
/// 複用 `smart::detect_backend` 的副檔名判斷邏輯，包含：
/// - 單一副檔名：zip, 7z, rar, tar, gz, bz2, xz, zst, cab, iso, apk, ipa, jar, war, ear
/// - 分割壓縮檔：.001（如 archive.7z.001, archive.zip.001）
/// - 分割壓縮檔：.z01, .z02, ...（WinZip split）
fn is_supported_archive(path: &Path) -> bool {
    let ext = match path.extension().and_then(|s| s.to_str()) {
        Some(e) => e.to_lowercase(),
        None => return false,
    };

    // Check standard extensions
    if SUPPORTED_EXTENSIONS.contains(&ext.as_str()) {
        return true;
    }

    // Check split archive: .001
    if ext == "001" {
        return true;
    }

    // Check WinZip split: .z01, .z02, ... (starts with 'z' followed by digits)
    if ext.starts_with('z') && ext.len() > 1 && ext[1..].parse::<u32>().is_ok() {
        return true;
    }

    false
}

/// 過濾出支援的壓縮檔格式
///
/// 接受一組路徑，回傳僅包含 ZipEase 支援格式的子集。
/// 複用 `smart::detect_backend` 的副檔名判斷邏輯。
///
/// # 支援格式
/// zip, 7z, rar, tar, gz, bz2, xz, zst, cab, iso, apk, ipa, jar, war, ear
/// 以及分割壓縮檔（.001, .z01 等）
pub fn filter_supported_archives(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths
        .iter()
        .filter(|p| is_supported_archive(p))
        .cloned()
        .collect()
}

/// 批次作業中單一檔案的狀態
#[derive(Debug, Clone, PartialEq)]
pub enum ArchiveStatus {
    /// 尚未開始處理
    Pending,
    /// 正在解壓中
    Extracting,
    /// 解壓成功
    Success,
    /// 解壓失敗，附帶錯誤訊息
    Failed(String),
    /// 需要密碼
    PasswordRequired,
    /// 偵測為 Zip Bomb
    ZipBomb,
    /// 被跳過（取消時尚未處理的檔案）
    Skipped,
}

/// 整批作業的完整結果
#[derive(Debug)]
pub struct BatchResult {
    /// 每個壓縮檔的路徑與最終狀態
    pub results: Vec<(PathBuf, ArchiveStatus)>,
    /// 作業是否被使用者取消
    pub cancelled: bool,
    /// 成功解壓的檔案總數
    pub total_files_extracted: u32,
}

impl BatchResult {
    /// 回傳成功解壓的壓縮檔數量
    pub fn success_count(&self) -> u32 {
        self.results
            .iter()
            .filter(|(_, status)| matches!(status, ArchiveStatus::Success))
            .count() as u32
    }

    /// 回傳失敗的壓縮檔數量（包含 Failed、PasswordRequired、ZipBomb、Skipped）
    pub fn failure_count(&self) -> u32 {
        self.results
            .iter()
            .filter(|(_, status)| {
                matches!(
                    status,
                    ArchiveStatus::Failed(_)
                        | ArchiveStatus::PasswordRequired
                        | ArchiveStatus::ZipBomb
                        | ArchiveStatus::Skipped
                )
            })
            .count() as u32
    }
}

/// 批次進度回報的資料結構
pub struct BatchProgress {
    /// 當前檔案索引（0-based）
    pub archive_index: u32,
    /// 總檔案數
    pub archive_count: u32,
    /// 當前檔案進度 0-100
    pub file_percent: i32,
    /// 當前正在解壓的檔案名稱
    pub current_file_name: String,
}

/// 批次解壓核心函數
///
/// 依序對每個壓縮檔執行 `smart_extract_with_progress`，並透過 `progress_fn` 回報進度。
/// 每個檔案處理前檢查 `cancel_flag`，若已設定則將剩餘檔案標記為 `Skipped`。
/// 單一檔案的錯誤不會阻斷其餘檔案的處理。
pub fn batch_extract<F>(
    archives: &[PathBuf],
    output_dir: &Path,
    cancel_flag: &AtomicBool,
    progress_fn: F,
) -> BatchResult
where
    F: Fn(BatchProgress),
{
    let archive_count = archives.len() as u32;
    let mut results: Vec<(PathBuf, ArchiveStatus)> = Vec::with_capacity(archives.len());
    let mut cancelled = false;

    for (index, archive_path) in archives.iter().enumerate() {
        // Check cancel flag before processing each archive
        if cancel_flag.load(Ordering::Relaxed) {
            cancelled = true;
            // Mark all remaining archives as Skipped
            for remaining in &archives[index..] {
                results.push((remaining.clone(), ArchiveStatus::Skipped));
            }
            break;
        }

        let archive_index = index as u32;

        // Report initial progress for this archive (0%)
        let file_name = archive_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        progress_fn(BatchProgress {
            archive_index,
            archive_count,
            file_percent: 0,
            current_file_name: file_name.clone(),
        });

        // Wrap the per-file progress callback to include batch-level info
        let batch_progress_wrapper = |current: usize, total: usize, entry_name: &str| {
            let percent = if total > 0 {
                ((current as f64 / total as f64) * 100.0) as i32
            } else {
                0
            };

            progress_fn(BatchProgress {
                archive_index,
                archive_count,
                file_percent: percent.clamp(0, 100),
                current_file_name: entry_name.to_string(),
            });
        };

        // Execute extraction and map errors to ArchiveStatus
        let status = match smart_extract_with_progress(archive_path, output_dir, batch_progress_wrapper) {
            Ok(()) => {
                // Report 100% completion for this archive
                progress_fn(BatchProgress {
                    archive_index,
                    archive_count,
                    file_percent: 100,
                    current_file_name: file_name,
                });
                ArchiveStatus::Success
            }
            Err(err) => map_lock_error_to_status(err),
        };

        results.push((archive_path.clone(), status));
    }

    let total_files_extracted = results
        .iter()
        .filter(|(_, s)| matches!(s, ArchiveStatus::Success))
        .count() as u32;

    BatchResult {
        results,
        cancelled,
        total_files_extracted,
    }
}

/// Map a `LockError` to the corresponding `ArchiveStatus`
fn map_lock_error_to_status(err: LockError) -> ArchiveStatus {
    match err {
        LockError::PasswordRequired(_) => ArchiveStatus::PasswordRequired,
        LockError::ZipBomb(_) => ArchiveStatus::ZipBomb,
        other => ArchiveStatus::Failed(other.message()),
    }
}
