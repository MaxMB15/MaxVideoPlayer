# Max Video Player — iOS

iOS / iPadOS support is planned as a **Tauri mobile** build of the desktop app,
sharing the React frontend and the Rust crates. The full plan, covering the dual
playback engines (AVPlayer and libmpv), the UI tiers and distribution, is in
[`DESIGN.md`](DESIGN.md). It is still pending approval.

## Proof of concept (`poc/`)

`poc/` is a standalone Tauri 2 app (`com.maxvideoplayer.iospoc`, iOS 17+) that
was built to de-risk the two playback paths before the real port:

- **AVPlayer**: `IosVideoBridge.swift`, for HLS/MP4 streams iOS plays natively.
- **libmpv**: `IosMpvBridge.m`, using [MPVKit](https://github.com/mpvkit/MPVKit)
  through Swift Package Manager. mpv renders into a `CAMetalLayer`
  (`vo=gpu-next`, Vulkan via MoltenVK, `hwdec=videotoolbox`) inside a modal
  with native controls.

The React UI in `poc/src/App.tsx` takes a URL and calls the Rust commands
`play_url` / `play_url_mpv` (`poc/src-tauri/src/lib.rs`), which call into the
native bridges.

Status: the app builds and exports a signed `.ipa`. Playback on a physical
device has not been confirmed yet.

### Building

Requirements:

- macOS with Xcode 15+ and [XcodeGen](https://github.com/yonaskolb/XcodeGen)
  (`brew install xcodegen`)
- Rust iOS targets: `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`
- Tauri CLI as a cargo subcommand: `cargo install tauri-cli` (the Xcode
  pre-build script runs `cargo tauri ios xcode-script`)

```bash
cd apps/ios/poc
npm install

# Simulator or connected device, with hot reload
npm run tauri ios dev

# Release build / .ipa
npm run tauri ios build
```

You can also open `src-tauri/gen/apple/mvp-ios-poc.xcodeproj` in Xcode. MPVKit is
fetched by SwiftPM on first build.

Signing uses `DEVELOPMENT_TEAM` in `src-tauri/gen/apple/project.yml`. Change it
to your own team ID, then run `xcodegen` in that directory to regenerate the
Xcode project.

### What is not in git

Everything below is build output and is recreated by the commands above:

| Path                                 | Recreated by                       |
| ------------------------------------ | ---------------------------------- |
| `node_modules/`, `dist/`             | `npm install`, `npm run build`     |
| `src-tauri/target/`                  | any cargo / Tauri build            |
| `src-tauri/gen/schemas/`             | any Tauri build                    |
| `src-tauri/gen/apple/Externals/`     | Xcode pre-build (built `libapp.a`) |
| `src-tauri/gen/apple/assets/`        | Tauri CLI (frontend bundle)        |
| `src-tauri/gen/apple/build/`         | `tauri ios build` (archive, .ipa)  |
| `src-tauri/gen/apple/**/xcuserdata/` | Xcode (per-user state)             |
| `libs/`                              | `scripts/build-all-ios.sh`         |

The POC is not part of the root Cargo workspace or npm workspace, so root
`cargo build` / `npm test` ignore it.

### `scripts/*-ios.sh`

These build FFmpeg, libass, libplacebo and libmpv for iOS by hand into `libs/`.
They were the first approach and are **not used by the current build**, which
gets prebuilt libraries from MPVKit. They are kept for reference, e.g. for a
future LGPL-only build for the App Store.

## Next steps

1. Confirm both playback paths on a physical device.
2. Approve or revise `DESIGN.md` (section 2 needs updating for MPVKit).
3. Run `tauri ios init` in `apps/desktop` and port the mpv bridge into an iOS
   module of `crates/tauri-plugin-mpv`.
