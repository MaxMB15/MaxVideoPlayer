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

# Find libmpv — prefer libs/linux/ (source build), then system pkg-config.
# Source build is preferred because it includes audio outputs we control.
LIBMPV_PATH=""
if [[ -f "$WORKSPACE_ROOT/libs/linux/libmpv.so" ]]; then
  LIBMPV_PATH="$WORKSPACE_ROOT/libs/linux/libmpv.so"
  echo "    Using source-built libmpv from libs/linux/"
elif pkg-config --exists mpv 2>/dev/null; then
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
if [[ -z "$LIBMPV_PATH" ]]; then
  echo "Error: libmpv.so not found in libs/linux/ or via pkg-config"
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

# bundle_all_deps: recursively resolve and copy dependencies to a fixpoint.
# Uses the bundle dir itself as the visited set — if a .so already exists
# there, it's been processed and won't be traversed again.
bundle_all_deps() {
  local queue=("$@")
  while [[ ${#queue[@]} -gt 0 ]]; do
    local lib="${queue[0]}"
    queue=("${queue[@]:1}")
    ldd "$lib" 2>/dev/null | grep "=> /" | awk '{print $3}' | while read -r dep; do
      local basename_dep
      basename_dep=$(basename "$dep")
      if echo "$basename_dep" | grep -qE "$SYSTEM_LIBS_RE"; then
        continue
      fi
      if [[ ! -f "$BUNDLE_DIR/$basename_dep" ]]; then
        cp "$dep" "$BUNDLE_DIR/"
        echo "    bundled: $basename_dep"
        # Append to queue file so the outer loop picks it up
        echo "$BUNDLE_DIR/$basename_dep" >> "$QUEUE_FILE"
      fi
    done
    # Read any newly discovered libs back into the queue
    if [[ -f "$QUEUE_FILE" ]]; then
      while IFS= read -r newlib; do
        queue+=("$newlib")
      done < "$QUEUE_FILE"
      rm -f "$QUEUE_FILE"
    fi
  done
}

QUEUE_FILE=$(mktemp)
trap 'rm -f "$QUEUE_FILE"' EXIT

# Bundle libmpv and all transitive dependencies
bundle_all_deps "$LIBMPV_PATH"

# Set RPATH so the binary and all bundled libs find co-located libs
patchelf --set-rpath '$ORIGIN' "$BINARY"
patchelf --set-rpath '$ORIGIN' "$BUNDLE_DIR/libmpv.so"

# Also set RPATH on all bundled .so files so they can find each other
for so in "$BUNDLE_DIR"/*.so*; do
  if [[ -f "$so" && ! -L "$so" ]]; then
    patchelf --set-rpath '$ORIGIN' "$so" 2>/dev/null || true
  fi
done

# Verify audio output support
echo ""
echo "==> Verifying bundled audio libraries..."
AUDIO_LIBS=0
for pattern in libpulse libasound libpipewire; do
  if ls "$BUNDLE_DIR"/${pattern}* 2>/dev/null | head -1 >/dev/null; then
    echo "    ✓ Found ${pattern}"
    AUDIO_LIBS=$((AUDIO_LIBS + 1))
  fi
done
if [[ $AUDIO_LIBS -eq 0 ]]; then
  echo "    ⚠ WARNING: No audio libraries bundled. AppImage may have no audio output."
  echo "    Install audio dev packages and rebuild libmpv: sudo apt-get install libpulse-dev libasound2-dev"
fi

echo ""
echo "    Done. Bundled libs placed alongside binary in $BUNDLE_DIR"
