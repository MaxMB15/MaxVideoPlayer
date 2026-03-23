#!/usr/bin/env bash
# Bundle libmpv and its dependencies for Linux AppImage distribution.
# Analogous to bundle-libmpv.sh (macOS). Uses ldd + patchelf instead of dylibbundler.
# CWD when run: workspace root (via beforeBundleCommand cwd)
#
# For .deb/.rpm packages libmpv is declared as a system dependency in
# tauri.conf.json, so this bundling step only matters for AppImage.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
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
  if [[ -n "$LIBMPV_DIR" ]]; then
    # Try versioned names first (libmpv.so.2, libmpv.so.1), then unversioned
    for name in libmpv.so.2 libmpv.so.1 libmpv.so; do
      if [[ -f "$LIBMPV_DIR/$name" ]]; then
        LIBMPV_PATH="$LIBMPV_DIR/$name"
        break
      fi
    done
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
echo "    libmpv: $LIBMPV_PATH"

# Tauri's AppImage bundler places extra libs alongside the binary.
# Set RPATH to $ORIGIN so the binary finds them in the same directory.
BUNDLE_DIR="$TARGET_RELEASE"

# Copy libmpv next to the binary
cp "$LIBMPV_PATH" "$BUNDLE_DIR/libmpv.so"
# Create versioned symlinks that the linker may look for
ln -sf libmpv.so "$BUNDLE_DIR/libmpv.so.2"
ln -sf libmpv.so "$BUNDLE_DIR/libmpv.so.1"

# System libraries that must NOT be bundled — they are always present on the
# target system and bundling them causes ABI conflicts.
SYSTEM_LIBS_RE="linux-vdso|ld-linux|libc\.so|libm\.so|libdl\.so|libpthread"
SYSTEM_LIBS_RE+="|librt\.so|libstdc\+\+|libgcc_s|libresolv|libnss_"
SYSTEM_LIBS_RE+="|libX[a-z]|libxcb|libwayland|libdrm|libgbm"
SYSTEM_LIBS_RE+="|libGL\.so|libEGL\.so|libGLX|libGLdispatch"
SYSTEM_LIBS_RE+="|libgtk|libgdk|libglib|libgobject|libgio|libpango|libcairo|libatk"
SYSTEM_LIBS_RE+="|libdbus|libsystemd|libfontconfig|libfreetype"

# Resolve and copy transitive multimedia dependencies
ldd "$LIBMPV_PATH" | grep "=> /" | awk '{print $3}' | while read -r dep; do
  basename_dep=$(basename "$dep")
  if echo "$basename_dep" | grep -qE "$SYSTEM_LIBS_RE"; then
    continue
  fi
  if [[ ! -f "$BUNDLE_DIR/$basename_dep" ]]; then
    cp "$dep" "$BUNDLE_DIR/"
    echo "    bundled: $basename_dep"
  fi
done

# Set RPATH so the binary and libmpv find co-located libs
patchelf --set-rpath '$ORIGIN' "$BINARY"
patchelf --set-rpath '$ORIGIN' "$BUNDLE_DIR/libmpv.so"

echo "    Done. Bundled libs placed alongside binary in $BUNDLE_DIR"
