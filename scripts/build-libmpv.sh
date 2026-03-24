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
    # Pin to a stable release to avoid bleeding-edge dependency bumps (e.g. libplacebo >=7.360.1)
    MPV_TAG="v0.40.0"
    if [[ ! -d "$MPV_SRC/.git" ]]; then
      echo "    Cloning mpv source (${MPV_TAG})..."
      git clone https://github.com/mpv-player/mpv.git --depth=1 --branch "$MPV_TAG" "$MPV_SRC"
    else
      echo "    mpv source already present, skipping clone."
    fi

    # Patch: mpv 0.40.0 uses FF_PROFILE_* macros removed in ffmpeg 8.x.
    # Replace with the AV_PROFILE_* equivalents (available since ffmpeg 5.0).
    DEMUX_MKV="$MPV_SRC/demux/demux_mkv.c"
    if grep -q 'FF_PROFILE_ARIB' "$DEMUX_MKV" 2>/dev/null; then
      echo "    Patching demux_mkv.c: FF_PROFILE_* -> AV_PROFILE_* ..."
      # macOS sed requires an explicit backup extension with -i; use '' for in-place
      sed -i.bak \
        -e 's/FF_PROFILE_ARIB_PROFILE_A/AV_PROFILE_ARIB_PROFILE_A/g' \
        -e 's/FF_PROFILE_ARIB_PROFILE_C/AV_PROFILE_ARIB_PROFILE_C/g' \
        -e 's/FF_PROFILE_UNKNOWN/AV_PROFILE_UNKNOWN/g' \
        "$DEMUX_MKV"
      rm -f "${DEMUX_MKV}.bak"
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

  linux)
    echo "==> Building libmpv from source for Linux..."
    mkdir -p "$LIBS_DIR/linux"

    # Check build tools
    for tool in meson ninja pkg-config; do
      if ! command -v "$tool" &>/dev/null; then
        echo "Error: $tool not found. Run: sudo apt-get install meson ninja-build pkg-config"
        exit 1
      fi
    done

    if ! pkg-config --exists libavcodec; then
      echo "Error: ffmpeg dev packages not found."
      echo "Run: sudo apt-get install libavcodec-dev libavformat-dev libavutil-dev libswscale-dev libavfilter-dev"
      exit 1
    fi

    # Clone mpv source (same version as macOS for consistency)
    MPV_SRC="$LIBS_DIR/mpv-src"
    MPV_TAG="v0.40.0"
    if [[ ! -d "$MPV_SRC/.git" ]]; then
      echo "    Cloning mpv source (${MPV_TAG})..."
      git clone https://github.com/mpv-player/mpv.git --depth=1 --branch "$MPV_TAG" "$MPV_SRC"
    else
      echo "    mpv source already present, skipping clone."
    fi

    # Build
    BUILD_DIR="$MPV_SRC/build-linux"
    echo "    Running meson setup..."
    meson setup "$BUILD_DIR" "$MPV_SRC" \
      --buildtype=release \
      --wipe \
      -Dlibmpv=true \
      -Dgl=enabled \
      -Dvulkan=disabled \
      -Dwayland=enabled \
      -Dx11=enabled \
      -Degl=enabled

    echo "    Building libmpv.so (this takes a few minutes)..."
    ninja -C "$BUILD_DIR" libmpv.so.2

    # Copy .so to libs/linux/
    SO=$(find "$BUILD_DIR" -name "libmpv.so*" -not -type l | head -1)
    if [[ -z "$SO" ]]; then
      echo "Error: libmpv.so not found after build"
      exit 1
    fi
    rm -f "$LIBS_DIR/linux/libmpv.so" "$LIBS_DIR/linux/libmpv.so.2"
    cp "$SO" "$LIBS_DIR/linux/libmpv.so"
    ln -sf libmpv.so "$LIBS_DIR/linux/libmpv.so.2"
    echo "    Built libmpv -> $LIBS_DIR/linux/libmpv.so"
    echo "    Done."
    ;;

  *)
    echo "Usage: $0 {macos|linux|ios|android}"
    echo ""
    echo "Downloads or builds libmpv for the target platform."
    echo "Output goes to libs/<platform>/"
    exit 1
    ;;
esac
