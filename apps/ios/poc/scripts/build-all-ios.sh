#!/usr/bin/env bash
set -euo pipefail

# Convenience runner: builds ffmpeg + libplacebo + libass (+freetype/fribidi/harfbuzz) + libmpv
# for iOS (aarch64-apple-ios) in dependency order. Each sub-script is idempotent
# and skips already-built libs, so re-running is cheap.
#
# Usage: ./scripts/build-all-ios.sh

HERE="$(cd "$(dirname "$0")" && pwd)"

"$HERE/build-ffmpeg-ios.sh"
"$HERE/build-libass-ios.sh"
"$HERE/build-libplacebo-ios.sh"
"$HERE/build-libmpv-ios.sh"

echo ""
echo "==> All iOS libs ready in libs/ios/"
