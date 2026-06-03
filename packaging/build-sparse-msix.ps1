#Requires -Version 5.1
<#
.SYNOPSIS
    Builds a Sparse MSIX package for ZipEase Shell Extension registration on Windows 11.

.DESCRIPTION
    This script creates a sparse MSIX package from the AppxManifest.xml template.
    The sparse package provides Package Identity required for Windows 11 modern
    context menu integration via IExplorerCommand.

    For development builds: creates an unsigned package (requires developer mode).
    For release builds: signs the package with a certificate.

.PARAMETER OutputPath
    Path where the .msix file will be created. Defaults to .\ZipEase.ShellExtension.msix

.PARAMETER Sign
    If specified, signs the package using SignTool.exe with the provided certificate.

.PARAMETER CertificatePath
    Path to the .pfx certificate file for signing. Required when -Sign is specified.

.PARAMETER CertificatePassword
    Password for the .pfx certificate. Required when -Sign is specified.

.EXAMPLE
    # Development build (unsigned)
    .\build-sparse-msix.ps1

.EXAMPLE
    # Release build (signed)
    .\build-sparse-msix.ps1 -Sign -CertificatePath .\cert.pfx -CertificatePassword "password"
#>

param(
    [string]$OutputPath = ".\ZipEase.ShellExtension.msix",
    [switch]$Sign,
    [string]$CertificatePath,
    [string]$CertificatePassword
)

$ErrorActionPreference = "Stop"

# Resolve paths
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ManifestPath = Join-Path $ScriptDir "AppxManifest.xml"
$AssetsDir = Join-Path $ScriptDir "Assets"

# Validate manifest exists
if (-not (Test-Path $ManifestPath)) {
    Write-Error "AppxManifest.xml not found at: $ManifestPath"
    exit 1
}

# Validate assets directory exists
if (-not (Test-Path $AssetsDir)) {
    Write-Error "Assets directory not found at: $AssetsDir"
    exit 1
}

# Create a temporary staging directory for the package contents
$StagingDir = Join-Path $env:TEMP "ZipEase_MSIX_Staging_$(Get-Random)"
New-Item -ItemType Directory -Path $StagingDir -Force | Out-Null

try {
    Write-Host "=== ZipEase Sparse MSIX Builder ===" -ForegroundColor Cyan
    Write-Host ""

    # Copy manifest to staging
    Write-Host "[1/4] Copying AppxManifest.xml..." -ForegroundColor Yellow
    Copy-Item $ManifestPath (Join-Path $StagingDir "AppxManifest.xml")

    # Copy assets to staging
    Write-Host "[2/4] Copying assets..." -ForegroundColor Yellow
    $StagingAssetsDir = Join-Path $StagingDir "Assets"
    New-Item -ItemType Directory -Path $StagingAssetsDir -Force | Out-Null

    $requiredAssets = @(
        "Square150x150Logo.png",
        "Square44x44Logo.png"
    )

    foreach ($asset in $requiredAssets) {
        $assetPath = Join-Path $AssetsDir $asset
        if (Test-Path $assetPath) {
            Copy-Item $assetPath (Join-Path $StagingAssetsDir $asset)
        } else {
            Write-Warning "Asset not found (will use placeholder): $asset"
            # Create a minimal 1x1 PNG placeholder
            $placeholderBytes = [Convert]::FromBase64String(
                "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="
            )
            [System.IO.File]::WriteAllBytes((Join-Path $StagingAssetsDir $asset), $placeholderBytes)
        }
    }

    # Find MakeAppx.exe
    Write-Host "[3/4] Locating MakeAppx.exe..." -ForegroundColor Yellow
    $MakeAppx = $null

    # Search in Windows SDK paths
    $sdkPaths = @(
        "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\MakeAppx.exe",
        "${env:ProgramFiles}\Windows Kits\10\bin\*\x64\MakeAppx.exe"
    )

    foreach ($pattern in $sdkPaths) {
        $found = Get-Item $pattern -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -First 1
        if ($found) {
            $MakeAppx = $found.FullName
            break
        }
    }

    if (-not $MakeAppx) {
        Write-Error @"
MakeAppx.exe not found. Please install the Windows SDK:
  - Visual Studio Installer → Individual Components → Windows 10 SDK (or Windows 11 SDK)
  - Or download from: https://developer.microsoft.com/en-us/windows/downloads/windows-sdk/
"@
        exit 1
    }

    Write-Host "  Found: $MakeAppx" -ForegroundColor Gray

    # Build the MSIX package
    Write-Host "[4/4] Building MSIX package..." -ForegroundColor Yellow
    $OutputFullPath = Resolve-Path $OutputPath -ErrorAction SilentlyContinue
    if (-not $OutputFullPath) {
        $OutputFullPath = Join-Path (Get-Location) $OutputPath
    } else {
        $OutputFullPath = $OutputFullPath.Path
    }

    # Remove existing output file if present
    if (Test-Path $OutputFullPath) {
        Remove-Item $OutputFullPath -Force
    }

    & $MakeAppx pack /d $StagingDir /p $OutputFullPath /nv
    if ($LASTEXITCODE -ne 0) {
        Write-Error "MakeAppx.exe failed with exit code $LASTEXITCODE"
        exit 1
    }

    Write-Host ""
    Write-Host "MSIX package created: $OutputFullPath" -ForegroundColor Green

    # Sign the package if requested
    if ($Sign) {
        if (-not $CertificatePath) {
            Write-Error "-CertificatePath is required when -Sign is specified."
            exit 1
        }

        if (-not (Test-Path $CertificatePath)) {
            Write-Error "Certificate not found: $CertificatePath"
            exit 1
        }

        Write-Host ""
        Write-Host "Signing package..." -ForegroundColor Yellow

        # Find SignTool.exe
        $SignTool = $null
        $signToolPaths = @(
            "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\SignTool.exe",
            "${env:ProgramFiles}\Windows Kits\10\bin\*\x64\SignTool.exe"
        )

        foreach ($pattern in $signToolPaths) {
            $found = Get-Item $pattern -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -First 1
            if ($found) {
                $SignTool = $found.FullName
                break
            }
        }

        if (-not $SignTool) {
            Write-Error "SignTool.exe not found. Please install the Windows SDK."
            exit 1
        }

        $signArgs = @(
            "sign",
            "/fd", "SHA256",
            "/a",
            "/f", $CertificatePath
        )

        if ($CertificatePassword) {
            $signArgs += @("/p", $CertificatePassword)
        }

        $signArgs += $OutputFullPath

        & $SignTool @signArgs
        if ($LASTEXITCODE -ne 0) {
            Write-Error "SignTool.exe failed with exit code $LASTEXITCODE"
            exit 1
        }

        Write-Host "Package signed successfully." -ForegroundColor Green
    } else {
        Write-Host ""
        Write-Host "NOTE: Package is UNSIGNED (development mode)." -ForegroundColor DarkYellow
        Write-Host "  To install, enable Developer Mode in Windows Settings." -ForegroundColor DarkYellow
        Write-Host "  For release builds, use: .\build-sparse-msix.ps1 -Sign -CertificatePath <path>" -ForegroundColor DarkYellow
    }

    Write-Host ""
    Write-Host "Done!" -ForegroundColor Cyan

} finally {
    # Clean up staging directory
    if (Test-Path $StagingDir) {
        Remove-Item $StagingDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}
