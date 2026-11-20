#!/bin/sh
set -eu
base="/Library/Application Support/CivicCast Lab"
media="$base/app/media"
export PATH="$media/bin:/usr/bin:/bin"
export GST_PLUGIN_SYSTEM_PATH_1_0="$media/lib/gstreamer-1.0"
export GST_PLUGIN_SCANNER="$media/libexec/gstreamer-1.0/gst-plugin-scanner"
export GST_REGISTRY="$base/state/gst-registry.bin"
export CIVICCAST_CREDENTIAL_DIRECTORY="$base/credentials"
export CIVICCAST_PORTAL_ROOT="/Library/WebServer/Documents/civiccast-lab"
export HOME="$base/state"
# Finish plugin discovery before a supervised worker starts its handshake deadline.
"$media/bin/gst-inspect-1.0" mpegtsmux >/dev/null
exec "$base/app/bin/stationd" service --database "$base/state/station.db" --address 127.0.0.1:8787
