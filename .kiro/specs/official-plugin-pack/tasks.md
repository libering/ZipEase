# Tasks Document

## Overview

本文件列出「官方插件包」功能的开发任务，按实现阶段分组。

---

## Phase 1: 7za.dll COM 插件

### Task 1.1: 新增 CLSID 常数定义
- [ ] 在 `ZipEase.Core/zipease-extract/src/extract/sevenzadll/types.rs` 新增 CLSID 常数：
  - `CLSID_XZ_HANDLER` (XZ 格式)
  - `CLSID_LZMA_HANDLER` (LZMA 格式)
  - `CLSID_WIM_HANDLER` (WIM 格式)
  - `CLSID_VHD_HANDLER` (VHD/VHDX 格式)

### Task 1.2: 创建 7za.dll COM 插件专案
- [ ] 在 `plugins/plugin-7za-com/` 创建 Rust 专案
- [ ] 设定 `Cargo.toml`，输出 `cdylib` 或可执行档
- [ ] 复用 `sevenzadll` 模块的 COM 接口代码
- [ ] 实现 JSON Lines 协议处理 (stdin/stdout)

### Task 1.3: 实现 list 动作
- [ ] 解析 `{"action":"list","path":"..."}` 请求
- [ ] 调用 `IInArchive` 接口开启档案
- [ ] 遍历条目，回传 `{"status":"ok","entries":[...]}`
- [ ] 错误处理：无效档案、密码错误等

### Task 1.4: 实现 extract 动作
- [ ] 解析 `{"action":"extract","path":"...","output":"..."}` 请求
- [ ] 调用 `IInArchive` 接口提取档案
- [ ] 使用 `safe_join()` 构建输出路径
- [ ] 回传进度 `{"status":"progress","pct":N,"file":"..."}`
- [ ] 回传完成 `{"status":"done","count":N}`

### Task 1.5: 创建 plugin.json
- [ ] 创建 `plugins/plugin-7za-com/plugin.json`
- [ ] 设定 `name`: "ZipEase XZ/LZMA/WIM/VHD Plugin"
- [ ] 设定 `extensions`: [".xz", ".lzma", ".wim", ".vhd", ".vhdx"]
- [ ] 设定 `requires_7za_dll`: true

---

## Phase 2: Native Rust 插件

### Task 2.1: 创建 LZ4 插件专案
- [ ] 在 `plugins/plugin-lz4/` 创建 Rust 专案
- [ ] 加入 `lz4` crate 依赖
- [ ] 实现 JSON Lines 协议处理

### Task 2.2: 实现 LZ4 解压缩
- [ ] 解析 list 请求：LZ4 是单文件格式，回传单个条目
- [ ] 解析 extract 请求：使用 `lz4::Decoder` 解压缩
- [ ] 错误处理：无效格式、I/O 错误

### Task 2.3: 创建 Zstandard 插件专案
- [ ] 在 `plugins/plugin-zstd/` 创建 Rust 专案
- [ ] 加入 `zstd` crate 依赖
- [ ] 实现 JSON Lines 协议处理

### Task 2.4: 实现 Zstandard 解压缩
- [ ] 解析 list 请求：Zstandard 是单文件格式，回传单个条目
- [ ] 解析 extract 请求：使用 `zstd::stream::Decoder` 解压缩
- [ ] 错误处理：无效格式、I/O 错误

### Task 2.5: 创建 plugin.json 档案
- [ ] 创建 `plugins/plugin-lz4/plugin.json`
- [ ] 创建 `plugins/plugin-zstd/plugin.json`

---

## Phase 3: Python + 外部工具插件

### Task 3.1: 创建 ACE 插件
- [ ] 在 `plugins/plugin-ace/` 创建目录
- [ ] 编写 `plugin_ace.py` (JSON Lines 协议)
- [ ] 下载 `unace.exe` 到 `tools/` 目录
- [ ] 实现 list 动作：解析 `unace l` 输出
- [ ] 实现 extract 动作：调用 `unace x`
- [ ] 创建 `plugin.json`

### Task 3.2: 创建 ARJ 插件
- [ ] 在 `plugins/plugin-arj/` 创建目录
- [ ] 编写 `plugin_arj.py`
- [ ] 下载 `arj.exe` 到 `tools/` 目录
- [ ] 实现 list 动作：解析 `arj l` 输出
- [ ] 实现 extract 动作：调用 `arj x`
- [ ] 创建 `plugin.json`

### Task 3.3: 创建 LHA/LZH 插件
- [ ] 在 `plugins/plugin-lha/` 创建目录
- [ ] 编写 `plugin_lha.py`
- [ ] 下载 `lha.exe` 到 `tools/` 目录
- [ ] 实现 list 动作：解析 `lha l` 输出
- [ ] 实现 extract 动作：调用 `lha x`
- [ ] 创建 `plugin.json`

