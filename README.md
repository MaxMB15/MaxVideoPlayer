# MaxVideoPlayer

[![Build & Bundle](https://github.com/MaxMB15/MaxVideoPlayer/actions/workflows/build.yml/badge.svg)](https://github.com/MaxMB15/MaxVideoPlayer/actions/workflows/build.yml)
[![Tests](https://img.shields.io/github/actions/workflow/status/MaxMB15/MaxVideoPlayer/build.yml?branch=main&label=tests&job=test)](https://github.com/MaxMB15/MaxVideoPlayer/actions/workflows/build.yml)
[![Release](https://img.shields.io/github/v/release/MaxMB15/MaxVideoPlayer?include_prereleases&label=release)](https://github.com/MaxMB15/MaxVideoPlayer/releases/latest)
[![License: PolyForm Noncommercial](https://img.shields.io/badge/License-PolyForm%20Noncommercial-blue.svg)](LICENSE)

A cross-platform IPTV player built with **Tauri v2**, **React**, and **libmpv**. The Rust core (`mvp-core`) handles M3U/Xtream/EPG parsing and SQLite caching across all targets. A custom `tauri-plugin-mpv` embeds libmpv directly into the native window — using `NSOpenGLView` on macOS and EGL + X11/Wayland subsurfaces on Linux — for hardware-accelerated playback of virtually any IPTV protocol (HLS, RTMP, RTSP, TS, etc.).

See [LICENSE](LICENSE) for terms, [NOTICE](NOTICE) for trademark and legal disclaimers.

## Platform Support

| Platform | Status | Video | Packages |
|----------|--------|-------|----------|
| macOS | Active | libmpv embedded (NSOpenGLView + OpenGL Core 3.2) | `.dmg` |
| Linux | Active | libmpv embedded (EGL + X11 child window / Wayland subsurface) | `.deb`, `.rpm`, `.AppImage` |
| Windows | Planned | libmpv | |
| iOS / iPadOS | Planned | AVPlayer + mvp-core via UniFFI | |
| Android / Fire Stick | Planned | ExoPlayer + mvp-core via JNI | |

## Architecture

```
MaxVideoPlayer/
├── crates/
│   ├── core/                  # mvp-core — M3U, Xtream Codes, EPG, SQLite cache
│   └── tauri-plugin-mpv/      # Custom Tauri plugin wrapping libmpv2
│       ├── src/engine.rs      # MpvEngine — libmpv lifecycle
│       ├── src/renderer.rs    # PlatformRenderer trait
│       ├── src/macos.rs       # NSOpenGLView + OpenGL Core 3.2 (macOS)
│       ├── src/linux.rs       # EGL + X11/Wayland (Linux)
│       ├── src/mpv.rs         # MpvState — Tauri managed state
│       └── src/commands.rs    # Tauri command handlers
├── apps/
│   └── desktop/
│       ├── src-tauri/         # Tauri app entry point
│       └── src/               # React frontend (TypeScript)
│           ├── components/    # UI components by domain
│           ├── hooks/         # useMpv, useChannels, usePlatform
│           └── lib/tauri.ts   # All invoke() calls in one place
├── libs/                      # libmpv binaries (gitignored, built by script)
└── scripts/
    ├── build-libmpv.sh            # Build libmpv from source (macOS/Linux)
    ├── bundle-libmpv.sh           # Platform dispatch for bundling at release
    └── bundle-libmpv-linux.sh     # Bundle .so deps for Linux AppImage
```

## macOS Setup

Homebrew's `mpv` formula is Vulkan-only. The embedded renderer requires OpenGL, so libmpv must be built from source:

```bash
# Install build dependencies
# Note: ffmpeg@7 required — mpv 0.40.0 uses APIs removed in ffmpeg 8.x
brew install meson ninja pkg-config ffmpeg@7 libass dylibbundler
export PKG_CONFIG_PATH="$(brew --prefix ffmpeg@7)/lib/pkgconfig:$PKG_CONFIG_PATH"

# Build libmpv from source (~3 min first run)
./scripts/build-libmpv.sh macos
```

This clones the mpv source into `libs/mpv-src/` and outputs `libs/macos/libmpv.dylib`. Subsequent runs skip the clone.

## Linux (Ubuntu) Setup

Install system dependencies:

```bash
sudo apt-get update
sudo apt-get install -y \
  libmpv-dev libegl-dev \
  libgtk-3-dev libwebkit2gtk-4.1-dev \
  libjavascriptcoregtk-4.1-dev libsoup-3.0-dev \
  libayatana-appindicator3-dev \
  libssl-dev pkg-config librsvg2-dev \
  patchelf
```

The system `libmpv-dev` package is used for development. To build libmpv from source instead:

```bash
sudo apt-get install meson ninja-build \
  libavcodec-dev libavformat-dev libavutil-dev libswscale-dev libavfilter-dev \
  libass-dev libdrm-dev

./scripts/build-libmpv.sh linux
export LD_LIBRARY_PATH="$(pwd)/libs/linux:$LD_LIBRARY_PATH"
```

## Development

```bash
npm install

# macOS
export DYLD_LIBRARY_PATH="$(pwd)/libs/macos:$DYLD_LIBRARY_PATH"
cd apps/desktop && npx tauri dev

# Linux (system libmpv-dev)
cd apps/desktop && npx tauri dev

# Linux (source-built libmpv)
export LD_LIBRARY_PATH="$(pwd)/libs/linux:$LD_LIBRARY_PATH"
cd apps/desktop && npx tauri dev
```

## Testing

```bash
cargo test -p mvp-core        # Rust core tests
cd apps/desktop && npm test   # Frontend tests (Vitest)
```

## Production Build

```bash
cd apps/desktop && npx tauri build
```

On macOS, `bundle-libmpv.sh` runs automatically as `beforeBundleCommand` and uses `dylibbundler` to embed libmpv into the `.app`. On Linux, `bundle-libmpv-linux.sh` bundles `libmpv.so` and its dependencies for AppImage distribution using `ldd` + `patchelf`. For `.deb` and `.rpm`, libmpv is declared as a system dependency.

## Auto-Updates

MaxVideoPlayer uses [tauri-plugin-updater](https://github.com/tauri-apps/plugins-workspace/tree/v2/plugins/updater) to check for new releases on startup. When an update is found, a dismissible banner appears with a one-click install.

### Setting up signing (required for production)

1. Generate a signing keypair:
   ```bash
   cd apps/desktop && npx tauri signer generate -w ~/.tauri/maxvideoplayer.key
   ```
2. Copy the **public key** output into `apps/desktop/src-tauri/tauri.conf.json` -> `plugins.updater.pubkey`.
3. Add the **private key** and optional password as GitHub repository secrets:
   - `TAURI_SIGNING_PRIVATE_KEY` — contents of `~/.tauri/maxvideoplayer.key`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — password (leave empty if none)

### Releasing a new version

**Recommended (GitHub Actions):** [Actions → Release — bump version & tag](https://github.com/MaxMB15/MaxVideoPlayer/actions/workflows/release-bump.yml) → **Run workflow**.

1. Choose **Semver bump** ([semver.org](https://semver.org)):
   - **patch** — bug fixes (0.3.0 → 0.3.1)
   - **minor** — backward-compatible features (0.3.1 → 0.4.0)
   - **major** — breaking changes (0.4.0 → 1.0.0)
2. Optionally enable **Merge main into dev** to sync your git-flow `dev` branch after the tag.
3. The workflow updates `tauri.conf.json`, `apps/desktop/src-tauri/Cargo.toml`, Settings About text, and `Cargo.lock`, commits to `main`, pushes tag **`vX.Y.Z`**, which starts **`release.yml`** (builds macOS + Linux, draft GitHub Release, `latest.json`).

**Who can run it:** By default only the **repository owner** login matches the allowlist. For **organization-owned** repos, set a repository **variable** `RELEASE_ALLOWED_ACTORS` to your GitHub username (comma-separated for multiple maintainers). **Protected `main`:** add a **secret** `RELEASE_AUTOMATION_PAT` — a [fine-grained PAT](https://github.com/settings/tokens?type=beta) with **Contents: Read and write** on this repo (the account must be allowed to push to `main`). If unset, the default `GITHUB_TOKEN` is used (may be blocked by branch protection).

**Optional:** Uncomment `environment: release` in `.github/workflows/release-bump.yml` and create a [GitHub Environment](https://docs.github.com/en/actions/deployment/targeting-different-environments/using-environments-for-deployment) named `release` with **required reviewers** so a second approval is required before the version bump runs.

**Local script (same semver rules):** `./scripts/release.sh` from `main` — uses `scripts/bump-version.py` and can merge `main` → `dev` interactively.

The `release.yml` workflow (on tag `v*`) builds signed artifacts for macOS (`.dmg`) and Linux (`.deb`, `.rpm`, `.AppImage`), creates a draft GitHub Release, and uploads `latest.json` for the auto-updater.

## Features

- **M3U / M3U+** playlist support (URL and local file)
- **Xtream Codes** provider support
- **EPG / XMLTV** programme guide
- **Favorites** with persistent SQLite storage
- **Hardware-accelerated** playback (VideoToolbox on macOS, VAAPI/NVDEC on Linux)
- Sidebar navigation with Channels, Player, Guide, Playlists, and Settings views
- Channel list with virtual scrolling (`@tanstack/react-virtual`) for large playlists
- Graceful fallback to a native mpv window if the embedded renderer fails

## Tech Stack

| Layer | Technology |
|-------|------------|
| Desktop shell | Tauri v2 |
| Frontend | React 18, TypeScript, Tailwind CSS v3 |
| UI components | shadcn-style (Radix UI primitives) |
| Video engine | libmpv2 (custom Tauri plugin) |
| Rust core | mvp-core (M3U, Xtream, EPG, SQLite) |
| Database | SQLite via rusqlite (bundled) |
| EPG parsing | quick-xml |
