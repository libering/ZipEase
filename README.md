# ZipEase

> Windows 平台上的免費開源解壓縮工具，Unarchiver One 的開源替代品。

ZipEase 是一款輕量的 Windows 桌面解壓縮應用程式，採用 Fluent Design 介面，支援拖放操作與 CJK 編碼自動偵測，讓解壓縮變得簡單直覺。


## 功能特色

- 拖放壓縮檔即可預覽內容

- 支援目錄導航（雙擊進入子資料夾）

- 自動偵測 CJK 編碼（Shift-JIS、Big5、GBK、EUC-KR 等），告別亂碼

- 密碼保護壓縮檔支援（ZIP、7z）

- 解壓進度顯示

- Fluent Design 介面，支援 Dark mode 與 Mica backdrop

- 解壓過程中鎖定來源目錄，防止意外修改

## 支援格式

| 格式 | 說明 |
| - | - |
| ZIP | 含 CJK 編碼自動偵測 |
| TAR / TAR.GZ / TAR.BZ2 / TAR.XZ | 完整 TAR 系列 |
| 7z | 含密碼支援 |
| RAR | 透過內建 7za.dll |


## 系統需求

- Windows 10 / 11（x64）

- .NET 8 Runtime

## 從原始碼建置

**Rust 核心**

```
cd ZipEase.Core  
cargo build --release
```

**UI**

用 Visual Studio 2022 開啟 `ZipEase.slnx` 後建置即可。確保 `ZipEase.Core/target/release/zipease\_core.dll` 已存在。

## 授權

MIT License