### Task 3.4: 创建 DMG 插件
- [ ] 在 `plugins/plugin-dmg/` 创建目录
- [ ] 编写 `plugin_dmg.py`
- [ ] 使用 7z 命令行工具处理 DMG (部分支援)
- [ ] 实现 list 动作
- [ ] 实现 extract 动作
- [ ] 创建 `plugin.json`

---

## Phase 4: PluginBackend 增强

### Task 4.1: 实现超时终止机制
- [ ] 在 `PluginBackend.cs` 加入 5 分钟超时
- [ ] 超时后调用 `process.Kill(entireProcessTree: true)`
- [ ] 回传超时错误讯息

### Task 4.2: 实现工作目录设定
- [ ] 设定 `ProcessStartInfo.WorkingDirectory` 为插件目录
- [ ] 确保相对路径正确解析

### Task 4.3: 实现 requires_7za_dll 检查
- [ ] 在 `PluginRegistry` 检查 `requires_7za_dll` 栏位
- [ ] 如果 7za.dll 不存在，跳过载入或显示警告

### Task 4.4: 实现回退机制
- [ ] 解析 `fallback_extensions` 栏位
- [ ] 当主插件失败时，尝试回退插件

---

## Phase 5: 测试

### Task 5.1: 单元测试 - JSON Lines 协议
- [ ] 测试请求解析
- [ ] 测试回应序列化
- [ ] 测试错误处理

### Task 5.2: 单元测试 - safe_join
- [ ] 测试正常路径
- [ ] 测试路径穿越攻击 (`../`, 绝对路径)
- [ ] 测试特殊字符处理

### Task 5.3: 整合测试 - 端到端
- [ ] 测试各格式的 list 功能
- [ ] 测试各格式的 extract 功能
- [ ] 测试错误场景 (无效档案、密码错误)

### Task 5.4: 属性测试 (PBT)
- [ ] JSON Lines round-trip 测试
- [ ] safe_join 不变性质测试
- [ ] 进度百分比单调递增测试

---

## Phase 6: 发布

### Task 6.1: 创建打包脚本
- [ ] 编写 PowerShell 脚本 `build-plugin-pack.ps1`
- [ ] 编译 Rust 插件
- [ ] 收集 Python 插件和工具
- [ ] 打包为 ZIP

### Task 6.2: 编写文档
- [ ] 编写 `plugins/README.md`
- [ ] 安装说明
- [ ] 支援格式列表
- [ ] 故障排除

### Task 6.3: 配置 semantic-release 自动发布
- [ ] 安装 semantic-release 及相关插件
  - `@semantic-release/commit-analyzer` - 分析 commit 消息
  - `@semantic-release/release-notes-generator` - 生成 Release Notes
  - `@semantic-release/github` - 创建 GitHub Release 并上传 assets
  - `@semantic-release/changelog` - 生成/更新 CHANGELOG.md
  - `@semantic-release/git` - 提交版本变更
- [ ] 创建 `.releaserc.json` 配置文件
- [ ] 创建 `.github/workflows/release.yml` GitHub Actions workflow
- [ ] 配置 commitlint 确保 commit 消息符合 Conventional Commits 规范
- [ ] 测试 release 流程（可在测试分支验证）

---

## Task Dependencies

```
Phase 1 (7za.dll COM)
├── Task 1.1 ─→ Task 1.2 ─→ Task 1.3 ─→ Task 1.4 ─→ Task 1.5

Phase 2 (Native Rust)
├── Task 2.1 ─→ Task 2.2 ─→ Task 2.5
└── Task 2.3 ─→ Task 2.4 ─→ Task 2.5

Phase 3 (Python)
├── Task 3.1 (独立)
├── Task 3.2 (独立)
├── Task 3.3 (独立)
└── Task 3.4 (独立)

Phase 4 (Backend)
├── Task 4.1 (独立)
├── Task 4.2 (独立)
├── Task 4.3 ─→ Task 4.4
└── Task 4.4 (依赖 4.3)

Phase 5 (Testing)
├── Task 5.1 ─→ Task 5.3
├── Task 5.2 ─→ Task 5.3
└── Task 5.4 ─→ Task 5.3

Phase 6 (Release)
├── Task 6.1 ─→ Task 6.3
├── Task 6.2 ─→ Task 6.3
└── Task 6.3 (依赖所有前置任务完成)
```

---

## Estimated Effort

| Phase | 预估工时 |
|-------|----------|
| Phase 1 | 8-12 小时 |
| Phase 2 | 6-8 小时 |
| Phase 3 | 8-10 小时 |
| Phase 4 | 4-6 小时 |
| Phase 5 | 6-8 小时 |
| Phase 6 | 2-4 小时 |
| **总计** | **34-48 小时** |
