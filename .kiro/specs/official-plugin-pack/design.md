# Design Document

## Introduction

本文档描述「官方插件包」的技术设计，遵循 ZipEase 的 Holy Trinity 架构原则（Rust 核心 + C# UI），复用现有 CLI 插件系统的基础设施。

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         ZipEase.UI (C#)                          │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │ PluginRegistry  │  │ PluginBackend   │  │ SettingsView    │  │
│  │ (扫描/载入)      │  │ (JSON Lines)    │  │ (插件列表)       │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ JSON Lines Protocol (stdin/stdout)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Official Plugin Pack                          │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                  Rust Plugins (7za.dll COM)                 │ │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │ │
│  │  │ .xz      │ │ .lzma    │ │ .wim     │ │ .vhd/.vhdx│        │ │
│  │  │ CLSID_XZ │ │CLSID_LZMA│ │CLSID_WIM │ │ CLSID_VHD │        │ │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘        │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                  Python Plugins (external tools)            │ │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │ │
│  │  │ .ace     │ │ .arj     │ │ .lha/.lzh│ │ .dmg     │        │ │
│  │  │ unace    │ │ arj      │ │ lha      │ │ 7z/hdiutil│       │ │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘        │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                  Rust Plugins (native crates)               │ │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐                     │ │
│  │  │ .lz4     │ │ .zst     │ │ (future) │                     │ │
│  │  │ lz4 crate│ │ zstd crate│ │          │                     │ │
│  │  └──────────┘ └──────────┘ └──────────┘                     │ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                        libs/7za.dll                               │
│              (7-Zip COM Interface - IInArchive)                   │
└─────────────────────────────────────────────────────────────────┘
```

## Component Design

### 1. Plugin Package Structure

**目录结构**:
```
official-plugin-pack/
├── plugin-7za-com/           # 7za.dll COM 接口插件
│   ├── plugin.json
│   └── plugin_7za_com.exe    # Rust 编译的可执行档
├── plugin-ace/               # ACE 格式插件
│   ├── plugin.json
│   ├── plugin_ace.py
│   └── tools/
│       └── unace.exe
├── plugin-arj/               # ARJ 格式插件
│   ├── plugin.json
│   ├── plugin_arj.py
│   └── tools/
│       └── arj.exe
├── plugin-lha/               # LHA/LZH 格式插件
│   ├── plugin.json
│   ├── plugin_lha.py
│   └── tools/
│       └── lha.exe
├── plugin-lz4/               # LZ4 格式插件
│   ├── plugin.json
│   └── plugin_lz4.exe        # Rust 原生实现
├── plugin-zstd/              # Zstandard 格式插件
│   ├── plugin.json
│   └── plugin_zstd.exe       # Rust 原生实现
└── README.md
```

**plugin.json Schema**:
```json
{
  "name": "ZipEase XZ/LZMA/WIM/VHD Plugin",
  "version": "1.0.0",
  "author": "ZipEase Team",
  "description": "Supports XZ, LZMA, WIM, VHD/VHDX formats via 7za.dll COM interface",
  "extensions": [".xz", ".lzma", ".wim", ".vhd", ".vhdx"],
  "executable": "plugin_7za_com.exe",
  "can_compress": false,
  "requires_7za_dll": true,
  "fallback_extensions": {
    ".xz": "ZipEase LZ4 Plugin",
    ".lzma": null
  }
}
```

**plugin.json 栏位说明**:

| 栏位 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | string | 是 | 插件显示名称，用于设定页显示 |
| `version` | string | 是 | 插件版本号 (语义化版本) |
| `author` | string | 是 | 插件作者 |
| `description` | string | 是 | 插件描述，说明支援的格式和实现方式 |
| `extensions` | string[] | 是 | 支援的副档名列表 |
| `executable` | string | 是 | 可执行档路径 (相对于 plugin.json) |
| `can_compress` | boolean | 否 | 是否支援压缩 (预设 false) |
| `requires_7za_dll` | boolean | 否 | 是否需要 7za.dll |
| `fallback_extensions` | object | 否 | 失败时的回退插件映射 |

**各插件 plugin.json 示例**:

```json
// plugin-ace/plugin.json
{
  "name": "ZipEase ACE Plugin",
  "version": "1.0.0",
  "author": "ZipEase Team",
  "description": "Supports ACE format via unace command-line tool",
  "extensions": [".ace"],
  "executable": "plugin_ace.py",
  "can_compress": false
}

// plugin-lz4/plugin.json
{
  "name": "ZipEase LZ4 Plugin",
  "version": "1.0.0",
  "author": "ZipEase Team",
  "description": "Supports LZ4 format via native Rust implementation",
  "extensions": [".lz4"],
  "executable": "plugin_lz4.exe",
  "can_compress": false
}

// plugin-zstd/plugin.json
{
  "name": "ZipEase Zstandard Plugin",
  "version": "1.0.0",
  "author": "ZipEase Team",
  "description": "Supports Zstandard format via native Rust implementation",
  "extensions": [".zst"],
  "executable": "plugin_zstd.exe",
  "can_compress": false
}
```

### 2. Rust Plugin: 7za.dll COM Interface

**档案**: `plugin-7za-com/src/main.rs`

```rust
//! 7za.dll COM Interface Plugin
//! 
//! 支援格式: XZ, LZMA, WIM, VHD, VHDX
//! 使用 IInArchive COM 接口，复用 sevenzadll 模块的基础设施

use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::ExitCode;
use serde::{Deserialize, Serialize};

mod sevenzadll;  // 复用现有模块

#[derive(Deserialize)]
struct ListRequest {
    action: String,
    path: String,
}

#[derive(Deserialize)]
struct ExtractRequest {
    action: String,
    path: String,
    output: String,
    password: Option<String>,
}

#[derive(Serialize)]
struct Entry {
    name: String,
    is_dir: bool,
    size: i64,
}

#[derive(Serialize)]
struct ProgressResponse {
    status: String,
    pct: u32,
    file: String,
}

#[derive(Serialize)]
struct DoneResponse {
    status: String,
    count: usize,
}

#[derive(Serialize)]
struct ErrorResponse {
    status: String,
    message: String,
}

fn main() -> ExitCode {
    let stdin = io::stdin();
    let line = match stdin.lock().lines().next() {
        Some(Ok(l)) => l,
        _ => return ExitCode::FAILURE,
    };
    
    // Parse and handle request...
    // Implementation follows existing sevenzadll pattern
    
    ExitCode::SUCCESS
}
```

**CLSID 定义** (新增于 `sevenzadll/types.rs`):
```rust
// XZ handler CLSID: {23170F69-40C1-278A-1000-0001100C0000}
pub const CLSID_XZ_HANDLER: GUID = GUID {
    data1: 0x23170F69,
    data2: 0x40C1,
    data3: 0x278A,
    data4: [0x10, 0x00, 0x00, 0x01, 0x10, 0x0C, 0x00, 0x00],
};

// LZMA handler CLSID: {23170F69-40C1-278A-1000-0001100B0000}
pub const CLSID_LZMA_HANDLER: GUID = GUID {
    data1: 0x23170F69,
    data2: 0x40C1,
    data3: 0x278A,
    data4: [0x10, 0x00, 0x00, 0x01, 0x10, 0x0B, 0x00, 0x00],
};

// WIM handler CLSID: {23170F69-40C1-278A-1000-0001100E0000}
pub const CLSID_WIM_HANDLER: GUID = GUID {
    data1: 0x23170F69,
    data2: 0x40C1,
    data3: 0x278A,
    data4: [0x10, 0x00, 0x00, 0x01, 0x10, 0x0E, 0x00, 0x00],
};

// VHD handler CLSID: {23170F69-40C1-278A-1000-0001100F0000}
pub const CLSID_VHD_HANDLER: GUID = GUID {
    data1: 0x23170F69,
    data2: 0x40C1,
    data3: 0x278A,
    data4: [0x10, 0x00, 0x00, 0x01, 0x10, 0x0F, 0x00, 0x00],
};
```

### 3. Rust Plugin: Native LZ4/Zstandard

**档案**: `plugin-lz4/src/main.rs`

```rust
//! Native LZ4 Plugin
//! 
//! 使用 lz4 crate 实现，无需外部工具

use lz4::Decoder;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::Path;

fn decompress_xz(input: &Path, output: &Path) -> io::Result<()> {
    let input_file = File::open(input)?;
    let mut decoder = Decoder::new(input_file)?;
    let mut output_file = File::create(output)?;
    io::copy(&mut decoder, &mut output_file)?;
    Ok(())
}

// JSON Lines protocol handling...
```

**档案**: `plugin-zstd/src/main.rs`

```rust
//! Native Zstandard Plugin
//! 
//! 使用 zstd crate 实现

use zstd::stream::Decoder;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::Path;

fn decompress_zst(input: &Path, output: &Path) -> io::Result<()> {
    let input_file = File::open(input)?;
    let mut decoder = Decoder::new(input_file)?;
    let mut output_file = File::create(output)?;
    io::copy(&mut decoder, &mut output_file)?;
    Ok(())
}

// JSON Lines protocol handling...
```

### 4. Python Plugin: External Tools

**档案**: `plugin-ace/plugin_ace.py`

```python
#!/usr/bin/env python3
"""
ZipEase CLI Plugin — ACE Format

Protocol: read one JSON line from stdin, write JSON lines to stdout.
"""

import sys
import json
import subprocess
import os
from pathlib import Path

TOOLS_DIR = Path(__file__).parent / "tools"
UNACE_PATH = TOOLS_DIR / "unace.exe"

def handle_list(req):
    path = req.get("path")
    # Run unace l <archive>
    result = subprocess.run(
        [str(UNACE_PATH), "l", path],
        capture_output=True,
        text=True
    )
    
    if result.returncode != 0:
        print(json.dumps({"status": "error", "message": result.stderr}), flush=True)
        return
    
    # Parse output and extract entries
    entries = parse_unace_list(result.stdout)
    print(json.dumps({"status": "ok", "entries": entries}), flush=True)

def handle_extract(req):
    path = req.get("path")
    output = req.get("output", ".")
    
    # Run unace x <archive> <output>
    result = subprocess.run(
        [str(UNACE_PATH), "x", path, output],
        capture_output=True,
        text=True
    )
    
    if result.returncode != 0:
        print(json.dumps({"status": "error", "message": result.stderr}), flush=True)
        return
    
    # Report progress (simplified - unace doesn't support progress)
    print(json.dumps({"status": "progress", "pct": 100, "file": path}), flush=True)
    print(json.dumps({"status": "done", "count": 1}), flush=True)

if __name__ == "__main__":
    line = sys.stdin.readline().strip()
    if not line:
        sys.exit(1)
    
    req = json.loads(line)
    action = req.get("action")
    
    if action == "list":
        handle_list(req)
    elif action == "extract":
        handle_extract(req)
    else:
        print(json.dumps({"status": "error", "message": f"Unknown action: {action}"}), flush=True)
        sys.exit(1)
```

### 5. PluginBackend Enhancement

**档案**: `ZipEase.UI/Core/Plugin/PluginBackend.cs` (修改)

```csharp
// 新增: 超时控制与进程终止
public class PluginBackend
{
    private static readonly TimeSpan Timeout = TimeSpan.FromMinutes(5);
    
    public async Task<PluginResult> ExecuteAsync(PluginManifest plugin, PluginRequest request, CancellationToken cancellationToken)
    {
        var processInfo = new ProcessStartInfo
        {
            FileName = Path.Combine(plugin.Directory, plugin.Executable),
            WorkingDirectory = plugin.Directory,
            RedirectStandardInput = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            CreateNoWindow = true
        };
        
        using var process = new Process { StartInfo = processInfo };
        process.Start();
        
        // Send request
        var requestJson = JsonSerializer.Serialize(request);
        await process.StandardInput.WriteLineAsync(requestJson);
        process.StandardInput.Close();
        
        // Read responses with timeout
        var responses = new List<JsonElement>();
        var timeoutTask = Task.Delay(Timeout, cancellationToken);
        var readTask = ReadResponsesAsync(process.StandardOutput, responses);
        
        var completedTask = await Task.WhenAny(readTask, timeoutTask);
        
        if (completedTask == timeoutTask)
        {
            // Timeout - ensure process is killed
            try
            {
                process.Kill(entireProcessTree: true);
            }
            catch { /* Best effort */ }
            
            return new PluginResult { Status = "error", Message = "Plugin execution timeout" };
        }
        
        await process.WaitForExitAsync(cancellationToken);
        
        return ParseResult(responses);
    }
}
```

### 6. PluginRegistry Enhancement

**档案**: `ZipEase.UI/Core/Plugin/PluginRegistry.cs` (修改)

```csharp
// 新增: 支援 requires_7za_dll 和 fallback 机制
public class PluginRegistry
{
    public List<PluginManifest> LoadedPlugins { get; } = new();
    
    public PluginManifest? FindPluginForExtension(string extension)
    {
        return LoadedPlugins.FirstOrDefault(p => 
            p.Extensions.Contains(extension, StringComparer.OrdinalIgnoreCase));
    }
    
    public PluginManifest? FindFallbackPlugin(string extension)
    {
        var primary = FindPluginForExtension(extension);
        if (primary?.FallbackExtensions?.TryGetValue(extension, out var fallbackName) == true 
            && fallbackName != null)
        {
            return LoadedPlugins.FirstOrDefault(p => 
                p.Name.Equals(fallbackName, StringComparison.OrdinalIgnoreCase));
        }
        return null;
    }
}
```

## Format Implementation Matrix

| 格式 | 实现方式 | 工具/CLSID | 压缩支援 |
|------|----------|------------|----------|
| .ace | Python + unace.exe | unace (WinAce) | 否 |
| .arj | Python + arj.exe | arj | 否 |
| .lha/.lzh | Python + lha.exe | lha | 否 |
| .xz | 7za.dll COM | CLSID_XZ_HANDLER | 否 |
| .lzma | 7za.dll COM | CLSID_LZMA_HANDLER | 否 |
| .lz4 | Rust native | lz4 crate | 否 |
| .zst | Rust native | zstd crate | 否 |
| .wim | 7za.dll COM | CLSID_WIM_HANDLER | 否 |
| .dmg | Python + 7z | 7z (partial) | 否 |
| .vhd/.vhdx | 7za.dll COM | CLSID_VHD_HANDLER | 否 |

## Security Considerations

### 1. Path Traversal Prevention

所有插件在提取时必须使用 `safe_join()` 构建输出路径：

```rust
fn safe_join(base: &Path, name: &str) -> PathBuf {
    let name = name.replace('\\', "/");
    let name = name.trim_start_matches('/');
    
    let result = base.join(name);
    let canonical_base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    
    if let Ok(canonical_result) = result.canonicalize() {
        if !canonical_result.starts_with(&canonical_base) {
            return canonical_base.join(name.rsplit('/').next().unwrap_or(name));
        }
    }
    
    result
}
```

### 2. Process Isolation

- 插件以独立进程运行，崩溃不影响主程序
- 设定工作目录为插件目录，避免路径污染
- 超时后强制终止进程树（包含子进程）

### 3. Input Validation

- 验证 `plugin.json` schema
- 验证 `extensions` 列表中的副档名格式
- 验证 `executable` 路径不包含 `..` 或绝对路径

## Release Strategy

### semantic-release 自动化发布

使用 [semantic-release](https://github.com/semantic-release/semantic-release) 实现全自动版本管理和 GitHub Release 发布。

**工作流程**：
1. 分析 Git commit 消息（Conventional Commits 规范）
2. 自动递增版本号
3. 生成/更新 CHANGELOG.md
4. 创建 GitHub Release
5. 上传 ZIP asset

**配置文件**：

`.releaserc.json`:
```json
{
  "branches": ["main"],
  "plugins": [
    "@semantic-release/commit-analyzer",
    "@semantic-release/release-notes-generator",
    "@semantic-release/changelog",
    "@semantic-release/github",
    "@semantic-release/git"
  ]
}
```

`.github/workflows/release.yml`:
```yaml
name: Release Plugin Pack

on:
  push:
    branches: [main]
    paths:
      - 'plugins/**'

permissions:
  contents: write

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
      
      - name: Install semantic-release
        run: |
          npm install -g semantic-release
          npm install -g @semantic-release/commit-analyzer
          npm install -g @semantic-release/release-notes-generator
          npm install -g @semantic-release/changelog
          npm install -g @semantic-release/github
          npm install -g @semantic-release/git
      
      - name: Build plugin pack
        run: |
          ./build-plugin-pack.ps1
      
      - name: Release
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: semantic-release
```

**Commit 消息规范**：

使用 [Conventional Commits](https://www.conventionalcommits.org/)：

```
feat(plugin): add ACE format support      # 新功能 → minor version bump
fix(plugin): fix LZ4 decompression error  # bug 修复 → patch version bump
docs: update README                        # 文档更新 → no release
```

### GitHub Release Structure

```
official-plugin-pack-v1.0.0.zip
├── plugin-7za-com/
├── plugin-ace/
├── plugin-arj/
├── plugin-lha/
├── plugin-lz4/
├── plugin-zstd/
└── README.md
```

### Versioning

- 主版本号跟随 ZipEase 主版本
- 副版本号表示新增格式支援
- 修订号表示 bug 修复

## Testing Strategy

### Unit Tests

- 每个插件的 JSON Lines 协议处理
- `safe_join()` 路径穿越防护测试
- 超时终止逻辑测试

### Integration Tests

- 端到端: 拖放档案 → 插件载入 → 列表/提取
- 错误处理: 无效档案、密码错误、磁盘空间不足
- 回退机制: 7za.dll 失败 → Python 插件

### Property-Based Tests

- JSON Lines round-trip (request → response)
- `safe_join` 不变性质（结果始终在 base 目录内）
- 进度百分比单调递增

## Implementation Phases

### Phase 1: 7za.dll COM 插件 (高优先级)
- XZ, LZMA, WIM, VHD/VHDX
- 复用现有 `sevenzadll` 模块

### Phase 2: Native Rust 插件
- LZ4, Zstandard
- 使用 `lz4` 和 `zstd` crates

### Phase 3: Python + 外部工具插件
- ACE, ARJ, LHA/LZH
- DMG (部分支援)

### Phase 4: 发布与文档
- GitHub Release 打包
- README 文档
- 用户安装指南
