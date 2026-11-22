#!/bin/sh
set -eu

fail() { printf 'error: %s\n' "$*" >&2; exit 1; }
usage() { printf 'usage: %s VERSION ABSOLUTE_OUTPUT_DIR\n' "$0" >&2; exit 2; }

[ $# -eq 2 ] || usage
VERSION=$1
OUTDIR=$2

nl='
'
case $VERSION in
    *[!0-9.]*|*"$nl"*) fail "VERSION contains unsafe characters: $VERSION" ;;
esac
printf '%s\n' "$VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$' \
    || fail "VERSION must have four decimal components: $VERSION"

case $OUTDIR in
    /*) ;;
    *) fail "output dir must be absolute: $OUTDIR" ;;
esac

pkgname="CivicCast-Lite-$VERSION-ubuntu26-amd64.deb"
for f in "$OUTDIR/SHA256SUMS.txt" "$OUTDIR/$pkgname"; do
    if [ -e "$f" ] || [ -L "$f" ]; then
        fail "output destination already exists: $f"
    fi
done

[ "$(uname -s)" = Linux ] || fail "requires Linux"
[ "$(uname -m)" = x86_64 ] || fail "requires x86_64"
[ "$(dpkg --print-architecture)" = amd64 ] || fail "requires dpkg arch amd64"

[ -r /etc/os-release ] || fail "cannot read /etc/os-release"
os_id=$(unset ID VERSION_ID; . /etc/os-release; printf '%s' "${ID:-}")
os_version_id=$(unset ID VERSION_ID; . /etc/os-release; printf '%s' "${VERSION_ID:-}")
[ "$os_id" = ubuntu ] || fail "requires native Ubuntu"
[ "$os_version_id" = 26.04 ] || fail "requires Ubuntu 26.04"

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)
cd "$ROOT"

command -v rustup >/dev/null 2>&1 || fail "rustup not found"
rustup toolchain list 2>/dev/null | grep -Eq '^1\.100\.0' \
    || fail "rustup toolchain 1.100.0 is not installed"
rustc +1.100.0 --version 2>/dev/null | grep -q '^rustc 1\.100\.0 ' \
    || fail "rustc +1.100.0 did not report 1.100.0"

export DEBIAN_FRONTEND=noninteractive
export NEEDRESTART_MODE=a
sudo -E apt-get update
sudo -E apt-get install -y --no-install-recommends \
    build-essential ca-certificates curl xz-utils pkgconf meson ninja-build flex bison \
    libglib2.0-dev libssl-dev libpango1.0-dev libcairo2-dev libgdk-pixbuf-2.0-dev \
    libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
    gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad \
    gstreamer1.0-plugins-ugly gstreamer1.0-libav gstreamer1.0-x

build=
cleanup() { [ -z "${build:-}" ] || rm -rf "$build"; }
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
build=$(mktemp -d)

gst_url='https://gstreamer.freedesktop.org/src/gstreamer/'
gst_file='gstreamer-1.28.6.tar.xz'
gst_sha='62b6b9f0ad3147a6dd6420ac64a91180b14e990695bddd353b96041611d052ca'

curl -fL --retry 2 "$gst_url$gst_file" -o "$build/$gst_file"
(
    cd "$build"
    printf '%s  %s\n' "$gst_sha" "$gst_file" | sha256sum -c -
)

tar -xf "$build/$gst_file" -C "$build"
meson setup "$build/core-build" "$build/gstreamer-1.28.6" \
    --prefix=/usr --libdir=lib \
    -Dtests=disabled -Dexamples=disabled -Dintrospection=disabled -Ddoc=disabled \
    --wrap-mode=nofallback
ninja -C "$build/core-build" -j3
DESTDIR="$build/core-stage" meson install -C "$build/core-build"

export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_INCREMENTAL=0

for plugin in audiofx closedcaption; do
    cargo +1.100.0 info "gst-plugin-$plugin@0.15.4" >/dev/null
    found=
    for manifest in "${CARGO_HOME:-$HOME/.cargo}"/registry/src/*/"gst-plugin-$plugin-0.15.4/Cargo.toml"; do
        [ -f "$manifest" ] || continue
        CARGO_TARGET_DIR="$build/plugins" \
            cargo +1.100.0 build --locked -j 3 --manifest-path "$manifest"
        found=1
        break
    done
    [ -n "$found" ] || fail "no lockfile for gst-plugin-$plugin-0.15.4"
done

unset CARGO_PROFILE_DEV_DEBUG

sh installer/ubuntu/runtime.sh \
    "$build/core-stage/usr" \
    "$build/plugins/debug" \
    "$build/runtime.tar" \
    "$build/gstreamer-1.28.6/COPYING"

CARGO_TARGET_DIR="$build/target" \
    cargo +1.100.0 build --locked -j 3 -p stationd -p channel-worker

runtime_sha=$(sha256sum "$build/runtime.tar" | cut -d ' ' -f 1)

sh installer/ubuntu/build.sh \
    "$build/target/debug" \
    "$build/runtime.tar" \
    "$runtime_sha" \
    "$VERSION" \
    "$build/$pkgname"

test -f "$build/$pkgname" || fail "installer not produced"
test -s "$build/$pkgname" || fail "installer is empty"
dpkg-deb --info "$build/$pkgname" >/dev/null || fail "invalid deb format"
dpkg-deb --contents "$build/$pkgname" >/dev/null || fail "invalid deb contents"
[ "$(dpkg-deb -f "$build/$pkgname" Architecture)" = amd64 ] \
    || fail "installer architecture is not amd64"
pkgver=$(dpkg-deb -f "$build/$pkgname" Version)
[ "$pkgver" = "$VERSION" ] \
    || fail "installer version $pkgver does not match $VERSION"

mkdir -p "$OUTDIR"
OUTDIR=$(CDPATH= cd -- "$OUTDIR" && pwd)
mv "$build/$pkgname" "$OUTDIR/$pkgname"
(
    cd "$OUTDIR"
    sha256sum "$pkgname" > SHA256SUMS.txt
    sha256sum --check SHA256SUMS.txt
)
