#!/usr/bin/env bash
# Bundle libmpv and its dependencies into the macOS app for distribution.
# Run as beforeBundleCommand during tauri build. Requires: brew install mpv dylibbundler
# CWD when run: workspace root (via beforeBundleCommand cwd)
# No-op on non-macOS (e.g. when building on Windows/Linux).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
LIBS_BUNDLE="$WORKSPACE_ROOT/libs/macos-bundle"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Skipping libmpv bundle (not macOS)"
  mkdir -p "$LIBS_BUNDLE"
  exit 0
fi
LIBS_DIR="$WORKSPACE_ROOT/libs"
LIBS_MACOS="$LIBS_DIR/macos"
TARGET_RELEASE="$WORKSPACE_ROOT/target/release"
APP_NAME="max_video_player"

# Ensure libmpv exists (from build-libmpv.sh)
if [[ ! -f "$LIBS_MACOS/libmpv.dylib" && ! -f "$LIBS_MACOS/libmpv.2.dylib" ]]; then
  echo "Error: libmpv not found. Run ./scripts/build-libmpv.sh macos first."
  exit 1
fi

# Binary must exist (cargo build runs before beforeBundleCommand)
BINARY="$TARGET_RELEASE/$APP_NAME"
if [[ ! -f "$BINARY" ]]; then
  echo "Error: Binary not found at $BINARY"
  exit 1
fi

# dylibbundler must be installed
if ! command -v dylibbundler &>/dev/null; then
  echo "Error: dylibbundler not found. Run: brew install dylibbundler"
  exit 1
fi

# Create output dir and run dylibbundler
rm -rf "$LIBS_BUNDLE"
mkdir -p "$LIBS_BUNDLE"

dylibbundler -od -b -x "$BINARY" -d "$LIBS_BUNDLE" -p "@executable_path/../Frameworks"

echo "Bundled libmpv and dependencies to $LIBS_BUNDLE"
