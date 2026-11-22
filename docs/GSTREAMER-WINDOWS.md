# Rebuild the Windows GStreamer installer

CivicCast Lite's Windows installer runs the GStreamer installer in unattended, all-users runtime mode. The rebuild keeps the upstream payload, components, registry entries, and setup script. Inno Setup embeds an Authenticode-signed engine so the temporary setup program Windows actually runs also has a valid signature.

Run the steps below in PowerShell 7 on Windows with Python 3 and at least 10 GB of scratch space. Use an empty scratch directory on a short local path.

## Get the tools

Install [Inno Setup 6.7.3](https://github.com/jrsoftware/issrc/releases/tag/is-6_7_3). Verify the downloaded `innosetup-6.7.3.exe` has SHA-256 `9c73c3bae7ed48d44112a0f48e66742c00090bdb5bef71d9d3c056c66e97b732` and a valid Authenticode signature before installing it. If you already ran the hosted Windows toolchain bootstrap, use its `ISCC.exe`.

Download the [pinned innounp 2.71.4 ZIP](https://raw.githubusercontent.com/jrathlev/InnoUnpacker-Windows-GUI/5410e03e6ad409ac982ca9cb2365db815a8b53fb/innounp-2/bin/innounp-2.zip). Verify SHA-256 `078ddb005aa17e669f0b5545980bff92200f7401418174172dca0d75f9e5d8f0`, then extract it. The rebuild script verifies the compiler and unpacker binaries before use.

## Prepare, sign, and build

From the repository root, set the tools and an empty work directory, then prepare the engine.

```powershell
$work = 'C:\build\civiccast-gstreamer-1.28.6'
$iscc = Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'
$unpacker = 'C:\build\tools\innounp.exe'

./installer/rebuild-gstreamer-windows.ps1 -Phase Prepare `
    -WorkRoot $work -InnoCompiler $iscc -InnoUnpacker $unpacker
```

Send `gstreamer-1.28.6-embedded-setup-engine.exe` from the work directory to the project's existing Windows signing workflow. Its unsigned SHA-256 is `5f44c191d26825c23f3b63fe97f9c0a461b338f1d51e4546a501767f9fc7d19e`. Preserve the unsigned engine and save the signed file and its signing receipt separately.

```powershell
./installer/rebuild-gstreamer-windows.ps1 -Phase Build `
    -WorkRoot $work -InnoCompiler $iscc -InnoUnpacker $unpacker `
    -SignedEngine 'C:\build\signed\gstreamer-1.28.6-embedded-setup-engine.exe' `
    -SignedEngineReceipt 'C:\build\signed\signing-receipt.json'
```

Build verifies the engine receipt and checks the engine extracted from the new installer. It re-extracts the complete setup payload and checks all 5,450 file hashes against the upstream bytes. The unsigned outer installer is `output/gstreamer-1.0-msvc-x86_64-1.28.6.exe` under the work directory. Submit that file and the SHA-256 in `output/build-report.json` to the Windows signing workflow. Preserve its signed receipt for the CivicCast Lite installer build. Signing is a separate step for the embedded engine and for the outer installer.

## Upstream source and notices

The source package and installer come from the [GStreamer 1.28.6 release](https://gstreamer.freedesktop.org/releases/1.28/) and the [official Windows distribution directory](https://gstreamer.freedesktop.org/data/pkg/windows/1.28.6/msvc/). The Windows setup script and branding files are pinned to the [Cerbero 1.28.6 packaging source](https://github.com/GStreamer/cerbero/tree/59548269f4fd0f701818f0bafdb102959ec81e65). The rebuilt installer retains the vendor license and notices from that distribution.
