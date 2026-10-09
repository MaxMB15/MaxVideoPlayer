#!/usr/bin/env bash
# Bundle libmpv and its dependencies into the macOS app for distribution.
# Run as beforeBundleCommand during tauri build. Requires: brew install mpv dylibbundler
# CWD when run: workspace root (via beforeBundleCommand cwd)
# No-op on non-macOS (e.g. when building on Windows/Linux).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
LIBS_BUNDLE="$WORKSPACE_ROOT/libs/macos-bundle"

if [[ "$(uname -s)" == "Linux" ]]; then
  echo "==> Dispatching to Linux bundler..."
  exec "$SCRIPT_DIR/bundle-libmpv-linux.sh"
fi

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Skipping libmpv bundle (unsupported platform: $(uname -s))"
  mkdir -p "$LIBS_BUNDLE"
  exit 0
fi
LIBS_DIR="$WORKSPACE_ROOT/libs"
LIBS_MACOS="$LIBS_DIR/macos"
TARGET_RELEASE="$WORKSPACE_ROOT/target/release"
APP_NAME="max-video-player"

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

# Resolve Homebrew prefix for transitive dep search (works on Apple Silicon + Intel)
BREW_LIB="$(brew --prefix 2>/dev/null || echo /opt/homebrew)/lib"

# -s tells dylibbundler where to find libraries it can't resolve via @rpath.
# Without this it drops into an interactive prompt (and hangs in CI).
dylibbundler -od -b -x "$BINARY" -d "$LIBS_BUNDLE" -p "@executable_path/../Frameworks" \
  -s "$LIBS_MACOS" \
  -s "$BREW_LIB"

# dylibbundler rewrites every LC_RPATH it finds to the -p path, so libmpv (built
# with one rpath per Homebrew dependency) ends up carrying the same entry ~11
# times. dyld refuses to load an image with duplicate LC_RPATHs once it is linked
# against the macOS 26 SDK, which crashed v0.5.0 at launch. Keep the first copy.
rpaths_of() {
  otool -l "$1" | awk '$1 == "cmd" && $2 == "LC_RPATH" {
    getline; getline
    sub(/^ *path /, ""); sub(/ \(offset [0-9]+\)$/, "")
    print
  }'
}

dedupe_rpaths() {
  local file="$1" rpath count changed=0
  while IFS= read -r rpath; do
    count=$(rpaths_of "$file" | grep -cxF -- "$rpath")
    # install_name_tool -delete_rpath removes one matching entry per call.
    while (( count > 1 )); do
      install_name_tool -delete_rpath "$rpath" "$file"
      count=$((count - 1))
      changed=1
    done
  done < <(rpaths_of "$file" | sort | uniq -d)
  if (( changed )); then
    # Editing load commands invalidates the signature; arm64 won't load unsigned code.
    codesign --force --preserve-metadata=entitlements,requirements,flags,runtime --sign - "$file"
    echo "Removed duplicate LC_RPATH entries from $(basename "$file")"
  fi
}

for f in "$BINARY" "$LIBS_BUNDLE"/*.dylib; do
  dedupe_rpaths "$f"
  if [[ -n "$(rpaths_of "$f" | sort | uniq -d)" ]]; then
    echo "Error: $(basename "$f") still has duplicate LC_RPATH entries" >&2
    exit 1
  fi
done

echo "Bundled libmpv and dependencies to $LIBS_BUNDLE"
