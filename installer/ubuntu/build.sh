#!/bin/sh
# Package a native build with its matching, relocatable GStreamer 1.28.6 runtime.
set -eu
test "$#" = 5 || { echo "usage: build.sh BIN_DIR RUNTIME_TAR RUNTIME_SHA256 VERSION OUTPUT_DEB" >&2; exit 2; }
test "$(uname -s)" = Linux
here=$(cd "$(dirname "$0")" && pwd)
bin=$(realpath "$1")
archive=$(realpath "$2")
version=$4
output=$(realpath -m "$5")
printf '%s  %s\n' "$3" "$archive" | sha256sum --check --status
dpkg --validate-version "$version"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
root="$work/package"
app="$root/usr/lib/civiccast-lab"
media="$app/runtime/gstreamer"
install -d "$root/DEBIAN" "$root/usr/lib/systemd/system" "$media"
tar -xf "$archive" -C "$media" --no-same-owner
for exe in stationd channel-worker; do install -m 755 "$bin/$exe" "$app/$exe"; done
export LD_LIBRARY_PATH="$media/lib"
export GST_PLUGIN_SYSTEM_PATH_1_0="$media/lib/gstreamer-1.0"
export GST_PLUGIN_SCANNER="$media/libexec/gstreamer-1.0/gst-plugin-scanner"
export GST_REGISTRY="$work/registry.bin"
test -x "$GST_PLUGIN_SCANNER"
test "$("$media/bin/gst-inspect-1.0" --version | sed -n 's/^GStreamer //p')" = 1.28.6
for factory in openh264enc mpegtsmux hlssink2 rtmp2sink; do "$media/bin/gst-inspect-1.0" "$factory" >/dev/null; done
for exe in stationd channel-worker; do
    ldd "$app/$exe" >"$work/ldd"
    if grep -q 'not found' "$work/ldd"; then cat "$work/ldd" >&2; exit 1; fi
done
install -m 644 "$here/civiccast-lab.service" "$root/usr/lib/systemd/system/"
install -m 755 "$here/bootstrap" "$app/bootstrap"
for script in postinst prerm postrm; do install -m 755 "$here/$script" "$root/DEBIAN/$script"; done
cat > "$root/DEBIAN/control" <<EOF
Package: civiccast-lab
Version: $version
Architecture: $(dpkg --print-architecture)
Maintainer: CivicCast Lab
Depends: systemd, init-system-helpers, adduser, ca-certificates
Description: CivicCast Lab multi-channel station and playout workers
EOF
dpkg-deb --root-owner-group --build "$root" "$output"
sha256sum "$output"
