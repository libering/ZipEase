use ctor::ctor;
use log::LevelFilter;
use simplelog::{ConfigBuilder, WriteLogger};
use std::fs::OpenOptions;
use std::path::PathBuf;

/// DLL 載入時自動執行的初始化函數。
/// 架構決策 (Why): 由於 C# 透過 P/Invoke 呼叫 Rust FFI，進入點可能是任何一個導出函數。
/// 使用 #[ctor] 確保無論 C# 呼叫哪個 FFI 函數，Logger 都已在 DLL 載入記憶體時就緒，
/// 避免發生 "未初始化 Logger 就試圖寫入" 的靜默失敗。
#[ctor]
fn init_logging_on_load() {
    // 獲取 Windows 的 %TEMP% 目錄
    let temp_dir = std::env::temp_dir();
    
    // 構造帶有 PID 的 Log 檔案名稱，避免 ZipEase 多開時發生寫入衝突
    let pid = std::process::id();
    let log_path: PathBuf = temp_dir.join(format!("ZipEase_rust_{}.log", pid));

    // 以 Append 模式開啟或建立 Log 檔案
    let log_file = match OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        Ok(f) => f,
        Err(_) => return, // 防禦性編程: 如果無法建立日誌檔，靜默失敗以避免崩潰整個 DLL 載入過程
    };

    // 配置 SimpleLog
    let config = ConfigBuilder::new()
        .set_time_format_rfc3339() // 採用標準時間格式，方便與 C# 側的日誌對齊
        .build();

    // 初始化 Logger (設定為 Debug 級別，發布正式版時建議改為 Info)
    let _ = WriteLogger::init(LevelFilter::Debug, config, log_file);

    log::info!("=========================================");
    log::info!("ZipEase Rust Core (cdylib) Initialized.");
    log::info!("Process ID: {}", pid);
    log::info!("=========================================");
}