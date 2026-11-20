#!/bin/sh
# Build on the target architecture with the locked Rust binaries and upstream runtime package.
set -eu
test "$#" = 4 || { echo "usage: build.sh BINARY_DIR GST_RUNTIME_PKG VERSION OUTPUT_PKG" >&2; exit 2; }
tools_dir=$(cd "$(dirname "$0")" && pwd)
bin=$(cd "$1" && pwd)
runtime=$2
version=$3
out=$4
case "$version" in ''|*[!0-9.]*|.*|*.) exit 2;; esac
test "$(uname -s)" = Darwin
test "$(shasum -a 256 "$runtime" | cut -d' ' -f1)" = a8eb366c59b7e9e5dc049848fed6bcd203a8878aa7517c051639fda78797c6ad
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
root="$stage/root"
app="$root/Library/Application Support/CivicCast Lab/app"
mkdir -p "$app/bin" "$app/media" "$root/Library/LaunchDaemons"
pkgutil --expand-full "$runtime" "$stage/upstream"
for pkg in "$stage/upstream"/*.pkg; do
    case "$(basename "$pkg")" in osx-framework-*) continue;; esac
    if test -d "$pkg/Payload"; then ditto "$pkg/Payload" "$app/media"; fi
done
for exe in channel-worker stationd; do
    install -m 755 "$bin/$exe" "$app/bin/$exe"
    # Replace all SDK build paths so the installed executable uses the bundled libraries.
    otool -l "$app/bin/$exe" | awk '/cmd LC_RPATH/{r=1;next}r&&$1=="path"{print $2;r=0}' |
    while IFS= read -r old; do install_name_tool -delete_rpath "$old" "$app/bin/$exe"; done
    install_name_tool -add_rpath '@executable_path/../media/lib' "$app/bin/$exe"
    codesign --force --sign - "$app/bin/$exe"
done
install -m 755 "$tools_dir/service.sh" "$app/bin/start-station"
install -m 644 "$tools_dir/org.civiccast.lab.plist" "$root/Library/LaunchDaemons/"
plutil -lint "$root/Library/LaunchDaemons/org.civiccast.lab.plist"
mkdir "$stage/scripts"
install -m 755 "$tools_dir/scripts/preinstall" "$tools_dir/scripts/postinstall" "$stage/scripts/"
pkgbuild --root "$root" --scripts "$stage/scripts" --ownership recommended     --identifier org.civiccast.lab.station --version "$version" --install-location / "$out"
