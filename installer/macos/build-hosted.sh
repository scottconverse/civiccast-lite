#!/bin/sh
set -eu

die() { printf '%s\n' "$*" >&2; exit 1; }

[ "$#" -eq 2 ] || die "usage: $0 VERSION ABSOLUTE_OUTPUT_DIR"
version=$1
out=$2

case "$version" in
    ''|*[!0-9.]*|.*|*.|*..*) die "invalid VERSION: $version" ;;
esac

case "$version" in
    *.*.*.*.*) die "VERSION must have exactly four decimal components: $version" ;;
esac

case "$version" in
    *.*.*.*) ;;
    *) die "VERSION must have exactly four decimal components: $version" ;;
esac

case "$out" in
    /*) ;;
    *) die "ABSOLUTE_OUTPUT_DIR must be absolute: $out" ;;
esac
mkdir -p "$out"

pkg_name="CivicCast-Lite-$version-arm64.pkg"
pkg="$out/$pkg_name"
sha_file="$out/SHA256SUMS.txt"
if [ -e "$pkg" ] || [ -L "$pkg" ]; then
    die "output package already exists: $pkg"
fi
if [ -e "$sha_file" ] || [ -L "$sha_file" ]; then
    die "output checksum file already exists: $sha_file"
fi

[ "$(uname -s)" = Darwin ] || die "macOS required"
[ "$(uname -m)" = arm64 ] || die "Apple Silicon required"
case "$(sw_vers -productVersion)" in
    15.*) ;;
    *) die "macOS 15 required" ;;
esac

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
root=$(CDPATH= cd "$script_dir/../.." && pwd)

build=$(mktemp -d "${TMPDIR:-/tmp}/build-hosted.XXXXXX")
trap 'rm -rf "$build"' 0
trap 'exit 1' HUP INT TERM

prefix="$build/gstreamer"
runtime=gstreamer-1.0-1.28.6-universal.pkg
devel=gstreamer-1.0-devel-1.28.6-universal.pkg
mkdir -p "$prefix"

curl -fL --retry 2 "https://gstreamer.freedesktop.org/data/pkg/osx/1.28.6/$runtime" -o "$build/$runtime"
curl -fL --retry 2 "https://gstreamer.freedesktop.org/data/pkg/osx/1.28.6/$devel" -o "$build/$devel"

(
    cd "$build"
    printf '%s  %s\n' 'a8eb366c59b7e9e5dc049848fed6bcd203a8878aa7517c051639fda78797c6ad' "$runtime" | shasum -a 256 -c -
    printf '%s  %s\n' '177b1428d0f47b844e7bff2aeeb22047686d802eba21580dab52f4a6fe1dcf02' "$devel" | shasum -a 256 -c -
)

for sdk in "$runtime" "$devel"; do
    expanded="$build/$sdk.expanded"
    pkgutil --expand-full "$build/$sdk" "$expanded"
    for part in "$expanded"/*.pkg; do
        case "$(basename "$part")" in
            osx-framework-*) continue ;;
        esac
        if [ -d "$part/Payload" ]; then
            ditto "$part/Payload" "$prefix"
        fi
    done
    rm -rf "$expanded"
done

export PATH="$prefix/bin:$HOME/.cargo/bin:$PATH"
export DYLD_LIBRARY_PATH="$prefix/lib"
export PKG_CONFIG_PATH="$prefix/lib/pkgconfig:$prefix/share/pkgconfig"
export GST_PLUGIN_SYSTEM_PATH_1_0="$prefix/lib/gstreamer-1.0"
export GST_PLUGIN_SCANNER="$prefix/libexec/gstreamer-1.0/gst-plugin-scanner"
export GST_REGISTRY="$build/registry.bin"
export CARGO_HOME="$build/cargo-home"
export CARGO_TARGET_DIR="$build/target"
export CARGO_INCREMENTAL=0
export RUSTFLAGS="-C link-arg=-Wl,-rpath,$prefix/lib -L native=$prefix/lib"

cd "$root"

test "$(pkg-config --modversion gstreamer-1.0)" = 1.28.6
cargo +1.100.0 --version >/dev/null
cargo +1.100.0 build --locked --release --jobs 3 -p stationd -p channel-worker

built_pkg="$build/$pkg_name"
(
    cd "$root"
    sh installer/macos/build.sh "$CARGO_TARGET_DIR/release" "$build/$runtime" "$version" "$built_pkg"
)

validate="$build/validate.pkg"
rm -rf "$validate"
pkgutil --expand "$built_pkg" "$validate"

found=0
for pi in "$validate/PackageInfo" "$validate"/*/PackageInfo; do
    [ -f "$pi" ] || continue
    identifier=$(xmllint --xpath 'string(/pkg-info/@identifier)' "$pi") || die "bad package metadata: $pi"
    pi_version=$(xmllint --xpath 'string(/pkg-info/@version)' "$pi") || die "bad package metadata: $pi"
    [ "$identifier" = org.civiccast.lab.station ] || die "unexpected package identifier in $pi: $identifier"
    [ "$pi_version" = "$version" ] || die "unexpected package version in $pi: $pi_version"
    found=1
done
[ "$found" -eq 1 ] || die "package has no PackageInfo metadata"
if [ -f "$validate/Distribution" ]; then
    grep -q '<installer-gui-script' "$validate/Distribution" || die "bad Distribution metadata"
fi
rm -rf "$validate"

mv "$built_pkg" "$pkg"

hash=$(shasum -a 256 "$pkg" | awk '{print $1}')
printf '%s  %s\n' "$hash" "$(basename "$pkg")" > "$sha_file"
(
    cd "$out"
    shasum -a 256 -c SHA256SUMS.txt
)
