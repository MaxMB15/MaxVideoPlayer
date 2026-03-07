# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

All frontend commands run from `apps/desktop/`:

```bash
# Dev server (frontend only)
npm run dev

# Run Tauri app (dev mode with hot reload)
cd apps/desktop && npx tauri dev

# Build for production
cd apps/desktop && npx tauri build

# Run tests
cd apps/desktop && npm test

# Run tests in watch mode
cd apps/desktop && npm run test:watch
```

Rust workspace commands (run from repo root):

```bash
cargo build
cargo test
cargo check
```

### macOS Setup (required before first build)

Homebrew's `mpv` is Vulkan-only and cannot provide the OpenGL render context required for embedded playback. **Build libmpv from source** using the provided script:

```bash
# Install build dependencies
brew install meson ninja pkg-config ffmpeg libass dylibbundler

# Build libmpv from source (clones mpv repo, builds with -Dgl=enabled)
./scripts/build-libmpv.sh macos
```

This clones the mpv source into `libs/mpv-src/` and outputs `libs/macos/libmpv.dylib`. The build takes a few minutes on first run; subsequent runs skip the clone.

For development, set `DYLD_LIBRARY_PATH` so the binary can find the dylib:
```bash
export DYLD_LIBRARY_PATH="$(pwd)/libs/macos:$DYLD_LIBRARY_PATH"
cd apps/desktop && npx tauri dev
```

The `bundle-libmpv.sh` script runs automatically as `beforeBundleCommand` during `tauri build` to bundle dylibs into the `.app` using `dylibbundler`.

## Architecture

This is a **Tauri v2** IPTV player with a React frontend and Rust backend.

### Project Goals & Platform Targets

The goal is a cross-platform IPTV player targeting **macOS, Windows, iOS/iPadOS, and Android** (including Fire Stick). macOS is the current development focus.

**Why MPV:** MPV is used as the video rendering engine because it supports virtually all IPTV protocols (HLS, RTMP, RTSP, TS, etc.), is lightweight, and is highly performant. The custom `tauri-plugin-mpv` crate wraps `libmpv2` to embed MPV within the app.

### Workspace Structure

```
MaxVideoPlayer/
├── Cargo.toml                    # Rust workspace root
├── apps/
│   └── desktop/
│       ├── src/                  # React frontend (TypeScript)
│       └── src-tauri/            # Tauri Rust app (main.rs calls max_video_player_lib::run())
├── crates/
│   ├── core/                     # mvp-core: pure Rust business logic
│   └── tauri-plugin-mpv/         # Custom Tauri plugin wrapping libmpv2
└── scripts/                      # macOS libmpv build/bundle scripts
```

### Frontend (React + TypeScript)

- **Entry:** `apps/desktop/src/main.tsx` -> `App.tsx`
- **Routing:** `react-router-dom` with routes: `/` (channels), `/player`, `/guide`, `/playlists`, `/settings`
- **State:** `ChannelsContext` (`useChannels.ts`) is the global provider for all channel/provider state, initialized at the top of the app and consumed via `useChannels()` hook throughout
- **Tauri bridge:** `src/lib/tauri.ts` is the single file containing all `invoke()` calls to the Rust backend. MPV plugin commands use the `plugin:mpv|<command>` namespace; core commands use bare names
- **Player state:** `useMpv.ts` polls the Rust MPV state every 1 second via `mpvGetState()`
- **UI components:** shadcn-style components in `src/components/ui/`, feature components organized by domain (`channels/`, `player/`, `epg/`, `playlist/`, `settings/`)
- **Styling:** Tailwind CSS v3 + `tailwind-merge` + `class-variance-authority`
- **Virtual list:** `@tanstack/react-virtual` used in channel list for performance

### Backend (Rust)

**`crates/core` (mvp-core):** Platform-agnostic business logic
- `iptv/m3u.rs` - M3U playlist parsing
- `iptv/xtream.rs` - Xtream Codes API client
- `iptv/epg.rs` - EPG/XMLTV parsing
- `cache/store.rs` - SQLite cache via `rusqlite` (bundled)
- `models/` - `Channel`, `Playlist` data models

**`crates/tauri-plugin-mpv`:** Custom Tauri plugin for video playback
- `engine.rs` - `MpvEngine` wraps `libmpv2` crate; platform-agnostic; handles create/load/play/pause/stop/seek/volume
- `renderer.rs` - `PlatformRenderer` trait (`attach`, `resize`, `detach`); all `#[cfg]` lives in per-platform files
- `mpv.rs` - `MpvState` (Tauri managed state): tries embedded renderer, emits `mpv://render-fallback` event and falls back to separate window on failure
- `macos.rs` - `MacosGlRenderer`: NSOpenGLView + OpenGL Core 3.2 render context; all AppKit calls dispatched to main thread
- `commands.rs` - Tauri command handlers registered under `plugin:mpv|*`

**`apps/desktop/src-tauri`:** Main Tauri application entry point
- Integrates `mvp-core` and `tauri-plugin-mpv`
- Exposes core IPTV commands directly (e.g., `load_m3u_playlist`, `get_providers`, `toggle_favorite`)

### macOS Video Rendering Strategy

`MpvState::load()` tries embedded rendering first, then falls back:
1. **Embedded (primary):** `MacosGlRenderer` creates an `NSOpenGLView` (Core 3.2 profile) as a subview below WKWebView; `mpv_render_context_create` with `MPV_RENDER_API_TYPE_OPENGL`; frames rendered via `[glContext flushBuffer]`
2. **Fallback:** If embed fails, emits `mpv://render-fallback` Tauri event to frontend, launches `vo=gpu` (Vulkan/MoltenVK) — opens a separate mpv window with native OSC controls visible

All AppKit/OpenGL calls (`NSOpenGLView`, `NSOpenGLContext`, frame rendering) **must** run on the main thread via `dispatch::Queue::main().exec_sync/exec_async()`. Raw pointers are cast to `usize` before crossing dispatch boundaries to satisfy Rust's `Send` requirement.

### Data Flow

Frontend -> `src/lib/tauri.ts` -> `invoke()` -> Tauri command -> Rust handler -> `mvp-core` or `MpvState`
