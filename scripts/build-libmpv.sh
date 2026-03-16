#!/usr/bin/env bash
set -euo pipefail

# Build/download libmpv for target platform.
# Output: libs/<platform>/ (workspace root — desktop uses macos/linux/windows, ios uses ios/, android uses android/)
#
# Usage: ./scripts/build-libmpv.sh macos|ios|android

PLATFORM="${1:-}"
LIBS_DIR="$(cd "$(dirname "$0")/.." && pwd)/libs"

case "$PLATFORM" in
  macos)
    echo "==> Building libmpv from source for macOS..."
    mkdir -p "$LIBS_DIR/macos"

    # Check build tools
    for tool in meson ninja pkg-config; do
      if ! command -v "$tool" &>/dev/null; then
        echo "Error: $tool not found. Run: brew install meson ninja pkg-config"
        exit 1
      fi
    done

    if ! pkg-config --exists libavcodec; then
      echo "Error: ffmpeg not found. Run: brew install ffmpeg"
      exit 1
    fi

    # Clone mpv source (shallow, latest)
    MPV_SRC="$LIBS_DIR/mpv-src"
    if [[ ! -d "$MPV_SRC/.git" ]]; then
      echo "    Cloning mpv source..."
      git clone https://github.com/mpv-player/mpv.git --depth=1 "$MPV_SRC"
    else
      echo "    mpv source already present, skipping clone."
    fi

    # Build
    BUILD_DIR="$MPV_SRC/build-macos"
    echo "    Running meson setup..."
    meson setup "$BUILD_DIR" "$MPV_SRC" \
      --buildtype=release \
      --wipe \
      -Dlibmpv=true \
      -Dgl=enabled \
      -Dvulkan=disabled \
      -Dlibplacebo=disabled \
      -Dcocoa=enabled

    echo "    Building libmpv dylib only (this takes a few minutes)..."
    ninja -C "$BUILD_DIR" libmpv.2.dylib

    # Copy dylib to libs/macos/
    DYLIB=$(find "$BUILD_DIR" -name "libmpv*.dylib" | head -1)
    if [[ -z "$DYLIB" ]]; then
      echo "Error: libmpv.dylib not found after build"
      exit 1
    fi
    rm -f "$LIBS_DIR/macos/libmpv.dylib" "$LIBS_DIR/macos/libmpv.2.dylib"
    cp "$DYLIB" "$LIBS_DIR/macos/libmpv.dylib"
    # Symlink for the versioned install name (@rpath/libmpv.2.dylib)
    ln -sf libmpv.dylib "$LIBS_DIR/macos/libmpv.2.dylib"
    echo "    Built libmpv -> $LIBS_DIR/macos/libmpv.dylib"
    echo "    Done."
    ;;

  ios)
    echo "==> Building libmpv for iOS (arm64)..."
    mkdir -p "$LIBS_DIR/ios"
    echo "    iOS static library build requires cross-compiling mpv + ffmpeg"
    echo "    for aarch64-apple-ios. Use mpv-build with iOS toolchain or"
    echo "    download pre-built from: https://github.com/nichobi/iina-plus"
    echo "    Place libmpv.a in libs/ios/ (for apps/ios)"
    ;;

  android)
    echo "==> Building libmpv for Android..."
    mkdir -p "$LIBS_DIR/android/arm64-v8a"
    mkdir -p "$LIBS_DIR/android/armeabi-v7a"
    echo "    Android .so build requires NDK cross-compilation."
    echo "    Use mpv-android build scripts or download pre-built from:"
    echo "    https://github.com/nichobi/mpv-android"
    echo "    Place libmpv.so in libs/android/<abi>/ (for apps/android)"
    ;;

  *)
    echo "Usage: $0 {macos|ios|android}"
    echo ""
    echo "Downloads or builds libmpv for the target platform."
    echo "Output goes to libs/<platform>/"
    exit 1
    ;;
esac
