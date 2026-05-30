# Requirements Document

## Introduction

ZipEase 目前内建支援 ZIP、7z、RAR、TAR 系列、CAB、ISO 等常用格式。为扩展对更多压缩和映像格式的支援，需要开发「官方插件包」，以可选插件形式提供额外格式支援，遵循现有的 CLI 插件系统架构。

## Glossary

- **ZipEase**: Windows 桌面压缩档案管理应用，采用 Rust 核心 + C# WPF UI 架构
- **CLI Plugin System**: 现有插件系统，插件为任意可执行档，通过 JSON Lines 协议与 ZipEase 通讯
- **JSON Lines Protocol**: 插件通讯协议，stdin 输入 JSON 请求，stdout 输出 JSON Lines 回应
- **PluginManifest**: 插件描述档 `plugin.json`，包含名称、版本、副档名、可执行档路径等元数据
- **PluginRegistry**: C# 端插件扫描与载入机制
- **PluginBackend**: C# 端插件通讯后端，处理 JSON Lines 协议
- **7za.dll**: 7-Zip 的动态连结库，提供 COM 接口支援多种格式
- **CLSID**: COM 类别识别码，用于指定 7za.dll 处理特定格式的处理器
- **Official Plugin Pack**: 官方维护的插件集合，发布于 GitHub Releases

## Requirements

### Requirement 1: 支援经典压缩格式 (ACE, ARJ, LHA/LZH)

**User Story:** 作为 ZipEase 用户，我希望能够解压缩 ACE、ARJ、LHA/LZH 等经典压缩格式，以便处理旧档案或从其他系统迁移的压缩档。

#### Acceptance Criteria

1. WHEN 用户拖放 `.ace` 档案到 ZipEase，THE PluginBackend SHALL 识别并调用对应插件进行解压缩
2. WHEN 用户拖放 `.arj` 档案到 ZipEase，THE PluginBackend SHALL 识别并调用对应插件进行解压缩
3. WHEN 用户拖放 `.lha` 或 `.lzh` 档案到 ZipEase，THE PluginBackend SHALL 识别并调用对应插件进行解压缩
4. IF 7za.dll 支援该格式的 CLSID 存在，THEN THE Official Plugin Pack SHALL 优先使用 7za.dll COM 接口实现
5. IF 7za.dll COM 接口初始化失败或发生错误，THEN THE PluginBackend SHALL 回退使用 Python 或 Rust 插件实现
6. IF 7za.dll 不支援该格式，THEN THE Official Plugin Pack SHALL 提供 Python 或 Rust 插件实现

### Requirement 2: 支援单文件压缩格式 (XZ, LZMA, LZ4, Zstandard)

**User Story:** 作为 ZipEase 用户，我希望能够解压缩 XZ、LZMA、LZ4、Zstandard 等单文件压缩格式，以便处理使用这些算法压缩的独立档案。

#### Acceptance Criteria

1. WHEN 用户拖放 `.xz` 档案到 ZipEase，THE PluginBackend SHALL 识别并调用对应插件进行解压缩
2. WHEN 用户拖放 `.lzma` 档案到 ZipEase，THE PluginBackend SHALL 识别并调用对应插件进行解压缩
3. WHEN 用户拖放 `.lz4` 档案到 ZipEase，THE PluginBackend SHALL 识别并调用对应插件进行解压缩
4. WHEN 用户拖放 `.zst` 档案到 ZipEase，THE PluginBackend SHALL 识别并调用对应插件进行解压缩
5. IF 格式需要外部命令列工具（如 `xz.exe`、`lz4.exe`、`zstd.exe`），THEN THE Official Plugin Pack SHALL 捆绑这些工具于插件目录
6. WHEN 解压缩完成，THE PluginBackend SHALL 回传 `{"status":"done","count":N}` 指示成功

### Requirement 3: 支援映像格式 (WIM, DMG, VHD/VHDX)

**User Story:** 作为 ZipEase 用户，我希望能够浏览和提取 WIM、DMG、VHD/VHDX 等映像格式的内容，以便存取系统映像或磁盘镜像中的档案。

#### Acceptance Criteria

1. WHEN 用户拖放 `.wim` 档案到 ZipEase，THE PluginBackend SHALL 识别并调用对应插件列出档案列表
2. WHEN 用户拖放 `.dmg` 档案到 ZipEase，THE PluginBackend SHALL 识别并调用对应插件列出档案列表
3. WHEN 用户拖放 `.vhd` 或 `.vhdx` 档案到 ZipEase，THE PluginBackend SHALL 识别并调用对应插件列出档案列表
4. WHEN 用户选择提取映像中的档案，THE PluginBackend SHALL 调用插件的 `extract` 动作
5. IF 映像格式需要挂载或特殊处理，THEN THE PluginBackend SHALL 通过进度回传 `{"status":"progress","pct":N,"file":"..."}` 通知用户

