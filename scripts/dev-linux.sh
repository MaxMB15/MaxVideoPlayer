#!/usr/bin/env bash
# Wrapper for `tauri dev` on Linux that ensures libmpv is built from source
# (with audio outputs) and loadable at runtime.
#
# System libmpv packages on some distros (e.g. Pop!_OS) ship without audio
# output plugins compiled in, which breaks playback audio in dev. Building
# from source via scripts/build-libmpv.sh guarantees ALSA/Pulse/PipeWire
# support if the dev headers are installed.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
LIBS_LINUX="$WORKSPACE_ROOT/libs/linux"

if [[ ! -f "$LIBS_LINUX/libmpv.so" ]]; then
  echo "==> libs/linux/libmpv.so not found — building libmpv from source..."
  "$SCRIPT_DIR/build-libmpv.sh" linux
fi

export LD_LIBRARY_PATH="$LIBS_LINUX:${LD_LIBRARY_PATH:-}"
echo "==> LD_LIBRARY_PATH=$LD_LIBRARY_PATH"

exec npm run tauri dev --workspace=apps/desktop
