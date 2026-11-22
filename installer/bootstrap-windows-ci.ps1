#!/usr/bin/env pwsh
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($env:GITHUB_ACTIONS -ne 'true') { throw 'GITHUB_ACTIONS must be true' }
if ([string]::IsNullOrEmpty($env:RUNNER_TEMP)) { throw 'RUNNER_TEMP is required' }

$RepoRoot = Split-Path $PSScriptRoot -Parent
$RunnerTemp = $env:RUNNER_TEMP
$LocalTemp = Join-Path $env:LOCALAPPDATA 'Temp'
$GstDir = Join-Path $LocalTemp 'TownLightStation-Toolchain-20260820'
$CapDir = Join-Path $LocalTemp 'TownLightStation-Toolchain-20260831'
$TsdDir = Join-Path $LocalTemp 'TownLightStation-Toolchain-20260826'
New-Item -ItemType Directory -Force -Path $GstDir, $CapDir, $TsdDir | Out-Null

function Download-Pinned {
    param([string]$Url, [string]$Dest, [string]$Sha)
    $want = $Sha.ToLowerInvariant()
    if (Test-Path $Dest) {
        if ((Get-FileHash -Path $Dest -Algorithm SHA256).Hash.ToLowerInvariant() -eq $want) { return }
        Remove-Item -Force $Dest
    }
    & curl.exe -fLsS --retry 2 --max-time 600 -o $Dest $Url
    if ($LASTEXITCODE -ne 0) { throw "curl failed ($LASTEXITCODE) for $Url" }
    $got = (Get-FileHash -Path $Dest -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($got -ne $want) { Remove-Item -Force $Dest; throw "SHA mismatch for $Url expected=$want actual=$got" }
}

function Invoke-Setup {
    param([string]$File, [string[]]$Arguments, [int]$TimeoutSec = 600, [switch]$AllowReboot)
    $p = Start-Process -FilePath $File -ArgumentList $Arguments -PassThru -WindowStyle Hidden
    if (-not $p.WaitForExit($TimeoutSec * 1000)) {
        try { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue } catch {}
        throw "Setup timed out: $File (${TimeoutSec}s)"
    }
    $code = $p.ExitCode
    if ($code -ne 0 -and -not ($AllowReboot -and $code -eq 3010)) { throw "Setup failed: $File rc=$code" }
    return $code
}

# 1. GStreamer
$gstExe = Join-Path $GstDir 'gstreamer-1.0-msvc-x86_64-1.28.6.exe'
Download-Pinned 'https://gstreamer.freedesktop.org/data/pkg/windows/1.28.6/msvc/gstreamer-1.0-msvc-x86_64-1.28.6.exe' $gstExe '059251444d1267b486eba390b18d25fed87e10315e72f757ec6c7e912fa746b5'
$gstRoot = Join-Path $env:LOCALAPPDATA 'Programs\gstreamer\1.0\msvc_x86_64'
$gstLog = Join-Path $RunnerTemp 'gstreamer-install.log'
Invoke-Setup $gstExe @('/VERYSILENT','/SUPPRESSMSGBOXES','/SP-','/CURRENTUSER','/NORESTART','/TYPE=devel','/TASKS=',"/DIR=`"$gstRoot`"","/LOG=`"$gstLog`"") 600 | Out-Null

# 2. Caption ZIP
$capZip = Join-Path $CapDir 'ffmpeg-n9.0.1-11-ge47273f4d9-win64-lgpl-9.0.zip'
Download-Pinned 'https://github.com/FFmpeg-Builds/FFmpeg-Builds/releases/download/autobuild-2026-08-31-13-27/ffmpeg-n9.0.1-11-ge47273f4d9-win64-lgpl-9.0.zip' $capZip '2484854ad6988d34560f4e6ea7a6ecb9dde0af7c229d2591815d056b04ec4f56'

# 3. TSDuck
$tsdZip = Join-Path $TsdDir 'TSDuck-Win64-3.44-4676-Portable.zip'
Download-Pinned 'https://github.com/tsduck/tsduck/releases/download/v3.44-4676/TSDuck-Win64-3.44-4676-Portable.zip' $tsdZip 'b0ca0f963fcd77488b8c32d6f9d85030daa891ed753c36e57bbb458807a44eb1'

# 4. Inno Setup
$innoExe = Join-Path $RunnerTemp 'innosetup-6.7.3.exe'
Download-Pinned 'https://github.com/jrsoftware/issrc/releases/download/is-6_7_3/innosetup-6.7.3.exe' $innoExe '9c73c3bae7ed48d44112a0f48e66742c00090bdb5bef71d9d3c056c66e97b732'
$innoRoot = Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6'
Invoke-Setup $innoExe @('/VERYSILENT','/SUPPRESSMSGBOXES','/SP-','/CURRENTUSER','/NORESTART',"/DIR=`"$innoRoot`"") 600 | Out-Null
$iscc = Join-Path $innoRoot 'ISCC.exe'
if (-not (Test-Path $iscc)) { throw "ISCC.exe missing at $iscc" }

# 5. Visual Studio Build Tools
$vsExe = Join-Path $RunnerTemp 'vs_BuildTools.exe'
Download-Pinned 'https://download.visualstudio.microsoft.com/download/pr/bc92e2cb-33de-4a0c-995d-efa817f16b16/37bb0fb429d163ecebd272a865d11a37b906d152bef960da2ddb29c2e2fd6eeb/vs_BuildTools.exe' $vsExe '37bb0fb429d163ecebd272a865d11a37b906d152bef960da2ddb29c2e2fd6eeb'
$btRoot = 'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools'
$clExe = Join-Path $btRoot 'VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64\cl.exe'
$rcPath = 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\rc.exe'
$boot = $true
if ((Test-Path $clExe) -and (Test-Path $rcPath)) {
    if ((Get-Item $clExe).VersionInfo.ProductVersion -eq '14.44.35229.0') { $boot = $false }
}
if ($boot) {
    $vsArgs = @('--quiet','--wait','--norestart','--nocache',"--installPath `"$btRoot`"",'--add','Microsoft.VisualStudio.Component.VC.14.44.17.14.x86.x64','--add','Microsoft.VisualStudio.Component.Windows11SDK.26100')
    try {
        Invoke-Setup $vsExe $vsArgs 1500 -AllowReboot | Out-Null
    } catch {
        Get-ChildItem -Path $env:TEMP -Filter 'dd_setup_*' -File -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTime -Descending | Select-Object -First 3 |
            ForEach-Object { "=== $($_.Name) ==="; Get-Content $_.FullName -Tail 60 -ErrorAction SilentlyContinue }
        throw
    }
}
if (-not (Test-Path $clExe) -or (Get-Item $clExe).VersionInfo.ProductVersion -ne '14.44.35229.0') { throw "cl.exe missing or wrong version at $clExe" }
if (-not (Test-Path $rcPath)) { throw "rc.exe missing at $rcPath" }

# Rust toolchain (canonical .rustup)
Remove-Item Env:RUSTUP_HOME -ErrorAction SilentlyContinue
Remove-Item Env:RUSTUP_TOOLCHAIN -ErrorAction SilentlyContinue
& rustup toolchain install 1.97.1-x86_64-pc-windows-msvc --profile minimal
if ($LASTEXITCODE -ne 0) { throw "rustup toolchain install failed ($LASTEXITCODE)" }
$cargoExe = Join-Path $env:USERPROFILE '.rustup\toolchains\1.97.1-x86_64-pc-windows-msvc\bin\cargo.exe'
if (-not (Test-Path $cargoExe)) { throw "cargo.exe missing at $cargoExe" }
$env:TOWNLIGHT_CARGO_EXECUTABLE = $cargoExe
$env:TOWNLIGHT_GSTREAMER_ROOT = $gstRoot
$env:PKG_CONFIG_PATH = Join-Path $gstRoot 'lib\pkgconfig'
Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue

# Build via repo scripts
Push-Location $RepoRoot
try {
    & powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File 'scripts/invoke-cargo.ps1' fetch --locked
    if ($LASTEXITCODE -ne 0) { throw "invoke-cargo.ps1 fetch failed ($LASTEXITCODE)" }
    & powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File 'installer/build-installer.ps1'
    if ($LASTEXITCODE -ne 0) { throw "build-installer.ps1 failed ($LASTEXITCODE)" }
} finally { Pop-Location }

# Verify artifacts
$setupExe = Join-Path $RepoRoot 'dist\TownLight-Station-0.1.0-x64-setup.exe'
$manifest = Join-Path $RepoRoot 'dist\candidate-manifest.json'
if (-not (Test-Path $setupExe)) { throw "missing $setupExe" }
if (-not (Test-Path $manifest)) { throw "missing $manifest" }
$m = Get-Content $manifest -Raw | ConvertFrom-Json
$actual = (Get-FileHash -Path $setupExe -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actual -ne $m.installer_sha256.ToLowerInvariant()) { throw "installer_sha256 mismatch: file=$actual manifest=$($m.installer_sha256)" }
if ($m.source_commit -ne $env:GITHUB_SHA) { throw "source_commit mismatch: manifest=$($m.source_commit) GITHUB_SHA=$env:GITHUB_SHA" }

Write-Host "Bootstrap complete."
