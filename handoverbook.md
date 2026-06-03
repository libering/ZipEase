# 📘 ZipEase Project Handover Book

> **Last Updated**: 2026-06-01
> **Status**: Feature Complete / **PHASE: Polish & Stabilization**

## 1. 項目概覽 (Project Overview)
**ZipEase** 是一個結合了「文件鎖定」與「智能解壓」功能的 Windows 桌面應用程式。
目標是完全復刻 Unarchiver One 的現代化流暢體驗（Fluent Design + 拖放核心）。

架構為「Holy Trinity」：
- **Rust cdylib** (`ZipEase.Core`) — 所有業務邏輯、解壓引擎、FFI 導出
- **C# WPF** (`ZipEase.UI`) — 純粹的 UI 皮膚，零業務邏輯
- **FFI (P/Invoke)** — 唯一的跨語言橋樑，僅傳遞原始型別與 UTF-16 指針

## 2. 當前狀態 (Current Status)

### 2.1 Core (Rust) — 🟢 完工
- [x] 核心解壓 (ZIP, TAR, 7z, CAB, ISO) — 可打開 & 解壓
- [x] **RAR 支援** — 改用 `unrar` crate（靜態連結 unrar C++ 源碼，不需要外部 DLL）；`RarBackend` 用 `read()` 模式手動寫檔，繞過 `extract_to` 的路徑問題
- [x] Directory Locking — 已實作並測試
- [x] FFI 解壓綁定 (`zip_ease_extract_*`) — 進度回調、Thread-safe
- [x] FFI 列表綁定 (`zip_ease_list_archive_contents` / `zip_ease_free_archive_entries`)
- [x] CJK 編碼偵測 (`chardetng` + `encoding_rs`) — UTF-8 優先，fallback 到 CJK 自動偵測
- [x] **7z 目錄過濾修復** — `list_entries` 過濾目錄條目，進度計數僅計算檔案
- [x] **TAR 目錄過濾修復** — 同上，並去除 `./` 前綴
- [x] **`ArchiveEntryInfo` struct** — `{ name, is_directory, size }` 用於 FFI 列表
- [x] **`LockError::PasswordRequired`** — 錯誤碼 `0x2004`
- [x] **`zip_ease_list_archive_contents_with_password`** — 密碼感知列表 FFI
- [x] **`zip_ease_extract_with_password`** — 密碼感知解壓 FFI（含進度回調）
- [x] **ZIP/7z 密碼支援** — `list_entries_info_with_password` + `extract_with_password_progress`
- [x] **`zip_ease_extract_force`** — 強行提取 FFI，使用 `by_index_raw` 繞過 CRC 驗證
- [x] **`zip_ease_extract_entry`** — 單條目提取 FFI，按索引提取單一檔案
- [x] **`zip_ease_extract_entry_by_name`** — name-based 單條目提取 FFI（非 ZIP 格式用）
- [x] **`zip_ease_free_string`** — 釋放 Rust 分配的 UTF-16 字串
- [x] **CAB 支援** — `CabBackend`，使用 `cab` crate (pure Rust)
- [x] **ISO 支援** — `IsoBackend`，純手工 ISO 9660 解析，支援 Joliet
- [x] **`zip_ease_trash_file`** — 移至資源回收桶 FFI
- [x] **`zip_ease_notify_success` / `zip_ease_notify_failure`** — WinRT toast notifications
- [x] **`zip_ease_who_locks`** — File lock detection FFI
- [x] **分割壓縮包支援** — `SevenZaDllBackendWithClsid`；`.001`, `.z01`~`.z09`
- [x] **APK/JAR/IPA 支援** — ZIP-based 格式
- [x] **命令列插件系統** — `PluginRegistry` + `PluginBackend` JSON Lines 協議
- [x] **`safe_join` 修復** — 拒絕 `..`、`RootDir`、`Prefix` 組件；修復 Windows `\\?\` 前綴導致的 `starts_with` 比對失敗
- [x] **FFI 錯誤碼修復** — `to_ffi_error()` 確保所有非 `0x2004` 錯誤碼為負數
- [x] **`simplelog` + `ctor` 整合** — DLL 載入時自動初始化 logger，寫到 `%TEMP%\ZipEase_rust_<timestamp>.log`
- [x] **Zip Bomb 偵測** — `bomb_detector.rs` 在 listing 階段偵測，`LockError::ZipBomb` + 錯誤碼 `0x2005`
- [x] **壓縮 Streaming 修復** — `io::copy` 取代 `read_to_end`，修復大檔案 OOM
- [x] **ZIP64 支援** — 檔案 ≥ 4 GB 自動啟用 `large_file(true)`
- [x] **多國語言 (i18n)** — `LocalizationManager`；繁中/英文
- [x] **7z 檔案大小修復** — `sevenz.rs` 讀取 `e.size`；`sevenzadll/backend.rs` 查詢 `KPID_SIZE`；目錄保持 `-1`
### 2.2 UI (C# WPF) — 🟢 完工
- [x] WPF-UI 4.0.0 (lepoco/wpf-ui) 整合
- [x] `App.xaml` — Mica 主題、Dark mode、WPF-UI 資源字典
- [x] `MainWindow.xaml` — `ui:FluentWindow`，Mica backdrop，拖放區域，DataGrid 預覽，進度條，InfoBar
- [x] `MainWindow.xaml.cs` — 最小化 code-behind，僅路由拖放事件
- [x] `MainWindowViewModel.cs` — 完整狀態機 (Idle/DragOver/Previewing/Extracting)，所有命令，`Dispatcher.BeginInvoke` 進度回調，錯誤處理
- [x] `ArchivePreviewService.cs` — 記憶體安全的 FFI 包裝器 (try/finally 保證釋放)
- [x] **CAB/ISO 支援** — `SupportedExtensions` 加入 `.cab`, `.iso`；UI 提示文字更新
- [x] `ArchiveEntry.cs` / `ArchiveEntryViewModel.cs` — POCO + ObservableObject
- [x] `UIState.cs` — 4 狀態枚舉
- [x] `NativeMethods.cs` — 所有 P/Invoke 聲明（含 `ExtractForce`, `ExtractEntry`, `FreeString`）
- [x] **目錄導航** — `NavigateIntoCommand` (雙擊進入)，`NavigateBackCommand` (上一頁)，`_navigationStack`
- [x] **文件數量顯示** — `FileCount` 屬性，toolbar TextBlock，僅計算非目錄條目
- [x] **密碼保護支援** — `PasswordDialog`，3 次重試限制，`_pendingPassword` 狀態管理
- [x] **強行提取 (Force Extract)** — toolbar CheckBox 綁定 `ForceExtract`，呼叫 `ExtractForceAsync`，忽略 CRC 錯誤
- [x] **單體檔案拖出** — DataGrid `PreviewMouseMove` → `ExtractSingleEntryCommand` → 提取到 temp → `DragDrop.DoDragDrop`
- [x] **安全刪除 (Safe Delete)** — InfoBar `ActionButton` 「移至資源回收桶 ♻️」，`TrashSourceCommand`，樂觀停用防止二次點擊，無永久刪除選項
- [x] **Toast Notifications** — `NotifySuccessAsync` / `NotifyFailureAsync` wrappers in `ExtractionManager.cs`; fire-and-forget `_ =` pattern; called after `ShowSuccess` / `ShowError` in `MainWindowViewModel`
- [x] **File Lock Detector** — `WhoLocksAsync` wrapper in `ExtractionManager.cs`; access-denied detection in `ExtractionException` catch block; replaces generic error with "X is using this file. Close it and try again."; `FreeString` in `finally` block
- [x] **側邊欄導航** — 頂部 Tab 改為左側 160px 側邊欄，解壓縮 / 壓縮 / 設定三個項目，選中項 accent 高亮
- [x] **設定頁面** (`AppSettings` + `SettingsView` + `SettingsViewModel`) — 持久化到 `%AppData%\ZipEase\settings.json`；設定項：強制提取、解壓後自動清理、檔案佔用偵測、任務完成通知、介面主題（跟隨系統/淺色/深色）
- [x] **設定頁插件清單與管理** — 設定頁「已安裝插件」美化為卡片清單；新增「開啟插件資料夾」與「重新整理」按鈕；支援在免重啟下動態掃描與載入插件；無插件時顯示精美提示。
- [x] **設定接線** — `Extract()` 讀取 `LastOutputDir` 記憶上次路徑；`ToastNotifications` / `AutoTrashAfterExtract` / `LockDetection` / `ForceExtract` 全部從 `AppSettings.Instance` 讀取
- [x] **解壓縮選取項目** — `ExtractSelectedCommand` 接受 `DataGrid.SelectedItems`，code-behind `OnExtractSelectedClick` 傳入；toolbar 加「解壓縮選取」按鈕
- [x] **搜尋/過濾** — `SearchText` 屬性即時過濾 DataGrid，toolbar 下方搜尋框，`ClearButtonEnabled`
- [x] **欄位排序** — DataGrid 加 `CanUserSortColumns="True"` + `SortMemberPath`，點擊欄位標題排序
- [x] **壓縮頁拖放** — `CompressViewModel.AddDroppedFilesCommand`；`MainWindow.xaml.cs` 加 `OnCompressDragEnter/Over/Drop`；`CompressView` 加 `AllowDrop`
- [x] **雙擊預覽** — `PreviewEntryCommand`；ZIP 走 index-based；非 ZIP 走 name-based (`zip_ease_extract_entry_by_name`)；stable preview dir（同壓縮包共用目錄）；背景預提取所有圖片（ZIP 並發 3，RAR/7z 整包一次）；圖片檢視器可直接「下一張」導航
- [x] **分割壓縮包支援** — `IsSupportedArchive` 加入 `.001`, `.z01`~`.z09`；UI 提示文字更新
- [x] **壓縮密碼支援** — `zip` crate `aes-crypto` feature；`CompressOptions.password`；`zip_ease_compress` FFI 加 `password_ptr` 參數；`CompressViewModel` 加 `UsePassword` / `Password` / `IsPasswordSupported`；UI 密碼欄位（ZIP only）
- [x] **多國語言 (i18n)** — `Strings.resx` / `Strings.zh-TW.resx` / `Strings.en.resx`；`LocalizationManager` 實作 `INotifyPropertyChanged`，所有 XAML 用 `{Binding Source={x:Static core:L.Current}, Path=...}`；語言切換即時生效；設定頁加語言選擇器
- [x] **動態主題系統 (Dynamic Theming)** — `ThemeLoader` singleton：掃描 `%AppData%\ZipEase\themes\` 載入自訂 XAML ResourceDictionary，FileSystemWatcher hot-reload（300ms debounce）
- [x] **背景材質切換 (Backdrop Switcher)** — `BackdropSwitcher` 靜態工具類：Mica/Acrylic/None 即時切換，OS 版本偵測 + fallback
- [x] **SVG 圖示包 (Icon Resolver)** — `IconResolver` singleton：`%AppData%\ZipEase\icons\` 中的 SVG 替換內建圖示，Svg.Skia 渲染 + DPI 縮放，ConcurrentDictionary 快取
- [x] **Zip Bomb 設定 UI** — Settings 頁面加入閾值設定（最大總大小、單檔大小、壓縮比、嵌套深度），可重置預設值
- [x] **右鍵選單整合 (Context Menu)** — `ZipEase.ShellExtension` NativeAOT DLL；`IExplorerCommand` COM 介面；「用 ZipEase 解壓縮」/「用 ZipEase 壓縮」；Windows 11 Sparse MSIX + Windows 10 Registry fallback；設定頁狀態顯示與重新註冊/停用按鈕；`CommandLineParser` 處理 `--compress`/bare paths/`--register-shell`/`--unregister-shell`
- [x] **命令列啟動** — `CommandLineParser`：bare paths → Extract mode，`--compress` → Compress mode，`--register-shell`/`--unregister-shell` → 註冊/反註冊後退出

### 2.3 Specs — 🟢 全部完成
- `file-locking-poc` — ✅ 完成
- `ui-integration` — ✅ 完成
- `ui-overhaul` — ✅ Tasks 1–8 全部完成
- `sevenzip-backend` — ✅ 完成（RAR via 7za.dll）
- `sevenz-list-fix` — ✅ 完成（7z 目錄過濾 + 進度修復）
- `ui-enhancements` — ✅ 完成（目錄導航、文件數量、密碼支援）
- `zip-encoding` — ✅ 完成（CP932/CJK 編碼偵測）
- `archive-compression` — ✅ 完成（ZIP/7z/TAR.GZ 壓縮）
- `safe-delete-trash` — ✅ 完成（移至資源回收桶，ADHD 友好：無後悔陷阱，唯一安全選項）
- `toast-notifications` — ✅ 完成（Windows WinRT toast，成功/失敗通知，Open Folder 按鈕）
- `file-lock-detector` — ✅ 完成（access denied 時顯示佔用程式名稱，`wholock` crate，PBT 測試）
- `preview-entry-fix` — ✅ 完成（非 ZIP name-based 提取、遞迴搜尋 temp、UUID 唯一目錄、APK 提示）
- `zip-bomb-protection` — ✅ 完成（listing 階段偵測 zip bomb，`0x2005` 錯誤碼，Settings UI 閾值設定）
- `compress-error-fix` — ✅ 完成（streaming 壓縮修復 OOM、ZIP64 大檔案支援、UTF-8 錯誤訊息修復）
- `dynamic-theming` — ✅ 完成（自訂 XAML 主題 hot-reload、Mica/Acrylic 材質切換、SVG 圖示包替換）
- `ui-and-listing-polish` — ✅ 完成（7z 檔案大小修復 + 淺色模式對比度修復）
- `context-menu` — ✅ 完成（Windows 右鍵選單整合：Shell Extension NativeAOT DLL、Sparse MSIX + Registry fallback、設定頁管理）
- `image-preview-plugin` — ✅ 完成（壓縮包內圖片預覽：縮放、平移、導航、縮圖、LRU 快取、PBT 測試）
- `archive-repair` — ✅ 已完成（損壞修復演算法：ZIP/RAR header 重建、Central Directory 重建、CRC 重算）
- `archive-search` — ✅ 已完成（壓縮檔內搜尋：Glob/子字串搜尋、FFI 介面、C# 搜尋框整合）
- `batch-extraction` — ✅ 已完成（批次解壓：多檔案拖放、解壓佇列、FFI 介面、C# 批次解壓管理）
- `codebase-cleanup` — ✅ 已完成（代碼清理：刪除冗餘模組、依賴優化、單元與屬性測試整理、FFI 異常安全 wrap）

## 3. 已修復的 Bug

### Bug 1: RAR 無法打開
- **根本原因**: 早期嘗試使用 `SevenZaDllBackend` 透過 `7za.dll` 解壓 RAR，但 standalone `7za.dll` 不支援 RAR 解壓所需的 CLSID，導致開啟 RAR 失敗。
- **解決方案**: 改用 `unrar` crate（靜態連結 unrar C++ 源碼，不需要外部 DLL）。為解決 Windows 路徑穿越與斜線相容問題，`RarBackend` 採用 `read()` 模式讀取位元組手動寫檔，繞過 `extract_to` 的限制。
- **相關檔案**: [rar.rs](file:///d:/vibe%20coding/zipease/ZipEase.Core/zipease-extract/src/extract/rar.rs)

### Bug 2: 7z 列表顯示目錄條目 + 進度計數錯誤
- **根本原因**: `sevenz.rs` 的 `list_entries` 未過濾目錄，`extract_with_progress` 的 `total` 包含目錄數
- **解決方案**: 加 `.filter(|e| !e.is_directory())`；`total` 改用過濾後的 `.count()`
- **相關檔案**: [sevenz.rs](file:///d:/vibe%20coding/zipease/ZipEase.Core/zipease-extract/src/extract/sevenz.rs)

### Bug 3: TAR 列表顯示目錄條目 + `./` 前綴
- **根本原因**: `tar.rs` 的 `list_entries` 未過濾目錄，路徑帶有 `./` 前綴
- **解決方案**: 加 `entry_type().is_dir()` 過濾；`trim_start_matches("./")` 去除前綴
- **相關檔案**: [tar.rs](file:///d:/vibe%20coding/zipease/ZipEase.Core/zipease-extract/src/extract/tar.rs)

### Bug 4: 7z 檔案大小顯示為 "—"（已修正）
- **根本原因**: `sevenz.rs` 和 `sevenzadll/backend.rs` 的 `list_entries_info` 硬編碼 `size: -1`，未讀取實際解壓大小
- **解決方案**:
  - `sevenz.rs`：改用 `e.size as i64`（目錄保持 `-1`）
  - `sevenzadll/backend.rs`：新增 `KPID_SIZE` (property ID 7) 查詢，從 `PROPVARIANT` 的 `VT_UI8` 取得 `u64` 值
  - `list_entries_info_with_password` 同步修復
- **相關檔案**: `ZipEase.Core/zipease-extract/src/extract/sevenz.rs`, `ZipEase.Core/zipease-extract/src/extract/sevenzadll/backend.rs`, `ZipEase.Core/zipease-extract/src/extract/sevenzadll/types.rs`
- **PBT 驗證**: bug condition test + preservation test（ZIP 大小不變、目錄旗標不變、檔名不變）

### Bug 5: 設定頁插件清單未正確顯示或顯示空白
- **根本原因**: `SettingsView.xaml` 將 `ItemsControl` 直接作為 `ui:CardControl` 的 Content 屬性，且缺乏「無插件」提示及開啟資料夾/重新整理按鈕；此外，若 `%AppData%\ZipEase\plugins\` 中缺少對應插件的執行檔，`PluginRegistry` 會將其過濾導致清單為空，造成使用者無法查看與重新整理。
- **解決方案**: 
  1. 將 `PluginRegistry.PluginsDir` 公開。
  2. 在 `LoadedPlugin` 新增 `DisplayExtensions` 與 `DisplayCapabilities` 屬性。
  3. 在 `SettingsViewModel` 實作 `OpenPluginsFolderCommand` 與 `ReloadPluginsCommand`，並提供 `HasPlugins`/`HasNoPlugins` 狀態。
  4. 重構 `SettingsView.xaml`，將插件管理拆分為獨立卡片，支援「開啟資料夾」與「重新整理」；下方使用 ItemsControl 動態載入精美卡片式插件清單，並在無插件時顯示 Info 圖示與引導文字。
- **相關檔案**: [PluginRegistry.cs](file:///d:/vibe%20coding/zipease/ZipEase.UI/Core/Plugin/PluginRegistry.cs), [SettingsViewModel.cs](file:///d:/vibe%20coding/zipease/ZipEase.UI/Core/SettingsViewModel.cs), [SettingsView.xaml](file:///d:/vibe%20coding/zipease/ZipEase.UI/Core/SettingsView.xaml)

## 4. 待辦事項 (Next Steps)

### 立即執行
1. **GitHub Push** — 所有功能已完成，可以提交

### 後續功能 (可選)
- 自動更新 — GitHub Releases API + Squirrel.Windows
- 解壓縮歷史記錄 — 記住最近 20 個壓縮檔路徑，設定頁可清除
- 批次解壓縮 — 一次拖入多個壓縮檔，全部解壓到同一目錄

### 插件扩展生态 (规划中) — 📝 Spec 已完成

**Spec 位置**: `.kiro/specs/official-plugin-pack/`

**目标**：常用格式保持内建（ZIP、7z、RAR、TAR 系列、CAB、ISO），其他格式通过插件扩展

**规划格式**：
- 经典压缩格式：ACE (.ace)、ARJ (.arj)、LHA/LZH (.lha/.lzh)
- 单文件压缩：XZ (.xz)、LZMA (.lzma)、LZ4 (.lz4)、Zstandard (.zst)
- 映像格式：WIM (.wim)、DMG (.dmg)、VHD/VHDX (.vhd/.vhdx)

**实现方式**：混合方案
- 7za.dll COM 接口：XZ、LZMA、WIM、VHD/VHDX
- Rust 原生：LZ4、Zstandard
- Python + 外部工具：ACE、ARJ、LHA、DMG

**发布方式**：
- semantic-release 自动化发布
- GitHub Releases ZIP 压缩档
- 用户手动放置到 `%AppData%\ZipEase\plugins\`

**状态**：
- [x] Requirements 文档完成（8 个需求）
- [x] Design 文档完成（架构图、组件设计、安全考量、semantic-release 配置）
- [x] Tasks 文档完成（6 阶段、24 任务、预估 34-48 小时）
- [x] Phase 1–6 完整實作（包含 7za.dll COM 插件、LZ4/Zstd 原生 Rust 插件、ACE/ARJ/LHA/DMG Python 適配器、C# 整合與 Fallback 機制、PowerShell 打包腳本、semantic-release 與 GitHub Actions 設定）


**关键文件**：
- `.kiro/specs/official-plugin-pack/requirements.md`
- `.kiro/specs/official-plugin-pack/design.md`
- `.kiro/specs/official-plugin-pack/tasks.md`

### 压缩包图片预览插件 — ✅ 已完成

**Spec 位置**: `.kiro/specs/image-preview-plugin/`

**功能**：在压缩包内直接预览图片，支持缩放、平移、导航

**实现**：
- **Rust 后端** (`zipease-preview` crate)：
  - 图片解码：PNG、JPEG、GIF、BMP、WebP、TIFF、ICO
  - Magic bytes 验证（防止伪造副档名）
  - LRU 快取（256 MB 容量）
  - 缩图生成（64×64，maintaining aspect ratio）
  - 自然排序导航
  - 安全限制：100 MB 档案大小、16384×16384 解析度、10 秒解码逾时、512 MB 记忆体上限
- **C# 前端**：
  - `PreviewPanel.xaml` + `PreviewViewModel.cs`
  - `ZoomPanService.cs`（缩放/平移逻辑）
  - `NavigationService.cs`（目录范围导航）
  - `ThumbnailService.cs`（缩图显示整合）
  - 键盘支援：←/→ 导航、Esc 关闭

**PBT 测试**：
- `magic_bytes_pbt.rs` — 魔术字节验证正确性
- `rgba_buffer_pbt.rs` — RGBA 缓冲区大小正确性
- `thumbnail_pbt.rs` — 缩图长宽比保持
- `navigation_filter_pbt.rs` — 目录范围导航过滤
- `safe_join_pbt.rs` — 路径穿越拒绝

**状态**：
- [x] Requirements 文档完成（11 个需求）
- [x] Design 文档完成
- [x] Tasks 文档完成（17 阶段、全部完成）
- [x] Rust 后端实现完成
- [x] C# 前端实现完成
- [x] PBT 测试通过

**关键文件**：
- `ZipEase.Core/zipease-preview/` (Rust crate)
- `ZipEase.UI/Core/ThumbnailService.cs`
- `ZipEase.UI/Core/PreviewViewModel.cs`
- `ZipEase.UI/Core/ZoomPanService.cs`

### 🟢 已解決問題 / Bug 修復確認

#### 1. Rust log 輸出（✅ 已確認）
- **結果**：`%TEMP%\ZipEase_rust_*.log` 正常寫入，包含 logger 初始化、列表操作、預覽提取等記錄

#### 2. APK 內部檔案預覽（✅ 已確認）
- **結果**：`um.apk` 正常打開，顯示 229 個條目，根層 3 個檔案（AndroidManifest.xml、classes.dex、resources.arsc），大小正確

#### 3. 7z 單條目預覽（✅ 已確認）
- **結果**：`7z2600-extra.7z` 雙擊 `License.txt` 成功提取並用系統關聯程式打開

#### 4. 淺色模式對比度不足（✅ 已修正）
- **症狀**：淺色模式下 CardControl 背景和視窗背景幾乎相同（都是白色），UI 元素之間缺乏對比度，看不清邊界
- **修正**：`App.xaml` 加入 `CardBackgroundFillColorDefaultBrush` = `#F5F5F5`；`App.xaml.cs` 動態管理（Light 加入、Dark 移除）

