# MaxVideoPlayer

[![Build & Bundle](https://github.com/MaxMB15/MaxVideoPlayer/actions/workflows/build.yml/badge.svg)](https://github.com/MaxMB15/MaxVideoPlayer/actions/workflows/build.yml)
[![Tests](https://img.shields.io/github/actions/workflow/status/MaxMB15/MaxVideoPlayer/build.yml?branch=main&label=tests&job=test)](https://github.com/MaxMB15/MaxVideoPlayer/actions/workflows/build.yml)
[![Release](https://img.shields.io/github/v/release/MaxMB15/MaxVideoPlayer?include_prereleases&label=release)](https://github.com/MaxMB15/MaxVideoPlayer/releases/latest)
[![License: PolyForm Noncommercial](https://img.shields.io/badge/License-PolyForm%20Noncommercial-blue.svg)](LICENSE)

A cross-platform IPTV player built with **Tauri v2**, **React**, and **libmpv**. The Rust core (`mvp-core`) handles M3U/Xtream/EPG parsing and SQLite caching across all targets. A custom `tauri-plugin-mpv` embeds libmpv directly into the native window using `NSOpenGLView` on macOS, giving hardware-accelerated playback for virtually any IPTV protocol (HLS, RTMP, RTSP, TS, etc.).

See [LICENSE](LICENSE) for terms, [NOTICE](NOTICE) for trademark and legal disclaimers.

## Platform Support

| Platform | Status | Video |
|----------|--------|-------|
| macOS | ✅ Active | libmpv embedded (NSOpenGLView + OpenGL Core 3.2) |
| Windows | Planned | libmpv |
| Linux | Planned | libmpv |
| iOS / iPadOS | Planned | AVPlayer + mvp-core via UniFFI |
| Android / Fire Stick | Planned | ExoPlayer + mvp-core via JNI |

## Architecture

```
MaxVideoPlayer/
├── crates/
│   ├── core/                  # mvp-core — M3U, Xtream Codes, EPG, SQLite cache
│   └── tauri-plugin-mpv/      # Custom Tauri plugin wrapping libmpv2
│       ├── src/engine.rs      # MpvEngine — libmpv lifecycle
│       ├── src/renderer.rs    # PlatformRenderer trait
│       ├── src/macos.rs       # NSOpenGLView + OpenGL Core 3.2 (macOS)
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
    ├── build-libmpv.sh        # Build libmpv from source
    └── bundle-libmpv.sh       # Bundle dylibs into .app at release
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

## Development

```bash
npm install

# Run the full Tauri app (hot reload)
export DYLD_LIBRARY_PATH="$(pwd)/libs/macos:$DYLD_LIBRARY_PATH"
cd apps/desktop && npx tauri dev
```

## Testing

```bash
cargo test -p mvp-core        # Rust core tests
cd apps/desktop && npm test   # Frontend tests (Vitest)
```

## Production Build

```bash
cd apps/desktop && cargo tauri build
```

`bundle-libmpv.sh` runs automatically as `beforeBundleCommand` and uses `dylibbundler` to embed libmpv and its dependencies into the `.app`.

## Auto-Updates

MaxVideoPlayer uses [tauri-plugin-updater](https://github.com/tauri-apps/plugins-workspace/tree/v2/plugins/updater) to check for new releases on startup. When an update is found, a dismissible banner appears with a one-click install.

### Setting up signing (required for production)

1. Generate a signing keypair:
   ```bash
   cd apps/desktop && npx tauri signer generate -w ~/.tauri/maxvideoplayer.key
   ```
2. Copy the **public key** output into `apps/desktop/src-tauri/tauri.conf.json` → `plugins.updater.pubkey`.
3. Add the **private key** and optional password as GitHub repository secrets:
   - `TAURI_SIGNING_PRIVATE_KEY` — contents of `~/.tauri/maxvideoplayer.key`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — password (leave empty if none)

### Releasing a new version

```bash
# Bump the version in tauri.conf.json and Cargo.toml, then:
git tag v0.2.0
git push origin v0.2.0
```

The `release.yml` workflow runs automatically, builds a signed `.dmg`, creates a draft GitHub Release, and uploads `latest.json` — which the updater endpoint points to.

## Features

- **M3U / M3U+** playlist support (URL and local file)
- **Xtream Codes** provider support
- **EPG / XMLTV** programme guide
- **Favorites** with persistent SQLite storage
- **Hardware-accelerated** playback via VideoToolbox on macOS
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
