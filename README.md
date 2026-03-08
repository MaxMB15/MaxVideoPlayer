# MaxVideoPlayer

A cross-platform IPTV player built with **Tauri v2**, **React**, and **libmpv**. The Rust core (`mvp-core`) handles M3U/Xtream/EPG parsing and SQLite caching across all targets. A custom `tauri-plugin-mpv` embeds libmpv directly into the native window using `NSOpenGLView` on macOS, giving hardware-accelerated playback for virtually any IPTV protocol (HLS, RTMP, RTSP, TS, etc.).

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
brew install meson ninja pkg-config ffmpeg libass dylibbundler

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