#### 5. 錯誤訊息亂碼（✅ 已修正）
- **原因**：`zip_ease_get_last_error` 回傳 UTF-8 C string，但 C# 用 `Marshal.PtrToStringUni`（UTF-16）讀取
- **修正**：改用 `Marshal.PtrToStringUTF8`

#### 6. DataGrid 空白（✅ 已修正）
- **原因**：`LoadArchive` 在 UI thread 同步呼叫 FFI，大檔案 block UI；`RefreshEntriesForCurrentPath` 過濾後根層為空
- **修正**：`LoadArchiveAsync` + `Task.Run`；flat fallback 顯示所有檔案

### 視覺圖示強化（規劃中）
- 應用程式圖示 (`.ico`) — 用 AI 生成後在 csproj 設定 `<ApplicationIcon>`
- DropZone 空白狀態插圖 — 換成自訂 SVG/PNG（目前用 WPF-UI `SymbolIcon`）
- DataGrid 檔案類型圖示 — 每行左側加小圖示（目前用 `SymbolIcon`，可換成自訂 PNG）
- 整合方式：PNG 放 `ZipEase.UI/Assets/`，XAML 用 `<Image Source="/Assets/xxx.png"/>`

#### 圖示資源推薦（符合開源授權）

| 圖示包 | 授權 | 數量 | 格式 | 來源 |
|--------|------|------|------|------|
| **Fluent System Icons** ⭐ 首選 | MIT License | 4,000+ | SVG, PNG | [github.com/microsoft/fluentui-system-icons](https://github.com/microsoft/fluentui-system-icons) |
| **Segoe Fluent Icons Font** | Microsoft 軟體授權條款 | Windows 內建 | 字型 | Windows 系統內建（Windows 11+） |
| **Material Design Icons** | Apache 2.0 / Pictogrammers Free License | 7,000+ | SVG, PNG | [pictogrammers.com/library/mdi](https://pictogrammers.com/library/mdi/) |
| **MahApps.Metro.IconPacks** | MIT License | 多種圖示包 | WPF 原生 | [github.com/MahApps/MahApps.Metro.IconPacks](https://github.com/MahApps/MahApps.Metro.IconPacks) |

**使用建議**：
1. **Fluent System Icons** — 與 WPF-UI 4.x 的 Fluent 設計語言完全匹配，下載 SVG 檔案
2. **Segoe Fluent Icons** — Windows 內建字型，XAML 直接引用，零相依性
3. **MahApps.Metro.IconPacks** — NuGet 套件 (`Install-Package MahApps.Metro.IconPacks`)，WPF 原生支援

**相關圖示關鍵字**：
- `archive`, `folder`, `document`, `arrow-download` (Fluent)
- `archive`, `folder-zip`, `file-compress`, `folder-arrow-down` (Material)

### 插件系統 (Format Plugin System) — ✅ 已完成
- 插件為任意可執行檔，放在 `%AppData%\ZipEase\plugins\{name}\`，附帶 `plugin.json`
- `PluginRegistry` 掃描目錄載入 metadata，`PluginBackend` 用 JSON Lines 協議通訊
- 整合到 `ArchivePreviewService.IsSupportedArchive()` 和解壓流程
- 設定頁顯示已安裝插件列表
- `docs/plugin-example/` 提供 Python 範例插件

### UI 主題系統 — ✅ 已完成
- 自訂 XAML ResourceDictionary 主題，放在 `%AppData%\ZipEase\themes\`
- FileSystemWatcher hot-reload（2 秒內生效）
- Mica / Acrylic / None 背景材質切換
- SVG 圖示包替換（`%AppData%\ZipEase\icons\`）
- 設定頁 Appearance 區段整合
- NuGet: `Svg.Skia` + `SkiaSharp.Views.WPF`

### 多國語言 (i18n) — ✅ 已完成
- `.resx` 資源檔：`Strings.zh-TW.resx`, `Strings.en.resx`
- `LocalizationManager` 實作 `INotifyPropertyChanged`，語言切換即時生效
- 設定頁語言選擇器

### 右鍵選單整合 (Context Menu) — ✅ 已完成
- `ZipEase.ShellExtension` NativeAOT DLL — `IExplorerCommand` COM 介面
- 「用 ZipEase 解壓縮」（壓縮檔右鍵）+ 「用 ZipEase 壓縮」（任意檔案/資料夾右鍵）
- Windows 11: Sparse MSIX 註冊；Windows 10: Registry fallback
- 首次啟動自動註冊，設定頁可管理（重新註冊/停用）
- `CommandLineParser` 處理命令列參數啟動對應模式
- 8 項 PBT + 單元測試 + 整合測試

### 其他自定義功能接口 — 規劃中
- **自動更新** — GitHub Releases API 版本檢查 + `Squirrel.Windows`，背景靜默更新
- **批次解壓縮** — ✅ 已完成（`BatchExtractionManager` + Rust `batch_extract`）
- **解壓縮歷史記錄** — 📝 Spec 已完成

**Spec 位置**: `.kiro/specs/extraction-history/`

**目標**：記住最近 20 個壓縮檔路徑，設定頁顯示歷史記錄

**功能**：
- 自動記錄成功解壓縮的壓縮檔路徑
- 設定頁顯示歷史記錄列表（檔名 + 相對時間）
- 點擊項目直接開啟壓縮檔
- 檔案不存在時灰色標示
- 一鍵清除（需確認對話框）

**實現方式**：純 C# 端，整合到 `AppSettings`

**狀態**：
- [x] Requirements 文档完成（6 个需求）
- [x] Design 文档完成（資料模型、ViewModel、UI 設計）
- [x] Tasks 文档完成（8 階段、16 任務、預估 5 小時）
- [ ] 實现待开始

**關鍵文件**：
- `.kiro/specs/extraction-history/requirements.md`
- `.kiro/specs/extraction-history/design.md`
- `.kiro/specs/extraction-history/tasks.md`

### 損壞修復演算法 (Archive Repair) — ✅ 已完成

**Spec 位置**: `.kiro/specs/archive-repair/`

**功能**：診斷並修復損壞的 ZIP/RAR 壓縮包

**實現**：
- **Rust 後端** (`zipease-extract/src/repair/`)：
  - `ZipScanner`：掃描 Local File Header 簽名、解析 EOCD、診斷 Central Directory 損壞
  - `ZipRepairer`：重建 Central Directory、重算 CRC-32、寫入修復後檔案
  - `RarScanner`：檢測 RAR4/RAR5 標記、驗證 Header CRC
  - `RarRepairer`：修復標記塊、重建 Archive Header
  - `RepairEngine`：格式檢測與路由入口
- **FFI 接口**：
  - `zip_ease_diagnose_archive`：診斷損壞類型，返回 JSON 結果
  - `zip_ease_repair_archive`：執行修復，支持進度回調
  - `zip_ease_free_diagnosis`：釋放記憶體
- **C# 前端**：
  - `RepairService.cs`：P/Invoke 包裝、JSON 反序列化
  - `MainWindowViewModel` 整合：修復流程狀態機
  - UI：InfoBar「文件損壞，點擊修復」→ 進度 → 結果顯示

**修復類型**：
- ZIP Central Directory 缺失/損壞 → 從 Local File Headers 重建
- ZIP CRC 錯誤 → 重算或使用 extract_force 繞過
- RAR 標記塊損壞 → 重建正確標記
- RAR Header CRC 錯誤 → 重新計算

**PBT 測試**：
- DamageReport 序列化 round-trip
- LFH signature scanner 完整性
- CD 重建保留 LFH metadata
- CRC-32 重算正確性
- 修復後 ZIP 可開啟
- 非破壞性修復保證

**狀態**：
- [x] Requirements 文檔完成（12 個需求）
- [x] Design 文檔完成
- [x] Tasks 文檔完成（12 階段、全部完成）
- [x] Rust 後端實現完成
- [x] C# 前端實現完成
- [x] PBT 測試通過

**關鍵文件**：
- `ZipEase.Core/zipease-extract/src/repair/mod.rs`
- `ZipEase.Core/zipease-extract/src/repair/zip_scanner.rs`
- `ZipEase.Core/zipease-extract/src/repair/zip_repairer.rs`
- `ZipEase.Core/zipease-extract/src/repair/rar_scanner.rs`
- `ZipEase.Core/zipease-extract/src/repair/rar_repairer.rs`
- `ZipEase.UI/Core/RepairService.cs`

### 壓縮格式擴展包 (規劃中) — 📝 Spec 已完成

**Spec 位置**: `.kiro/specs/archive-format-extensions/`

**目標**：擴展支援 BZIP2 單檔、XZ 單檔、LZ4、Zstandard 單檔等純壓縮格式，以及複合歸檔格式（.tar.lz4, .tar.zst），並建立統一的格式註冊與路由機制。

**功能**：
- 統一的格式註冊機制 (Format Registry)，以執行緒安全方式管理格式後端
- 整合成熟的 Rust crate 支援單檔解壓與壓縮 (BZIP2, XZ, LZ4, Zstandard)
- 支援 TAR 複合格式解壓與壓縮，且在解壓時以串流直接傳入解析器，保證記憶體安全
- 壓縮與解壓 UI 整合，自動偵測單檔格式限制並提供一鍵切換複合格式按鈕
- 提供完整的 Round-Trip 屬性測試驗證

**狀態**：
- [x] Requirements 文檔完成（12 個需求）
- [ ] Design 文檔待開始
- [ ] Tasks 文檔待開始
- [ ] 實作待開始

**關鍵文件**：
- `.kiro/specs/archive-format-extensions/requirements.md`

## 5. 關鍵技術決策與踩坑記錄


### 架構邊界 (嚴格執行)
- **零業務邏輯在 C#** — 所有邏輯在 Rust，C# 只做 UI 綁定
- **FFI 邊界** — 僅傳遞原始型別 (`int`, `long`) 和 UTF-16 指針 (`*const u16`, `*mut u16`)
- **記憶體管理** — `zip_ease_free_archive_entries` 和 `zip_ease_free_string` 必須在 `finally` 塊中呼叫

### 線程安全
- Rust callback 運行在後台線程，更新 UI 必須使用 `Dispatcher.BeginInvoke`
- FFI 回調委託必須用 `GCHandle.Alloc` 防止被 GC 回收

### WPF-UI 版本
- **必須使用** `lepoco/wpf-ui` 4.x (NuGet: `WPF-UI`)
- **嚴禁使用** 任何 3.x 版本或中文特供 fork

### CJK 編碼
- Rust 側使用 `chardetng` 自動偵測，UTF-8 優先
- C# 側使用 `Marshal.PtrToStringUni` 讀取 UTF-16 指針，無需額外處理

### 7za.dll COM 介面
- `SevenZaDllBackend` 透過 `CreateObject` 導出函數取得 `IInArchive` COM 物件
- GUID 常數硬編碼（RAR5: `{23170F69-40C1-278A-1000-000110CC0000}`）
- DLL 路徑解析：先嘗試 `GetModuleHandleW` 取得 exe 目錄，fallback 到當前目錄

### 強行提取 (Force Extract)
- `ZipBackend.extract_force_progress` 使用 `by_index_raw` 繞過 CRC 驗證
- 最佳努力模式：跳過無法讀取的條目，不中斷整個提取流程
- 僅支援 ZIP 格式（7z/RAR 的損壞修復需要不同策略）

### 單體檔案拖出
- `ExtractSingleEntryCommand` 提取到 `%TEMP%/ZipEase_drag_{guid}/` 臨時目錄
- 使用 `DataFormats.FileDrop` + `DragDrop.DoDragDrop` 啟動 shell 拖放
- 拖放完成後自動清理臨時目錄（best-effort，失敗不報錯）
- `_allEntries` 的索引與 Rust 側的 `entry_index` 必須一致（flat list，不受導航過濾影響）

### 安全刪除 (Safe Delete / Trash)
- `zip_ease_trash_file` 使用 `trash::delete()` — 絕不呼叫 `std::fs::remove_file`
- 樂觀停用：點擊後立即 `IsTrashButtonEnabled = false`，防止 ADHD 用戶二次點擊
- 失敗時恢復按鈕啟用狀態，顯示 plain-language 錯誤（無錯誤碼）
- `trash::Error` 全部 variant 映射到中文友好訊息，不暴露內部型別名稱

### Toast 通知 (Toast Notifications)
- `zip_ease_notify_success` / `zip_ease_notify_failure` 使用 `windows-rs` WinRT APIs 直接構建 XML toast
- AUMID `"ZipEase.App"` 自動寫入 `HKCU\Software\Classes\AppUserModelId\ZipEase.App`（已存在則跳過）
- "Open Folder" 按鈕使用 `activationType="protocol"` + `explorer.exe {path}` 參數
- 所有錯誤靜默丟棄 — 通知是 best-effort，絕不因通知失敗而崩潰
- C# 側使用 `_ =` discard pattern 明確表達 fire-and-forget 意圖

### 命令列插件系統 (CLI Plugin System)
- 插件放在 `%AppData%\ZipEase\plugins\{name}/`，附帶 `plugin.json` 描述檔
- `PluginRegistry` 啟動時掃描目錄，載入 `PluginManifest`（名稱、版本、副檔名、可執行檔路徑）
- `PluginBackend` 用 `Process.Start` 呼叫插件，stdin 傳 JSON 請求，stdout 讀 JSON Lines 回應
- 協議：`{"action":"list","path":"..."}` / `{"action":"extract","path":"...","output":"..."}` → `{"status":"ok","entries":[...]}` / `{"status":"progress","pct":42,"file":"..."}` / `{"status":"done","count":5}`
- `ArchivePreviewService.IsSupportedArchive` 和 `ListArchiveContentsWithPassword` 整合插件 fallback
- 設定頁顯示已安裝插件列表（名稱、版本、描述）
- `docs/plugin-example/` 提供 Python 範例插件

### 多國語言 (i18n)
- `LocalizationManager` 實作 `INotifyPropertyChanged`，`RaiseAllChanged()` 觸發所有 XAML binding 即時更新
- 所有 XAML 字串用 `{Binding Source={x:Static core:L.Current}, Path=...}`，不再用 MarkupExtension
- 語言切換在設定頁即時生效，不需重啟
- `L` 靜態類別作為 `x:Static` 的短別名
- `SevenZaDllBackendWithClsid(GUID)` — 帶 CLSID 參數的 7za.dll 後端，複用所有 COM 邏輯
- `CLSID_7Z_HANDLER` / `CLSID_ZIP_HANDLER` 加入 `sevenzadll.rs`
- `detect_split_backend()` 根據 stem 副檔名（`.7z.001` → 7z，`.zip.001` → ZIP，`.rar.001` → RAR）選擇 CLSID
- `.z01`~`.z09` (WinZip split) 直接路由到 ZIP CLSID
- C# `IsSupportedArchive` 加入這些副檔名，拖放和瀏覽都能識別

### 設定系統 (AppSettings)
- `AppSettings` 繼承 `ObservableObject`，`System.Text.Json` 持久化，singleton pattern
- 所有預設值遵循 ADHD-friendly 原則（強制提取關、自動清理關、通知開、鎖定偵測開）
- `SmartUnpack` / `SmartEncoding` 從設定頁移除 — 這是 ZipEase 的核心特徵，不提供關閉選項
- `LastOutputDir` 記憶上次解壓縮路徑，`FolderBrowserDialog` 自動預填
- 新增欄位：`backdropType` (int, 0/1/2)、`activeThemeFile` (string)、`ZipBombThresholds` (nested object)

### 側邊欄導航
- `SetActiveNav(NavPage)` 統一管理三個面板的 Visibility + accent 高亮
- `NavPage` enum: `Extract`, `Compress`, `Settings`
- 設定頁在側邊欄底部，解壓縮/壓縮在頂部

### 動態主題系統 (Dynamic Theming)
- **ThemeLoader** — singleton，持有 `FileSystemWatcher` + `Dictionary<string, ResourceDictionary>`
- 自訂主題以 overlay 方式載入 `MergedDictionaries` 尾端，WPF 自動覆蓋同名 key
- 移除自訂字典後預設值自動恢復，不需要手動還原
- Hot-reload 使用 300ms debounce + `ConcurrentQueue<FileSystemEventArgs>` + `Dispatcher.Invoke`
- 無效 XAML 保留先前有效版本，不 crash
- **BackdropSwitcher** — static helper，`IOsVersionProvider` 介面抽象 OS 版本查詢（可測試）
- Mica 需要 Windows 11 Build 22000+，Acrylic 需要 Windows 10 1803+ (Build 17134)
- 不支援時自動 fallback 到 None，回傳 false
- **IconResolver** — singleton，`ConcurrentDictionary<string, ImageSource?>` 快取
- SVG 渲染：`Svg.Skia` → `SKSurface` → PNG encode → `BitmapDecoder.Create` → frozen `BitmapFrame`
- DPI-aware：pixel dimensions = `⌈size × dpiScale⌉`
- 檔案 > 1 MB 跳過，渲染失敗快取 null（不重試）
- FileSystemWatcher 偵測 SVG 變更時只清除快取（cheap），下次 Resolve 重新渲染

### 壓縮修復 (Compress Error Fix)
- `io::copy` 取代 `read_to_end` — streaming 壓縮，修復大檔案 OOM
- `FileOptions::large_file(true)` — 檔案 ≥ 4 GB 自動啟用 ZIP64
- `PtrToStringUTF8` 修復 — `CompressionService`、`DirectoryLockManager`、`ArchivePreviewService` 的錯誤指針全部改用 UTF-8 解碼
- 注意：progress callback 的 filename 指針仍用 `PtrToStringUni`（Rust 分配的 UTF-16）

### Zip Bomb 偵測
- `bomb_detector.rs` 在 listing 階段（寫入磁碟前）偵測
- 四項閾值：最大總大小、單檔大小、壓縮比、嵌套深度
- `LockError::ZipBomb(String)` + FFI 錯誤碼 `0x2005`
- C# 側 `ArchivePreviewService` 捕獲 `0x2005` 顯示 plain-language 警告
- Settings UI 可調整閾值，`ResetZipBombDefaults` 一鍵恢復預設

### 檔案鎖定偵測 (File Lock Detector)
- `wholock::who_locks_file(&path)` 查詢 Windows RestartManager API 取得鎖定程式列表
- 返回 Rust 分配的 null-terminated UTF-16 字串，C# 必須呼叫 `FreeString` 釋放
- 僅在 access-denied 錯誤後觸發（檢查 "access"/"denied"/"sharing" 關鍵字）
- 原始錯誤訊息先顯示，鎖定查詢在背景執行，完成後替換 InfoBar 訊息
- `Box::into_raw(vec.into_boxed_slice())` 確保 len == capacity，與 `zip_ease_free_string` 的重建邏輯相容

### 7z 檔案大小修復 (7z Size Fix)
- `sevenz.rs`：`list_entries_info` / `list_entries_info_with_password` 改用 `e.size as i64`（目錄保持 `-1`）
- `sevenzadll/backend.rs`：新增 `KPID_SIZE` (property ID 7) + `VT_UI8` (variant type 21) 查詢
- 從 `PROPVARIANT` 的 `uhVal` 欄位取得 `u64`，轉為 `i64`
- 目錄條目始終回傳 `-1`；`VT_EMPTY` 或未知 variant 也回傳 `-1`
- PBT 驗證：bug condition test（修復前失敗、修復後通過）+ preservation test（ZIP/目錄行為不變）

### 右鍵選單整合 (Context Menu Integration)
- **Shell Extension DLL** — `ZipEase.ShellExtension`，C# NativeAOT，`IExplorerCommand` COM 介面
- 兩個命令：`ExtractCommand`（解壓縮）+ `CompressCommand`（壓縮），各有獨立 GUID
- DLL 僅做 `Process.Start("ZipEase.exe", args)` — 零業務邏輯，符合 Holy Trinity 架構
- **註冊策略**：`RegistrationManager.DetectStrategy()` — Build ≥ 22000 → Sparse MSIX，否則 → Registry
- **Registry 路徑**：`HKCU\*\shell\ZipEaseExtract`、`HKCU\*\shell\ZipEaseCompress`、`HKCU\Directory\shell\ZipEaseCompress`
- **AppliesTo**：`System.FileExtension:=.zip OR System.FileExtension:=.7z OR ...` 限制顯示條件
- **Sparse MSIX**：`packaging/AppxManifest.xml` + `build-sparse-msix.ps1`；`AllowExternalContent=true`
- **首次啟動自動註冊**：`App.xaml.cs` 偵測首次啟動 → 自動註冊 → 結果存入 `AppSettings`
- **設定頁管理**：狀態顯示（✅/❌/⚠️）+ 重新註冊/停用按鈕
- **命令列解析**：`CommandLineParser` — bare paths → Extract，`--compress` → Compress，`--register-shell`/`--unregister-shell` → 註冊操作
- **例外安全**：所有 Shell Extension 程式碼路徑 try-catch → `E_FAIL` HRESULT，絕不 crash explorer.exe
- **圖示降級**：`.ico` 不存在時回傳空字串，選單仍顯示但無圖示

### 官方插件包與 Fallback 機制
- **C# 雙向路由與回退機制**：解壓 (`ExtractAsync`) 與列表 (`ListArchiveContentsWithPassword`/`KeepNative`) 均支援插件。若主插件因「環境依賴缺失/啟動失敗」（例如 Windows 上未安裝 Python，或缺少 `7za.dll`）拋出 `PluginException`，C# 端會自動捕獲，並查詢 manifest 中的 `fallback_extensions` 欄位，調用對應的備用插件進行重試。
- **Python CLI 適配器在 Windows 上的執行**：對於 ACE, ARJ, LHA 格式，Python 適配器（如 `plugin_ace.py`）透過 stdin 接收 JSON 請求，並調用 `tools/` 中的 Win32 工具。在執行解壓時，Python 腳本會先切換 `cwd` 至 C# 指定的輸出目錄，然後將壓縮包絕對路徑傳給 CLI 工具以確保解壓路徑的準確。C# `PluginBackend.cs` 也適配了對 `.py` 檔案的調用（若為 `.py` 結尾，則調用 `python` 作為 executable 並傳入腳本路徑作為 arguments）。
- **單檔案壓縮格式處理**：對於 LZ4/Zstandard 等單檔案壓縮格式（`plugin-lz4`, `plugin-zstd`），在 UI 預覽時，由於格式限制優先將「壓縮檔名去掉副檔名」作為內部檔案名稱；解壓大小若無法從 header 中讀取，則回傳 `-1` 標記為「未知大小」。
- **WPF 屬性測試修正**：修正了 `ZoomPanServiceTests.cs` 中原有 Property-based Test 的一個 size 構造 bug，將 invalid 寬高生成器限制為僅生成 `0.0`，以防止 WPF 的 `Size` 構造函數因負數直接拋出 `ArgumentException` 崩潰。
- **UI 按鈕簡化與合併**：移除了預覽介面頂部的「損壞檔案強制提取」CheckBox（該選項已在設定頁有專屬 Toggle 控制，避免重複配置）；將「全部解壓縮」與「解壓縮選定」按鈕合併為單一的「解壓縮」按鈕，並於 Code-Behind 監聽 `SelectionChanged` 事件。無選取時顯示為「全部解壓縮」（圖示 `FolderArrowUp`），有選取項目時顯示為「解壓縮選定」（圖示 `ArrowDownload`），有效降低使用者的視覺與認知負荷。

## 6. 關鍵檔案索引


| 檔案 | 用途 |
|------|------|
| `ZipEase.Core/zipease-extract/src/extract/sevenzadll/backend.rs` | RAR 後端：7za.dll COM 介面 |
| `ZipEase.Core/zipease-extract/src/extract/sevenz.rs` | 7z 後端：目錄過濾修復 |
| `ZipEase.Core/zipease-extract/src/extract/tar.rs` | TAR 後端：目錄過濾，`./` 前綴去除 |
| `ZipEase.Core/zipease-extract/src/extract/zip.rs` | ZIP 後端：`extract_force_progress`，`extract_entry` |
| `ZipEase.Core/zipease-extract/src/extract/smart.rs` | 智能格式偵測，路由 `"rar"` 到 `SevenZaDllBackend`，分割格式到 `SevenZaDllBackendWithClsid` |
| `ZipEase.Core/zipease-extract/src/extract/cab.rs` | CAB 後端：`cab` crate，safe_join 路徑防護 |
| `ZipEase.Core/zipease-extract/src/extract/iso.rs` | ISO 後端：純手工 ISO 9660 + Joliet 解析 |
| `ZipEase.Core/zipease-extract/src/ffi/list.rs` | 列表 FFI：密碼感知列表 |
| `ZipEase.Core/zipease-extract/src/ffi/extract.rs` | 解壓 FFI：`zip_ease_extract_force`，`zip_ease_extract_entry`，`zip_ease_free_string` |
| `ZipEase.UI/Core/NativeMethods.cs` | 所有 P/Invoke 聲明 |
| `ZipEase.UI/Core/ArchivePreviewService.cs` | 記憶體安全 FFI 包裝器 |
| `ZipEase.UI/Core/ExtractionManager.cs` | `ExtractAsync`，`ExtractForceAsync`，`ExtractEntryAsync` |
| `ZipEase.UI/Core/MainWindowViewModel.cs` | 狀態機 + 所有命令（含 `ExtractSingleEntryCommand`、`ExtractSelectedCommand`、`PreviewEntryCommand`、`ForceExtract`、`SearchText`） |
| `ZipEase.UI/MainWindow.xaml` | FluentWindow UI 定義（側邊欄、搜尋框、解壓縮選取按鈕） |
| `ZipEase.UI/Core/AppSettings.cs` | 設定持久化：`%AppData%\ZipEase\settings.json` |
| `ZipEase.UI/Core/SettingsView.xaml` | 設定頁面 UI（CardControl + ToggleSwitch） |
| `ZipEase.UI/Core/Plugin/PluginManifest.cs` | 插件描述檔 schema |
| `ZipEase.UI/Core/Plugin/PluginRegistry.cs` | 插件掃描與載入 |
| `ZipEase.UI/Core/Plugin/PluginBackend.cs` | CLI 插件通訊（JSON Lines 協議） |
| `ZipEase.UI/Core/LocalizationManager.cs` | i18n：`INotifyPropertyChanged` + `L.Current` |
| `ZipEase.UI/Strings/Strings.resx` | 英文 fallback 字串資源 |
| `ZipEase.UI/Strings/Strings.zh-TW.resx` | 繁體中文字串資源 |
| `ZipEase.UI/Strings/Strings.en.resx` | 英文字串資源 |
| `docs/plugin-example/plugin.json` | 範例插件描述檔 |
| `docs/plugin-example/plugin.py` | 範例 Python 插件 |
| `ZipEase.Core/zipease-extract/src/trash/mod.rs` | 資源回收桶邏輯：`trash_file()`，plain-language 錯誤映射 |
| `ZipEase.Core/zipease-extract/src/ffi/trash.rs` | 資源回收桶 FFI：`zip_ease_trash_file`，`catch_unwind` |
| `ZipEase.Core/zipease-extract/tests/trash_pbt.rs` | PBT：UTF-16 round-trip、no permanent delete、idempotent disable |
| `ZipEase.Core/zipease-extract/src/notify/toast.rs` | Toast 通知邏輯：WinRT XML 構建、AUMID 註冊 |
| `ZipEase.Core/zipease-extract/src/ffi/notify.rs` | Toast FFI：`zip_ease_notify_success`、`zip_ease_notify_failure` |
| `ZipEase.Core/zipease-extract/src/lock_detector/mod.rs` | 鎖定偵測邏輯：`who_locks()`、`join_process_names()` |
| `ZipEase.Core/zipease-extract/src/ffi/lock_detector.rs` | 鎖定偵測 FFI：`zip_ease_who_locks`、`catch_unwind` |
| `ZipEase.Core/zipease-extract/tests/lock_detector_pbt.rs` | PBT：UTF-16 round-trip、graceful degradation、process name joining、allocator compatibility |
| `ZipEase.UI/App.xaml` | WPF-UI 主題資源 |
| `ZipEase.UI/Core/ThemeLoader.cs` | 動態主題載入：singleton，FileSystemWatcher hot-reload，XAML ResourceDictionary 管理 |
| `ZipEase.UI/Core/BackdropSwitcher.cs` | 背景材質切換：Mica/Acrylic/None，OS 版本偵測，IOsVersionProvider 可測試 |
| `ZipEase.UI/Core/IconResolver.cs` | SVG 圖示解析：Svg.Skia 渲染，DPI 縮放，ConcurrentDictionary 快取 |
| `ZipEase.UI/Core/PasswordDialog.xaml` | 密碼輸入對話框 |
| `ZipEase.UI/Core/ArchiveEntryViewModel.cs` | `IsDirectory`、`IsFile`、資料夾圖示 |
| `ZipEase.Core/zipease-compress/tests/compress_bugfix_pbt.rs` | PBT：streaming 壓縮 + ZIP64 bug condition + preservation roundtrip |
| `ZipEase.UI.Tests/ThemingPropertyTests.cs` | PBT：XAML 掃描過濾、資源覆蓋、AppSettings round-trip、OS fallback、圖示解析、DPI 縮放 |
| `ZipEase.UI.Tests/ThemeLoaderTests.cs` | 單元測試：ThemeLoader ScanFolder/Load/Unload/Reload |
| `ZipEase.UI.Tests/BackdropSwitcherTests.cs` | 單元測試：BackdropSwitcher ToBackdropType/IsSupported/Apply |
| `ZipEase.UI.Tests/IconResolverTests.cs` | 單元測試：IconResolver Resolve/InvalidateCache |
| `ZipEase.UI.Tests/AppSettingsThemingTests.cs` | 單元測試：AppSettings 主題欄位預設值與啟動清理 |
| `ZipEase.UI.Tests/ThemingIntegrationTests.cs` | 整合測試：hot-reload 偵測、啟動還原 |
| `ZipEase.UI.Tests/SettingsViewModelThemingTests.cs` | 單元測試：SettingsViewModel backdrop 屬性與命令 |
| `ZipEase.Core/zipease-extract/tests/sevenz_size_bugfix_pbt.rs` | PBT：7z 檔案大小 bug condition 驗證（size > 0 for file entries） |
| `ZipEase.Core/zipease-extract/tests/sevenz_size_preservation_pbt.rs` | PBT：7z 修復 preservation（ZIP 大小不變、目錄旗標不變、檔名不變） |
| `ZipEase.ShellExtension/CommandBase.cs` | Shell Extension 基底類別：`GetSelectedPaths`、`LaunchZipEase`、例外安全 |
| `ZipEase.ShellExtension/ExtractCommand.cs` | 「用 ZipEase 解壓縮」右鍵選單命令 |
| `ZipEase.ShellExtension/CompressCommand.cs` | 「用 ZipEase 壓縮」右鍵選單命令 |
| `ZipEase.ShellExtension/ArchiveExtensions.cs` | 壓縮檔副檔名判斷（控制選單顯示/隱藏） |
| `ZipEase.UI/Core/CommandLineParser.cs` | 命令列解析：Extract/Compress/Register/Unregister 模式 |
| `ZipEase.UI/Core/RegistrationManager.cs` | Shell Extension 註冊管理：Sparse MSIX / Registry 策略 |
| `ZipEase.UI/Core/ShellExtensionStatus.cs` | 註冊狀態枚舉 + 結果記錄 |
| `packaging/AppxManifest.xml` | Sparse MSIX 清單：COM server + FileExplorerContextMenus 宣告 |
| `packaging/build-sparse-msix.ps1` | MSIX 打包腳本 |
| `ZipEase.Core/plugin-lz4/src/main.rs` | 原生 LZ4 解壓插件（含 lz4_flex 串流解壓與單元測試） |
| `ZipEase.Core/plugin-zstd/src/main.rs` | 原生 Zstandard 解壓插件（含 zstd 串流解壓與單元測試） |
| `ZipEase.Core/plugin-ace/plugin_ace.py` | ACE 格式的 Python CLI 適配器 |
| `ZipEase.Core/plugin-arj/plugin_arj.py` | ARJ 格式的 Python CLI 適配器 |
| `ZipEase.Core/plugin-lha/plugin_lha.py` | LHA/LZH 格式的 Python CLI 適配器 |
| `ZipEase.Core/plugin-dmg/plugin_dmg.py` | DMG 格式的 Python CLI 適配器（優先調用 local/path 的 7z.exe） |
| `build-plugin-pack.ps1` | 官方插件包 PowerShell 一鍵打包指令 |
| `.releaserc.json` / `.github/workflows/release.yml` | 官方插件包 semantic-release 與 GitHub Actions CI/CD 設定 |

---

*Generated by ZipEase Architect Team*

## 7. 編譯與啟動

詳見 [`docs/BUILD_AND_TEST.md`](docs/BUILD_AND_TEST.md)。
