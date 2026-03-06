# MaxVideoPlayer — iOS

Native iOS app using Swift/SwiftUI with AVPlayer for video playback.

## Planned Architecture

- **UI**: SwiftUI
- **Video**: AVPlayer (native HLS, MPEG-TS support)
- **Core logic**: `mvp-core` (Rust) via UniFFI / XCFramework
  - M3U parsing, Xtream Codes API, EPG, SQLite cache

## Setup (Future)

1. Build `mvp-core` for `aarch64-apple-ios` (static lib or XCFramework)
2. Generate Swift bindings via UniFFI
3. Implement SwiftUI app that calls into mvp-core for playlist/EPG, uses AVPlayer for playback

## Dependencies

- Xcode 15+
- Rust target: `aarch64-apple-ios`
- UniFFI (for Swift bindings)
