# MaxVideoPlayer — Android

Native Android app using Kotlin/Jetpack Compose with ExoPlayer for video playback.

## Planned Architecture

- **UI**: Jetpack Compose
- **Video**: ExoPlayer (HLS, MPEG-TS, DASH)
- **Core logic**: `mvp-core` (Rust) via JNI or UniFFI
    - M3U parsing, Xtream Codes API, EPG, SQLite cache

## Setup (Future)

1. Build `mvp-core` for Android (arm64-v8a, armeabi-v7a) via `cargo-ndk`
2. Create JNI bindings or UniFFI Kotlin bindings
3. Implement Compose app that calls into mvp-core for playlist/EPG, uses ExoPlayer for playback

## Dependencies

- Android Studio
- NDK 26+
- Rust targets: `aarch64-linux-android`, `armv7-linux-androideabi`
