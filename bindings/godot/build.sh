#!/usr/bin/env bash
# Builds the extension and copies it into the demo project's addon folder (Linux).
# Usage:  ./build.sh            (debug build)
#         ./build.sh release
set -euo pipefail
profile="${1:-debug}"
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"

cd "$root"
if [ "$profile" = "release" ]; then cargo build -p manykey-godot --release; else cargo build -p manykey-godot; fi

dest="$here/demo/addons/manykey/bin/$profile"
mkdir -p "$dest"
cp "$root/target/$profile/libmanykey_godot.so" "$dest/"
echo "Copied libmanykey_godot.so to $dest"
