#!/bin/sh
# Assemble the GStreamer 1.28.6 runtime archive for civiccast-lab.
set -eu
test "$#" -ge 3 && test "$#" -le 4 || {
    echo "usage: runtime.sh CORE_STAGE_USR GST_RS_PLUGIN_DIR OUTPUT_TAR [CORE_COPYING]" >&2
    exit 2
}
core=$1
rsdir=$2
out=$(realpath -m "$3")
copying=${4:-}
test "$(uname -m)" = x86_64 || { echo "amd64 only" >&2; exit 1; }
test -x "$core/bin/gst-inspect-1.0" || { echo "missing gst-inspect-1.0" >&2; exit 1; }
test -x "$core/libexec/gstreamer-1.0/gst-plugin-scanner" || { echo "missing gst-plugin-scanner" >&2; exit 1; }
test -d "$core/lib/gstreamer-1.0" || { echo "missing core plugins" >&2; exit 1; }
test -d "$rsdir" || { echo "missing rust plugin dir" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
stage=$work/root
install -d "$stage/bin" "$stage/lib/gstreamer-1.0" "$stage/libexec/gstreamer-1.0" "$stage/share/doc" "$stage/share/licenses"

install -m 755 "$core/bin/gst-inspect-1.0" "$stage/bin/"
[ -x "$core/bin/gst-launch-1.0" ] && install -m 755 "$core/bin/gst-launch-1.0" "$stage/bin/"

for f in "$core"/lib/libgstreamer*.so*; do
    [ -e "$f" ] || [ -L "$f" ] || continue
    cp -a "$f" "$stage/lib/"
done

for f in "$core"/lib/gstreamer-1.0/libgstcore*.so; do
    [ -f "$f" ] || { echo "missing core plugin: $f" >&2; exit 1; }
    install -m 755 "$f" "$stage/lib/gstreamer-1.0/"
done
install -m 755 "$core/libexec/gstreamer-1.0/gst-plugin-scanner" "$stage/libexec/gstreamer-1.0/"

pkgs="gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav gstreamer1.0-x"
for pkg in $pkgs; do
    dpkg -L "$pkg" >/dev/null
    dpkg -L "$pkg" | while IFS= read -r f; do
        case "$f" in
            */gstreamer-1.0/*.so)
                [ "$(basename "${f%/*}")" = gstreamer-1.0 ] || continue
                [ -f "$f" ] || continue
                install -m 755 "$f" "$stage/lib/gstreamer-1.0/$(basename "$f")"
                ;;
        esac
    done
    c="/usr/share/doc/$pkg/copyright"
    test -f "$c" || { echo "missing $c" >&2; exit 1; }
    install -d "$stage/share/doc/$pkg"
    install -m 644 "$c" "$stage/share/doc/$pkg/copyright"
done
test -f "$stage/lib/gstreamer-1.0/libgstopenh264.so" || { echo "missing openh264 plugin from gstreamer1.0-plugins-bad" >&2; exit 1; }

for p in libgstrsaudiofx.so libgstrsclosedcaption.so; do
    test -f "$rsdir/$p" || { echo "missing $p" >&2; exit 1; }
    install -m 755 "$rsdir/$p" "$stage/lib/gstreamer-1.0/"
done

cargo=${CARGO_HOME:-"$HOME/.cargo"}
for crate in audiofx closedcaption; do
    dir=
    for d in "$cargo"/registry/src/*/"gst-plugin-$crate-0.15.4"; do
        if [ -d "$d" ]; then dir=$d; break; fi
    done
    test -n "$dir" || { echo "missing cargo source gst-plugin-$crate-0.15.4" >&2; exit 1; }
    licdir="$stage/share/licenses/gst-plugin-$crate-0.15.4"
    install -d "$licdir"
    found=0
    for f in "$dir"/LICENSE* "$dir"/COPYING*; do
        [ -f "$f" ] || continue
        install -m 644 "$f" "$licdir/"
        found=1
    done
    test "$found" = 1 || { echo "missing license files for gst-plugin-$crate-0.15.4" >&2; exit 1; }
done

if [ -n "$copying" ]; then
    test -f "$copying" || { echo "missing core COPYING: $copying" >&2; exit 1; }
else
    for f in "$core/COPYING" "$core/share/doc/gstreamer1.0/COPYING" "$core/share/doc/gstreamer1.0-core/COPYING"; do
        [ -f "$f" ] && { copying=$f; break; }
    done
    test -n "$copying" || { echo "provide core COPYING as 4th argument" >&2; exit 1; }
fi
install -m 644 "$copying" "$stage/COPYING"

{
    echo "GStreamer core: 1.28.6"
    echo "Rust plugins: gst-plugin-audiofx 0.15.4, gst-plugin-closedcaption 0.15.4"
    echo "dpkg packages:"
    dpkg-query -W -f='${Package} ${Version}\n' $pkgs
    echo "sha256:"
    find "$stage/bin" "$stage/libexec" "$stage/lib/gstreamer-1.0" -type f -print | sort | while IFS= read -r f; do
        sha256sum "$f" | sed "s|  $stage/|  |"
    done
} > "$stage/PROVENANCE"

mkdir -p "$(dirname "$out")"
tar -C "$stage" -cf "$out" .
sha256sum "$out" > "$out.sha256"
