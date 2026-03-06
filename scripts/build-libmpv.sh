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
    echo "==> Building/fetching libmpv for macOS..."
    mkdir -p "$LIBS_DIR/macos"

    if command -v brew &>/dev/null; then
      MPV_PREFIX="$(brew --prefix mpv 2>/dev/null || true)"
      if [ -n "$MPV_PREFIX" ] && [ -d "$MPV_PREFIX/lib" ]; then
        echo "    Using Homebrew mpv at $MPV_PREFIX"
        cp "$MPV_PREFIX/lib/libmpv.dylib" "$LIBS_DIR/macos/" 2>/dev/null || \
        cp "$MPV_PREFIX/lib/libmpv.2.dylib" "$LIBS_DIR/macos/libmpv.dylib" 2>/dev/null || \
        echo "    Warning: could not copy dylib"
        echo "    Done."
        exit 0
      fi
    fi

    echo "    No libmpv found. Run: brew install mpv"
    exit 1
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
