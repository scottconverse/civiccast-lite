# Build and package CivicCast Lite from a fresh checkout

Clone `https://github.com/scottconverse/civiccast-lite.git` on a build host for the target platform. Check out the exact revision you intend to build. Record the source commit, toolchain versions and SHA-256 of each package. The names TownLight Station and CivicCast Lab still appear in the packaging and service identifiers.

Git tracks the application, portal, operator console, lockfiles and packaging scripts. Build inputs also come from the tool and library publishers listed below. Station databases, test footage, saved credentials and private update keys are unnecessary for a build.

## GitHub-hosted installer candidates

Run [Build installers](https://github.com/scottconverse/civiccast-lite/actions/workflows/build-installers.yml) from the branch you want to package. The three jobs check out the same source revision on fresh Windows x64, macOS arm64, and Ubuntu 26.04 amd64 runners. Set the package version for macOS and Ubuntu at dispatch. The Windows job uses the product version in the Inno Setup definition.

Download the three artifacts from the workflow run. Verify the Windows setup SHA-256 against `candidate-manifest.json` and the macOS and Ubuntu packages against their `SHA256SUMS.txt` files. The run page identifies the source commit. These are build candidates. The Windows setup is unsigned, and the release signing procedure remains separate.

Select `use_signed_gstreamer` to embed the exact re-signed Windows GStreamer installer shipped as a [v0.1.0.13 build input](https://github.com/scottconverse/civiccast-lite/releases/tag/v0.1.0.13). The Windows SDK installation uses the pinned upstream installer. The workflow verifies the SHA-256 and Authenticode provenance of the redistributable before embedding it.

## Windows x64

1. Install Rust using [rustup](https://rustup.rs/) and select `rustup toolchain install 1.97.1-x86_64-pc-windows-msvc`. Install [Node.js 22](https://nodejs.org/dist/latest-v22.x/), including npm. The reference host used Node.js 22.23.5. The Windows build rebuilds the console using its npm lockfile.
2. Install [Visual Studio 2022 Build Tools 17.14](https://learn.microsoft.com/en-us/visualstudio/releases/2022/release-history) at the standard Build Tools path. The reference host used 17.14.41 (17.14.37710.0). Choose x64/x86 MSVC v143 14.44 and the Windows SDK 10.0.26100 components (`Microsoft.VisualStudio.Component.VC.14.44.17.14.x86.x64`, `Microsoft.VisualStudio.Component.Windows11SDK.26100`). The packager checks the exact MSVC tool directory `14.44.35207` and compiler file version `14.44.35229.0`.
3. Install [Inno Setup 6.7.3](https://github.com/jrsoftware/issrc/releases/download/is-6_7_3/innosetup-6.7.3.exe) for the current user into `%LOCALAPPDATA%\Programs\Inno Setup 6`. The installer SHA-256 is `9c73c3bae7ed48d44112a0f48e66742c00090bdb5bef71d9d3c056c66e97b732`. The packager validates the installed compiler hash, size and Authenticode signer.
4. Download these exact files into the given subdirectories of `%LOCALAPPDATA%\Temp`. Keep their upstream filenames. Compare the digest with `Get-FileHash -Algorithm SHA256` before using each file.

| File | Subdirectory | SHA-256 |
| --- | --- | --- |
| [GStreamer 1.28.6 MSVC x64 installer](https://gstreamer.freedesktop.org/data/pkg/windows/1.28.6/msvc/gstreamer-1.0-msvc-x86_64-1.28.6.exe) | `TownLightStation-Toolchain-20260820` | `059251444d1267b486eba390b18d25fed87e10315e72f757ec6c7e912fa746b5` |
| [FFmpeg LGPL caption decoder ZIP](https://github.com/FFmpeg-Builds/FFmpeg-Builds/releases/download/autobuild-2026-08-31-13-27/ffmpeg-n9.0.1-11-ge47273f4d9-win64-lgpl-9.0.zip) | `TownLightStation-Toolchain-20260831` | `2484854ad6988d34560f4e6ea7a6ecb9dde0af7c229d2591815d056b04ec4f56` |
| [TSDuck 3.44-4676 portable ZIP](https://github.com/tsduck/tsduck/releases/download/v3.44-4676/TSDuck-Win64-3.44-4676-Portable.zip) | `TownLightStation-Toolchain-20260826` | `b0ca0f963fcd77488b8c32d6f9d85030daa891ed753c36e57bbb458807a44eb1` |

Install GStreamer with the development preset (`/TYPE=devel`) to `%LOCALAPPDATA%\Programs\gstreamer\1.0\msvc_x86_64`. The [Windows workflow](../.github/workflows/windows-kernel.yml) shows the upstream installer's noninteractive options and SDK checks. The SDK supplies `pkg-config.exe` in `bin`, and the wrapper puts it on PATH. Set `PKG_CONFIG_PATH` to the SDK's `lib\pkgconfig` for this build shell. From the checkout root, restore the locked dependencies before invoking the offline native build:

```powershell
$env:TOWNLIGHT_CARGO_EXECUTABLE = Join-Path $env:USERPROFILE '.rustup\toolchains\1.97.1-x86_64-pc-windows-msvc\bin\cargo.exe'
$env:TOWNLIGHT_GSTREAMER_ROOT = Join-Path $env:LOCALAPPDATA 'Programs\gstreamer\1.0\msvc_x86_64'
$env:PKG_CONFIG_PATH = Join-Path $env:TOWNLIGHT_GSTREAMER_ROOT 'lib\pkgconfig'
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File scripts/invoke-cargo.ps1 fetch --locked
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File installer/build-installer.ps1
```

The build emits the candidate setup and `candidate-manifest.json` under `dist`. The manifest records the payload hashes and source commit. If rebuilding the console changes tracked files, commit the intended changes before making the release candidate.

For the signed redistributable, download `gstreamer-1.28.6-windows-x64-civiccast-signed.exe` from `v0.1.0.13` and verify SHA-256 `daf7cf53c5ed1d58b802143d34fea6645d8ac92c3a954e0219d7d3a42b135d4f`. Store it alongside the upstream installer in `%LOCALAPPDATA%\Temp\TownLightStation-Toolchain-20260820`, then pass `-UseSignedGStreamer` to `installer/build-installer.ps1`. The build checks the committed signing receipt and the file's Authenticode signature and timestamp.

### Windows release signing and assembly

Maintainers use the [CivicCast GitHub signing workflow](https://github.com/scottconverse/civiccast/blob/main/.github/workflows/sign-native-installer.yml) and its existing Azure Trusted Signing configuration. The publisher identity lives in Azure. Release signing needs an authorized GitHub maintainer and access to that workflow. The build host needs no signing key.

The [GStreamer rebuild recipe](GSTREAMER-WINDOWS.md) covers the package used for the Halo check. It signs the Inno setup engine before embedding it, then signs and verifies the outer GStreamer installer. The vendor runtime files remain byte-for-byte unchanged. The Windows Sandbox policy used during testing rejected four unsigned GStreamer runtime DLLs even after the setup engine was accepted. The resulting CivicCast package passed a clean native Windows installation on Halo.

1. Take `stationd.exe`, `channel-worker.exe`, `nas-publisher.exe`, `station-automation-launch-helper.exe` and `station-updater.exe` from the exact candidate build. Match their SHA-256 values to the manifest. Submit the exact files individually through the signer's draft-release intake using `release_tag`, `asset_name` and lowercase `expected_sha256`. Retrieve each `signed-native-installer` artifact and its `signing-receipt.json`. Verify the returned SHA-256, Authenticode publisher and timestamp. Retain the receipts.
2. Use the ISCC invocation near the end of [build-installer.ps1](../installer/build-installer.ps1) as the compiler define list. Keep the same source commit and pinned third-party inputs. For `v0.1.0.13`, use the verified signed GStreamer installer and its SHA-256 from the signed-input candidate manifest. Set the five executable path, SHA-256 and size definitions from the signed files. Extract `bin/ffmpeg.exe` and `LICENSE.txt` from the pinned FFmpeg ZIP and verify their size and hash against the packager. Use a dedicated output directory.
3. Compile with a temporary wrapper that includes the checkout's `installer/TownLightStation.iss`. Replace the two absolute example paths below with paths on the build host.

```ini
[Setup]
SourceDir=C:/path/to/civiccast-lite/installer
SignedUninstaller=yes
SignedUninstallerDir=C:/path/to/empty/uninstaller-staging
#include "C:/path/to/civiccast-lite/installer/TownLightStation.iss"
```

4. On the first compile, ISCC generates an unsigned `uninst-6.7.3-*.e32` and requests a signed copy. Copy the PE file to a temporary `.exe` name and sign those exact bytes through the same workflow. Verify the signed file and put it back under the original `.e32` name. Run the identical compile again with the signed uninstaller and signed payloads.
5. Send the resulting outer setup EXE through the same workflow. Verify its signature and timestamp, and compute the release checksum from the signed setup. Preserve the manifest and receipts from all signing steps with the build records.

## macOS arm64

Use an Apple Silicon host with Xcode Command Line Tools and Rust 1.100.0 from rustup. The recipe recreates the extracted GStreamer SDK, including its `pkg-config`, used by the release build. Run from the checkout root.

```sh
set -eu
version=0.1.0.13                      # Set the version being built.
build=$(mktemp -d)
prefix="$build/gstreamer"
runtime=gstreamer-1.0-1.28.6-universal.pkg
devel=gstreamer-1.0-devel-1.28.6-universal.pkg
mkdir -p "$prefix"

curl -fL --retry 2 "https://gstreamer.freedesktop.org/data/pkg/osx/1.28.6/$runtime" -o "$build/$runtime"
curl -fL --retry 2 "https://gstreamer.freedesktop.org/data/pkg/osx/1.28.6/$devel" -o "$build/$devel"
(
    cd "$build"
    printf '%s  %s\n' \
        a8eb366c59b7e9e5dc049848fed6bcd203a8878aa7517c051639fda78797c6ad "$runtime" \
        177b1428d0f47b844e7bff2aeeb22047686d802eba21580dab52f4a6fe1dcf02 "$devel" |
        shasum -a 256 -c -
)
for sdk in "$runtime" "$devel"; do
    pkgutil --expand-full "$build/$sdk" "$build/$sdk.expanded"
    for part in "$build/$sdk.expanded/"*.pkg; do
        case "$(basename "$part")" in osx-framework-*) continue;; esac
        if test -d "$part/Payload"; then
            ditto "$part/Payload" "$prefix"
        fi
    done
done
export PATH="$prefix/bin:$HOME/.cargo/bin:$PATH"
export DYLD_LIBRARY_PATH="$prefix/lib"
export PKG_CONFIG_PATH="$prefix/lib/pkgconfig:$prefix/share/pkgconfig"
export GST_PLUGIN_SYSTEM_PATH_1_0="$prefix/lib/gstreamer-1.0"
export GST_PLUGIN_SCANNER="$prefix/libexec/gstreamer-1.0/gst-plugin-scanner"
export GST_REGISTRY="$build/registry.bin"
export CARGO_TARGET_DIR="$build/target"
export RUSTFLAGS="-C link-arg=-Wl,-rpath,$prefix/lib -L native=$prefix/lib"
test "$(pkg-config --modversion gstreamer-1.0)" = 1.28.6
cargo +1.100.0 build --locked --release -p stationd -p channel-worker
sh installer/macos/build.sh "$CARGO_TARGET_DIR/release" "$build/$runtime" \
    "$version" "$build/CivicCast-Lite-$version-arm64.pkg"
shasum -a 256 "$build/CivicCast-Lite-$version-arm64.pkg"
```

The [package script](../installer/macos/build.sh) sets a relative loader path and applies an ad hoc signature to each station executable. This is the signing level used by the existing macOS release.

## Ubuntu 26.04 amd64

Use Ubuntu 26.04 on x86-64 and Rust 1.100.0 from rustup. The package combines GStreamer 1.28.6 core, the distro's matching 1.28-series plugin packages and two Rust plugins from crates.io. Use the pinned Cargo lockfiles. Keep the runtime's generated `PROVENANCE` for the exact distro package versions and bundled hashes.

```sh
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
    build-essential ca-certificates curl xz-utils pkgconf meson ninja-build flex bison \
    libglib2.0-dev libssl-dev libpango1.0-dev libcairo2-dev libgdk-pixbuf-2.0-dev \
    libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
    gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad \
    gstreamer1.0-plugins-ugly gstreamer1.0-libav gstreamer1.0-x
```

From the checkout root:

```sh
set -eu
version=0.1.0.13                      # Set the version being built.
build=$(mktemp -d)
source=gstreamer-1.28.6.tar.xz
curl -fL --retry 2 "https://gstreamer.freedesktop.org/src/gstreamer/$source" -o "$build/$source"
(
    cd "$build"
    printf '%s  %s\n' 62b6b9f0ad3147a6dd6420ac64a91180b14e990695bddd353b96041611d052ca "$source" |
        sha256sum -c -
    tar -xf "$source"
    meson setup core-build gstreamer-1.28.6 --prefix=/usr --libdir=lib \
        -Dtests=disabled -Dexamples=disabled -Dintrospection=disabled -Ddoc=disabled \
        --wrap-mode=nofallback
    ninja -C core-build
    DESTDIR="$build/core-stage" meson install -C core-build
)

export CARGO_TARGET_DIR="$build/plugins"
export CARGO_PROFILE_DEV_DEBUG=0
for plugin in audiofx closedcaption; do
    cargo +1.100.0 info "gst-plugin-$plugin@0.15.4"
    for manifest in "${CARGO_HOME:-$HOME/.cargo}"/registry/src/*/"gst-plugin-$plugin-0.15.4/Cargo.toml"; do
        if test -f "$manifest"; then
            cargo +1.100.0 build --locked --manifest-path "$manifest"
            break
        fi
    done
done
sh installer/ubuntu/runtime.sh "$build/core-stage/usr" "$CARGO_TARGET_DIR/debug" \
    "$build/runtime.tar" "$build/gstreamer-1.28.6/COPYING"
unset CARGO_PROFILE_DEV_DEBUG

export CARGO_TARGET_DIR="$build/target"
cargo +1.100.0 build --locked -p stationd -p channel-worker
runtime_sha=$(sha256sum "$build/runtime.tar" | cut -d ' ' -f 1)
sh installer/ubuntu/build.sh "$CARGO_TARGET_DIR/debug" "$build/runtime.tar" \
    "$runtime_sha" "$version" "$build/CivicCast-Lite-$version-ubuntu26-amd64.deb"
sha256sum "$build/CivicCast-Lite-$version-ubuntu26-amd64.deb"
```

The reference Linux package uses Cargo's default profile. The app embeds the checked-in console assets on Unix. To refresh them after a frontend change, run `npm ci --ignore-scripts --no-audit` and `npm run build` in `web/operator` and commit the updated assets.
