# ==============================================================================
# TruthBeacon Windows Bundle Packager (PowerShell)
# Orange Heart Industries - Release Staging Pipeline
# ==============================================================================

param (
    [switch]$SkipSign = $true
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RootDir = Split-Path -Parent $ScriptDir
$SrcTauriDir = Join-Path $RootDir "src-tauri"

Write-Host "======================================================================" -ForegroundColor Cyan
Write-Host " TruthBeacon: Windows 64-bit Build (MSVC) & Installer Bundling" -ForegroundColor Cyan
Write-Host "======================================================================" -ForegroundColor Cyan

Set-Location $RootDir

# 1. Verify Rust Target
Write-Host "==> [1/4] Ensuring x86_64-pc-windows-msvc target installed..." -ForegroundColor Yellow
rustup target add x86_64-pc-windows-msvc

# 2. Generate installer assets (NSIS header/sidebar, WiX banner/dialog)
Write-Host "==> [2/4] Generating Orange Heart installer branding assets..." -ForegroundColor Yellow
python scripts/generate_installer_assets.py

# 3. Build Windows Bundles via Tauri (WiX MSI + NSIS EXE)
Write-Host "==> [3/4] Building WiX MSI & NSIS Installers via cargo-tauri..." -ForegroundColor Yellow
Set-Location $SrcTauriDir
if ($SkipSign) {
    cargo tauri build --target x86_64-pc-windows-msvc --no-sign
} else {
    cargo tauri build --target x86_64-pc-windows-msvc
}

# 4. Generate Portable Standalone Executable
Write-Host "==> [4/4] Creating standalone portable .exe distribution..." -ForegroundColor Yellow
$ReleaseDir = Join-Path $SrcTauriDir "target\x86_64-pc-windows-msvc\release"
$BundleDir = Join-Path $ReleaseDir "bundle"
$PortableDir = Join-Path $BundleDir "portable"

New-Item -ItemType Directory -Force -Path $PortableDir | Out-Null
Copy-Item (Join-Path $ReleaseDir "truth-beacon.exe") (Join-Path $PortableDir "TruthBeacon-Portable.exe") -Force

Write-Host "======================================================================" -ForegroundColor Green
Write-Host " Windows Build & Bundling Completed Successfully!" -ForegroundColor Green
Write-Host " Generated Artifacts:" -ForegroundColor Green
Get-ChildItem -Recurse -Path $BundleDir -Include *.msi, *.exe | Select-Object FullName
Write-Host "======================================================================" -ForegroundColor Green
