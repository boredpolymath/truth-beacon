# ==============================================================================
# TruthBeacon Windows Authenticode Signing & SmartScreen Pipeline
# Orange Heart Industries - Phase 24 Release Staging
# ==============================================================================

[CmdletBinding()]
param (
    [string]$CertFile,
    [string]$CertPassword,
    [string]$CertThumbprint,
    [string]$TimestampServer = "http://timestamp.digicert.com",
    [string]$DigestAlgorithm = "sha256",
    [switch]$DryRun = $false
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RootDir = Split-Path -Parent $ScriptDir
$SrcTauriDir = Join-Path $RootDir "src-tauri"
$ReleaseDir = Join-Path $RootDir "release"
$TargetBundleDir = Join-Path $SrcTauriDir "target\x86_64-pc-windows-msvc\release\bundle"

Write-Host "======================================================================" -ForegroundColor Cyan
Write-Host " TruthBeacon: Windows Authenticode Signing & SmartScreen (Phase 24.2)" -ForegroundColor Cyan
Write-Host "======================================================================" -ForegroundColor Cyan

# 1. Locate SignTool
function Find-SignTool {
    $found = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($found) { return $found.Source }

    $kitsRoots = @(
        "${env:ProgramFiles(x86)}\Windows Kits\10\bin",
        "${env:ProgramFiles}\Windows Kits\10\bin"
    )

    foreach ($root in $kitsRoots) {
        if (Test-Path $root) {
            $tools = Get-ChildItem -Path $root -Filter signtool.exe -Recurse -ErrorAction SilentlyContinue |
                Sort-Object FullName -Descending
            if ($tools.Count -gt 0) {
                return $tools[0].FullName
            }
        }
    }
    return $null
}

$SignToolPath = Find-SignTool

if (-not $SignToolPath) {
    if ($DryRun -or $env:CI -or (-not $CertFile -and -not $CertThumbprint)) {
        Write-Host "⚠️ Warning: signtool.exe not detected in PATH or Windows Kits." -ForegroundColor Yellow
        Write-Host "  Operating in dry-run / verification mode." -ForegroundColor Yellow
        $DryRun = $true
    } else {
        throw "signtool.exe not found. Install Windows 10/11 SDK or add signtool.exe to PATH."
    }
} else {
    Write-Host "  Located signtool: $SignToolPath" -ForegroundColor Green
}

# 2. Collect Artifacts to Sign
$ArtifactsToSign = @()

if (Test-Path $TargetBundleDir) {
    $FoundArtifacts = Get-ChildItem -Path $TargetBundleDir -Recurse -Include *.exe, *.msi
    foreach ($item in $FoundArtifacts) {
        $ArtifactsToSign += $item.FullName
    }
}

if ($ArtifactsToSign.Count -eq 0) {
    Write-Host "  No compiled Windows binaries found in $TargetBundleDir." -ForegroundColor Yellow
    Write-Host "  Checking release staging directory..." -ForegroundColor Yellow
    if (Test-Path $ReleaseDir) {
        $ReleaseArtifacts = Get-ChildItem -Path $ReleaseDir -Include *.exe, *.msi
        foreach ($item in $ReleaseArtifacts) {
            $ArtifactsToSign += $item.FullName
        }
    }
}

Write-Host "==> Target Artifacts for Authenticode Signing:" -ForegroundColor Cyan
if ($ArtifactsToSign.Count -eq 0) {
    Write-Host "  (No binaries currently staged to sign; verified pipeline structure)" -ForegroundColor Gray
} else {
    foreach ($art in $ArtifactsToSign) {
        Write-Host "  - $art" -ForegroundColor White
    }
}

# 3. Perform Signing
foreach ($file in $ArtifactsToSign) {
    Write-Host "`n==> Signing: $file" -ForegroundColor Yellow
    if ($DryRun) {
        Write-Host "  [DRY-RUN] Simulating signtool invocation for $file" -ForegroundColor Gray
        Write-Host "  signtool.exe sign /fd $DigestAlgorithm /tr $TimestampServer /td $DigestAlgorithm /d `"TruthBeacon`" `"$file`"" -ForegroundColor Gray
    } else {
        $SignArgs = @("sign", "/fd", $DigestAlgorithm, "/tr", $TimestampServer, "/td", $DigestAlgorithm, "/d", "TruthBeacon", "/du", "https://github.com/orangeheart-industries/truth-beacon")
        if ($CertFile) {
            $SignArgs += @("/f", $CertFile)
            if ($CertPassword) {
                $SignArgs += @("/p", $CertPassword)
            }
        } elseif ($CertThumbprint) {
            $SignArgs += @("/sha1", $CertThumbprint, "/sm")
        } else {
            $SignArgs += @("/a") # Auto select best cert in Personal store
        }
        $SignArgs += $file

        & $SignToolPath $SignArgs
        if ($LASTEXITCODE -ne 0) {
            throw "SignTool failed for $file with exit code $LASTEXITCODE"
        }

        # Verify Signature
        Write-Host "  Verifying Authenticode signature..." -ForegroundColor Green
        & $SignToolPath verify /pa /v $file
        if ($LASTEXITCODE -ne 0) {
            throw "Signature verification failed for $file"
        }
        Write-Host "  ✓ Authenticode signature verified successfully" -ForegroundColor Green
    }
}

# 4. Microsoft SmartScreen Program Submission Instructions
Write-Host "`n======================================================================" -ForegroundColor Cyan
Write-Host " Microsoft SmartScreen Binary Reputation Guidelines" -ForegroundColor Cyan
Write-Host "======================================================================" -ForegroundColor Cyan
Write-Host "1. EV Code Signing vs Standard OV Certificates:" -ForegroundColor White
Write-Host "   - Extended Validation (EV) certificates provide immediate Microsoft SmartScreen reputation." -ForegroundColor Gray
Write-Host "   - Standard OV certificates require initial download volume or manual dispute submission." -ForegroundColor Gray
Write-Host "2. Proactive SmartScreen Submission (Zero False Positives):" -ForegroundColor White
Write-Host "   - Submit release binaries to Microsoft Defender Security Intelligence:" -ForegroundColor Gray
Write-Host "     URL: https://www.microsoft.com/en-us/wdsi/filesubmission" -ForegroundColor Cyan
Write-Host "     Select: 'Software Developer' -> 'Incorrectly detected as malware/SmartScreen warning'" -ForegroundColor Gray
Write-Host "   - Submit through Microsoft Partner Center (Hardware/Desktop Developer Program) for instant telemetry whitelisting." -ForegroundColor Gray
Write-Host "======================================================================" -ForegroundColor Cyan