### Requirement 4: 插件包结构与发布机制

**User Story:** 作为 ZipEase 用户，我希望官方插件包能够方便地从 GitHub Releases 下载并手动安装，以便扩展 ZipEase 的格式支援。

#### Acceptance Criteria

1. THE Official Plugin Pack SHALL 发布于 GitHub Releases 作为 ZIP 压缩档
2. THE ZIP 压缩档 SHALL 包含一个或多个插件目录，每个目录包含 `plugin.json` 和可执行档
3. WHEN 用户将插件目录放置到 `%AppData%\ZipEase\plugins\`，THE PluginRegistry SHALL 在下次启动时自动载入
4. THE `plugin.json` SHALL 包含 `name`、`version`、`author`、`description`、`extensions`、`executable`、`can_compress` 等栏位
5. THE `extensions` 栏位 SHALL 列出该插件支援的所有副档名（如 `[".ace", ".arj"]`）
6. THE `executable` 栏位 SHALL 指向相对于 `plugin.json` 的可执行档路径

### Requirement 5: 插件通讯协议实现

**User Story:** 作为插件开发者，我希望遵循明确的 JSON Lines 协议规范，以便开发与 ZipEase 相容的插件。

#### Acceptance Criteria

1. WHEN ZipEase 需要列出压缩档内容，THE PluginBackend SHALL 向插件 stdin 发送 `{"action":"list","path":"..."}` 请求
2. WHEN ZipEase 需要解压缩档案，THE PluginBackend SHALL 向插件 stdin 发送 `{"action":"extract","path":"...","output":"...","password":null}` 请求
3. WHEN 插件成功列出内容，THE Plugin SHALL 向 stdout 回传 `{"status":"ok","entries":[{"name":"...","is_dir":false,"size":N},...]}`
4. WHEN 插件需要回报进度，THE Plugin SHALL 向 stdout 回传 `{"status":"progress","pct":N,"file":"..."}`
5. WHEN 插件完成解压缩，THE Plugin SHALL 向 stdout 回传 `{"status":"done","count":N}`
6. IF 插件发生错误，THEN THE Plugin SHALL 向 stdout 回传 `{"status":"error","message":"..."}` 并以非零退出码结束

### Requirement 6: 7za.dll COM 接口扩展

**User Story:** 作为 ZipEase 开发者，我希望优先使用 7za.dll COM 接口支援新格式，以便复用现有架构并减少外部依赖。

#### Acceptance Criteria

1. WHEN 7za.dll 支援某格式（存在对应 CLSID），THE Official Plugin Pack SHALL 使用 Rust 插件调用 `IInArchive` COM 接口
2. THE Rust 插件 SHALL 遵循现有 `sevenzadll/backend.rs` 的实现模式
3. THE Rust 插件 SHALL 支援 `list` 和 `extract` 动作
4. THE Rust 插件 SHALL 通过 JSON Lines 协议与 PluginBackend 通讯
5. IF 7za.dll 不支援某格式，THEN THE Official Plugin Pack SHALL 考虑 Python 插件或外部命令列工具

### Requirement 7: 安全性要求

**User Story:** 作为 ZipEase 用户，我希望插件系统遵循现有的安全原则，以便放心使用第三方格式的压缩档。

#### Acceptance Criteria

1. THE PluginBackend SHALL 使用 `safe_join()` 构建所有输出路径，防止路径穿越攻击
2. THE PluginBackend SHALL 在插件执行时设定工作目录为插件所在目录
3. THE PluginBackend SHALL 捕获插件的所有 stdout 输出，避免意外泄漏到主进程
4. IF 插件执行超过 5 分钟未完成，THEN THE PluginBackend SHALL 终止插件进程并保证进程已被结束，同时回传超时错误
5. THE PluginRegistry SHALL 忽略无效或格式错误的 `plugin.json` 档案，不影响其他插件载入

### Requirement 8: 设定页插件列表显示

**User Story:** 作为 ZipEase 用户，我希望在设定页看到已安装插件的列表，以便确认插件是否正确载入。

#### Acceptance Criteria

1. WHEN 用户开启设定页，THE SettingsViewModel SHALL 显示 `PluginRegistry.LoadedPlugins` 列表
2. THE 插件列表 SHALL 显示每个插件的名称、版本、描述
3. THE 插件列表 SHALL 显示每个插件支援的副档名
4. IF 插件目录为空，THEN THE SettingsViewModel SHALL 显示「尚未安装插件」提示
