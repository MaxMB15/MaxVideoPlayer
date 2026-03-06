# MaxVideoPlayer

Cross-platform IPTV video player with **Rust** core logic. Per-platform apps share `mvp-core` (M3U, Xtream, EPG, cache) and use platform-native video players for efficiency.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│  crates/mvp-core (shared Rust)                                           │
│  • M3U parsing • Xtream Codes API • EPG • SQLite cache                   │
└─────────────────────────────────────────────────────────────────────────┘
         │                      │                      │
         ▼                      ▼                      ▼
┌─────────────────┐   ┌─────────────────┐   ┌─────────────────┐
│  apps/desktop   │   │  apps/ios       │   │  apps/android   │
│  Tauri + React  │   │  Swift +        │   │  Kotlin +       │
│  + libmpv       │   │  AVPlayer       │   │  ExoPlayer      │
│  (macOS/Win/Linux)│   │  (planned)     │   │  (planned)      │
└─────────────────┘   └─────────────────┘   └─────────────────┘
```

## Project Structure

```
MaxVideoPlayer/
├── crates/
│   ├── core/                  # mvp-core — shared Rust (IPTV, cache, EPG)
│   └── tauri-plugin-mpv/      # libmpv plugin for desktop app
├── apps/
│   ├── desktop/               # Tauri + React + libmpv
│   │   ├── src-tauri/
│   │   └── src/               # React frontend
│   ├── ios/                   # Swift + AVPlayer (planned)
│   └── android/               # Kotlin + ExoPlayer (planned)
├── libs/                      # libmpv per platform (desktop: macos/linux/windows, ios: ios/, android: android/)
├── scripts/
│   ├── build-libmpv.sh        # Copy libmpv for dev/build
│   └── bundle-libmpv.sh       # Bundle libmpv into .app (macOS release)
└── package.json               # Workspace scripts
```

## Supported Platforms

| Platform | Status | Stack |
|----------|--------|-------|
| macOS | ✅ Primary | Tauri, React, libmpv |
| Windows | Planned | Tauri, React, libmpv |
| Linux | Planned | Tauri, React, libmpv |
| iOS | Planned | Swift, AVPlayer, mvp-core (UniFFI) |
| Android / Fire Stick | Planned | Kotlin, ExoPlayer, mvp-core (JNI) |

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) >= 20
- **libmpv** (macOS dev): `brew install mpv`
- **dylibbundler** (macOS release build): `brew install dylibbundler`

## Development

```bash
npm install
npm run dev:desktop      # Desktop app (Tauri + React)
npm run dev:desktop:web  # Web only (Vite, no native window)
npm run test             # All tests (core + desktop)
```

## Testing

```bash
npm run test             # All tests
npm run test:core        # mvp-core (Rust) only
npm run test:desktop     # Desktop frontend (Vitest) only
```

## Desktop Build

**Development** (requires `brew install mpv`):

```bash
./scripts/build-libmpv.sh macos   # Copy libmpv to libs/macos/
npm run build:desktop
cd apps/desktop && cargo tauri build
```

**Release** (macOS): libmpv and dependencies are bundled into the .app automatically. Ensure `brew install mpv dylibbundler` before building.

## Tech Stack

| Layer | Technology |
|-------|------------|
| Core | Rust (mvp-core) |
| Desktop UI | Tauri v2, React, Tailwind |
| Desktop Video | libmpv |
| iOS (planned) | Swift, AVPlayer |
| Android (planned) | Kotlin, ExoPlayer |
| Database | SQLite (rusqlite) |
| EPG | quick-xml |
