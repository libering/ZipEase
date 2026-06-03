# ZipEase Official Plugin Pack Build & Pack Script
# Run this script to compile all Rust plugins and package them with Python plugins.

$ErrorActionPreference = "Stop"

$ProjectRoot = Get-Item $PSScriptRoot
$CoreDir = Join-Path $ProjectRoot "ZipEase.Core"
$TargetDir = Join-Path $CoreDir "target\release"
$DistDir = Join-Path $ProjectRoot "dist"
$PluginsDistDir = Join-Path $DistDir "plugins"

# Cleanup previous dist
if (Test-Path $DistDir) {
    Remove-Item $DistDir -Recurse -Force
}
New-Item -ItemType Directory -Path $PluginsDistDir | Out-Null

Write-Host "=== Compiling Rust Plugins ===" -ForegroundColor Green

# Build plugin-7za-com
Write-Host "Building plugin-7za-com..."
Push-Location $CoreDir
cargo build --release -p plugin-7za-com
cargo build --release -p plugin-lz4
cargo build --release -p plugin-zstd
Pop-Location

Write-Host "=== Packaging Plugins ===" -ForegroundColor Green

# 1. plugin-7za-com
$Dest = Join-Path $PluginsDistDir "plugin-7za-com"
New-Item -ItemType Directory -Path $Dest | Out-Null
Copy-Item (Join-Path $TargetDir "plugin-7za-com.exe") (Join-Path $Dest "plugin-7za-com.exe")
Copy-Item (Join-Path $CoreDir "plugin-7za-com\plugin.json") (Join-Path $Dest "plugin.json")
# Check if 7za.dll exists in libs and copy it
$LibsDll = Join-Path $ProjectRoot "libs\7za.dll"
if (Test-Path $LibsDll) {
    Copy-Item $LibsDll (Join-Path $Dest "7za.dll")
}

# 2. plugin-lz4
$Dest = Join-Path $PluginsDistDir "plugin-lz4"
New-Item -ItemType Directory -Path $Dest | Out-Null
Copy-Item (Join-Path $TargetDir "plugin-lz4.exe") (Join-Path $Dest "plugin-lz4.exe")
Copy-Item (Join-Path $CoreDir "plugin-lz4\plugin.json") (Join-Path $Dest "plugin.json")

# 3. plugin-zstd
$Dest = Join-Path $PluginsDistDir "plugin-zstd"
New-Item -ItemType Directory -Path $Dest | Out-Null
Copy-Item (Join-Path $TargetDir "plugin-zstd.exe") (Join-Path $Dest "plugin-zstd.exe")
Copy-Item (Join-Path $CoreDir "plugin-zstd\plugin.json") (Join-Path $Dest "plugin.json")

# 4. plugin-ace
$Dest = Join-Path $PluginsDistDir "plugin-ace"
New-Item -ItemType Directory -Path $Dest | Out-Null
Copy-Item (Join-Path $CoreDir "plugin-ace\plugin_ace.py") (Join-Path $Dest "plugin_ace.py")
Copy-Item (Join-Path $CoreDir "plugin-ace\plugin.json") (Join-Path $Dest "plugin.json")

# 5. plugin-arj
$Dest = Join-Path $PluginsDistDir "plugin-arj"
New-Item -ItemType Directory -Path $Dest | Out-Null
Copy-Item (Join-Path $CoreDir "plugin-arj\plugin_arj.py") (Join-Path $Dest "plugin_arj.py")
Copy-Item (Join-Path $CoreDir "plugin-arj\plugin.json") (Join-Path $Dest "plugin.json")

# 6. plugin-lha
$Dest = Join-Path $PluginsDistDir "plugin-lha"
New-Item -ItemType Directory -Path $Dest | Out-Null
Copy-Item (Join-Path $CoreDir "plugin-lha\plugin_lha.py") (Join-Path $Dest "plugin_lha.py")
Copy-Item (Join-Path $CoreDir "plugin-lha\plugin.json") (Join-Path $Dest "plugin.json")

# 7. plugin-dmg
$Dest = Join-Path $PluginsDistDir "plugin-dmg"
New-Item -ItemType Directory -Path $Dest | Out-Null
Copy-Item (Join-Path $CoreDir "plugin-dmg\plugin_dmg.py") (Join-Path $Dest "plugin_dmg.py")
Copy-Item (Join-Path $CoreDir "plugin-dmg\plugin.json") (Join-Path $Dest "plugin.json")

# Add a README to the pack
$ReadmeContent = @"
# ZipEase Official Plugin Pack

Place these folders in your `%AppData%\ZipEase\plugins\` directory to enable:
- ACE support
- ARJ support
- LHA/LZH support
- DMG support
- XZ, LZMA, WIM, VHD/VHDX support (requires 7za.dll in plugin folder or app directory)
- LZ4 support
- Zstandard support
"@
$ReadmeContent | Out-File -FilePath (Join-Path $PluginsDistDir "README.md") -Encoding utf8

# Compress into zip
Write-Host "Compressing official-plugin-pack.zip..."
$ZipPath = Join-Path $DistDir "official-plugin-pack.zip"
Compress-Archive -Path "$PluginsDistDir\*" -DestinationPath $ZipPath -Force

Write-Host "=== Build Completed! Output: $ZipPath ===" -ForegroundColor Green
