#!/usr/bin/env bash
# Bundle libmpv and its dependencies for Linux AppImage distribution.
# Analogous to bundle-libmpv.sh (macOS). Uses ldd + patchelf instead of dylibbundler.
# CWD when run: workspace root (via beforeBundleCommand cwd)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
LIBS_BUNDLE="$WORKSPACE_ROOT/libs/linux-bundle"
TARGET_RELEASE="$WORKSPACE_ROOT/target/release"
BINARY="$TARGET_RELEASE/max-video-player"

# Ensure binary exists
if [[ ! -f "$BINARY" ]]; then
  echo "Error: Binary not found at $BINARY"
  exit 1
fi

# Check for patchelf
if ! command -v patchelf &>/dev/null; then
  echo "Error: patchelf not found. Run: sudo apt-get install patchelf"
  exit 1
fi

# Find libmpv — prefer system pkg-config, then libs/linux/
LIBMPV_PATH=""
if pkg-config --exists mpv 2>/dev/null; then
  LIBMPV_DIR=$(pkg-config --variable=libdir mpv 2>/dev/null || echo "")
  if [[ -n "$LIBMPV_DIR" && -f "$LIBMPV_DIR/libmpv.so" ]]; then
    LIBMPV_PATH="$LIBMPV_DIR/libmpv.so"
  fi
fi
if [[ -z "$LIBMPV_PATH" && -f "$WORKSPACE_ROOT/libs/linux/libmpv.so" ]]; then
  LIBMPV_PATH="$WORKSPACE_ROOT/libs/linux/libmpv.so"
fi
if [[ -z "$LIBMPV_PATH" ]]; then
  echo "Error: libmpv.so not found via pkg-config or libs/linux/"
  exit 1
fi

echo "==> Bundling libmpv for Linux AppImage..."
rm -rf "$LIBS_BUNDLE"
mkdir -p "$LIBS_BUNDLE"

# Copy libmpv
cp "$LIBMPV_PATH" "$LIBS_BUNDLE/"

# Resolve and copy transitive dependencies (excluding system libs)
SYSTEM_LIBS="linux-vdso|ld-linux|libc.so|libm.so|libdl.so|libpthread|librt.so|libstdc++"
ldd "$LIBMPV_PATH" | grep "=> /" | awk '{print $3}' | while read -r dep; do
  basename_dep=$(basename "$dep")
  # Skip system libraries that are always present
  if echo "$basename_dep" | grep -qE "$SYSTEM_LIBS"; then
    continue
  fi
  if [[ ! -f "$LIBS_BUNDLE/$basename_dep" ]]; then
    cp "$dep" "$LIBS_BUNDLE/"
  fi
done

# Set RPATH on the main binary so it finds bundled libs
patchelf --set-rpath '$ORIGIN/lib' "$BINARY" 2>/dev/null || true

echo "Bundled libmpv and dependencies to $LIBS_BUNDLE"
