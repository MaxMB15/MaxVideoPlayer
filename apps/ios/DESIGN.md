# iOS / iPadOS Support — Design Spec

**Date:** 2026-04-18
**Status:** Pending Approval
**Platforms:** iOS 17+ (iPhone + iPad, including Apple silicon iPad)

> **Update (October 2026):** still pending approval. The POC in [`poc/`](poc/)
> moved the libmpv path to [MPVKit](https://github.com/mpvkit/MPVKit): mpv draws
> straight into a `CAMetalLayer` (`vo=gpu-next`, Vulkan via MoltenVK,
> `hwdec=videotoolbox`) instead of the OpenGL ES / software render route in
> section 2. Section 2 needs updating to match before implementation starts.

---

## Overview

Port MaxVideoPlayer to iOS and iPadOS while preserving the existing Tauri + Rust + React architecture. The core bet: **Tauri Mobile (v2) + a dual-engine player** where AVPlayer handles App‑Store‑friendly formats (HLS/MP4) and libmpv handles everything else (RTMP/RTSP/raw‑TS/UDP/MKV) on sideloaded builds. React components are reused verbatim for iPad landscape, lightly re-composed for iPad portrait, and a subset are rebuilt as a purpose-built iPhone shell.

Distribution rolls out in phases: **sideload (AltStore) first**, then the **Apple Developer Program**, then optionally an **App Store build** that compiles the MPV engine out via Cargo feature.

---

## Key Decisions

| Decision                 | Choice                                                                                                                                                   | Rationale                                                                                                                                    |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Mobile framework         | Tauri v2 Mobile                                                                                                                                          | Preserves Rust core + React UI; already wired (`cfg_attr(mobile, tauri::mobile_entry_point)`) and `staticlib`/`cdylib` crate types in place. |
| Player engine            | Dual: AVPlayer default, libmpv fallback via `PlayerEngine` trait + URL router                                                                            | AVPlayer covers ~80% of modern IPTV for free; libmpv fills protocol gaps. Single abstraction keeps call sites uniform.                       |
| App Store build          | Cargo feature `mpv-engine`, default‑off on `ios-appstore` profile                                                                                        | Strips libmpv entirely — avoids GPL / binary‑size / license review risk.                                                                     |
| Distribution order       | Sideload → Developer Program → App Store                                                                                                                 | Ship fast with full format support; App Store gets the AVPlayer‑only subset later.                                                           |
| Device scope             | iPhone + iPad, iOS 17+                                                                                                                                   | iOS 17 unlocks modern SwiftUI/AVKit APIs and aligns with current Tauri mobile toolchain. No legacy device burden.                            |
| UI composition           | Three-tier responsive: Desktop ≈ iPad‑Landscape (same code), iPad‑Portrait (tweaked composition), iPhone (rebuilt shell)                                 | Tauri gives us ~95% component reuse on iPad; iPhone needs native patterns (tab bar, sheets).                                                 |
| iPhone EPG               | Hybrid A+C — "Live" tab as home (channel cards w/ now-playing + ⓘ), drawer opens per-channel vertical schedule. No standalone EPG tab, no mini timeline. | Grid is unreadable at 390 px; per-channel drawer is the iOS‑native pattern.                                                                  |
| Rust core reuse          | 100% — mvp-core compiles clean for `aarch64-apple-ios`                                                                                                   | All deps (`reqwest`, `rusqlite` bundled, `quick-xml`, `chrono`, `rayon`, `memmap2`) are iOS‑compatible.                                      |
| Transcoding/remux bridge | Deferred to Phase 2, App Store track only                                                                                                                | MPV handles odd protocols natively on sideload builds; remux only earns its keep when AVPlayer is the only option.                           |

---

## 1. Player Engine Architecture

The single largest architectural change. Today `crates/tauri-plugin-mpv` is mpv‑only. On iOS we need two engines behind one interface.

### `PlayerEngine` trait (new, in `crates/tauri-plugin-player`)

```rust
pub trait PlayerEngine: Send + Sync {
    fn load(&self, url: &str, opts: LoadOptions) -> Result<()>;
    fn play(&self) -> Result<()>;
    fn pause(&self) -> Result<()>;
    fn stop(&self) -> Result<()>;
    fn seek(&self, secs: f64) -> Result<()>;
    fn set_volume(&self, v: f32) -> Result<()>;
    fn set_subtitle_track(&self, id: Option<i64>) -> Result<()>;
    fn state(&self) -> PlayerState;
    fn capabilities(&self) -> Capabilities;
}
```

Two implementations:

- **`AvPlayerEngine`** (iOS‑only, always compiled) — wraps `AVPlayer` + `AVPlayerLayer` via Tauri‑Mobile Swift plugin bridge. Provides PiP, AirPlay, CarPlay, background audio, Control Center integration for free.
- **`MpvEngine`** (existing, gated by `#[cfg(feature = "mpv-engine")]`) — unchanged for macOS/Linux, compiled for iOS sideload via a new `ios.rs` CAMetalLayer renderer.

### URL-based router

```rust
pub fn select_engine(url: &str) -> EngineKind {
    let scheme = url.split("://").next().unwrap_or("").to_lowercase();
    match scheme.as_str() {
        "rtmp" | "rtmps" | "rtsp" | "udp" | "mms" => EngineKind::Mpv,
        _ if url.ends_with(".ts") || url.ends_with(".mkv") || url.ends_with(".avi")
            => EngineKind::Mpv,
        _ => EngineKind::Av, // HLS .m3u8, MP4, default
    }
}
```

App Store build (`--no-default-features --features ios-appstore`): the match arms that return `Mpv` become `Err(UnsupportedProtocol)` — surfaced to the UI as "This format requires the sideload build."

### Rust crate reorg

- Rename `tauri-plugin-mpv` → **`tauri-plugin-player`**, keep `MpvEngine` inside feature‑gated module.
- New modules: `engine_av.rs`, `engine_mpv.rs`, `router.rs`, `capabilities.rs`.
- Renderer trait stays (`PlatformRenderer`), with iOS variants: `IosAvPlayerLayer` and (feature‑gated) `IosMpvMetalRenderer`.

---

## 2. iOS Video Rendering

### AVPlayer path (default)

- Swift side: `AVPlayer` → `AVPlayerLayer` added to a `UIView` that Tauri Mobile creates below the WKWebView (same "native layer under webview" pattern we already use on macOS with `NSOpenGLView`).
- WebView background set transparent so the layer shows through in a rect controlled by JS via `set_frame()` IPC.
- PiP via `AVPictureInPictureController` — one-line enable.
- AirPlay via `MPNowPlayingInfoCenter` + `AVRoutePickerView` surfaced in the player UI.
- Background audio via `AVAudioSession.sharedInstance().setCategory(.playback)` + `UIBackgroundModes: [audio]` in Info.plist.

### libmpv path (sideload only)

- Cross‑compile `libmpv.a` + LGPL ffmpeg for `aarch64-apple-ios` using `scripts/build-libmpv-ios.sh` (new) — mirrors existing macOS script but with iOS SDK sysroot and `--disable-debug --disable-programs`.
- `ios.rs` creates a `CAMetalLayer` subview, hands it to libmpv via `mpv_render_context` with `MPV_RENDER_API_TYPE_OPENGL` (OpenGL ES 3.0 on iOS — supported but deprecated) OR pivots to libmpv's `SW` render API + Metal upload if GLES proves unstable.
- Hardware decode: `hwdec=videotoolbox-copy` (works with render API; `videotoolbox` direct path requires private APIs).
- NO PiP / AirPlay for the MPV path in v1 — documented limitation. Users who need PiP switch to HLS URL.

### Why not libmpv via Metal directly

libmpv exposes `MPV_RENDER_API_TYPE_OPENGL` only. Newer forks have a Metal API proposal but nothing merged. OpenGL ES still works on iOS 17, just with deprecation warnings. If Apple removes it we pivot to the SW render API (libmpv draws into a CPU buffer, we upload to `MTLTexture` per frame — ~20% overhead, acceptable).

---

## 3. UI Composition — Three Tiers

### Tier 1: Desktop + iPad Landscape — same React tree

- 3‑column layout: sidebar (providers/categories) · channel list · EPG/Now‑Playing drawer.
- Touch adaptations globally: 44pt min hit targets (already respected by Tailwind sizes), long‑press substitutes for right‑click in channel list context menu.
- Works on iPad 11"/13" landscape without modification.

### Tier 2: iPad Portrait — tweaked composition

- Sidebar collapses to icon rail (40pt wide) — reuses existing `<Sidebar>` with a `variant="compact"` prop.
- Right drawer (EPG / Now Playing) becomes a bottom/side sheet via `<Sheet>` component (add — shadcn primitive).
- Channel list + player take the main column.
- Estimated: ~1 week of CSS breakpoint + one sheet component.

### Tier 3: iPhone — rebuilt shell

- **Bottom tab bar** replaces sidebar. 5 tabs: Home · Live · VoD · Favorites · Settings.
- **Home / Live = channel cards**: logo + channel name + currently‑airing program + ⓘ button.
- **ⓘ opens per‑channel EPG drawer** (vertical schedule, today/tomorrow sections — this is the "A" half of hybrid A+C).
- **No EPG tab** on iPhone. Cross‑channel grid is iPad‑only.
- **Player**: full‑screen with translucent overlay controls; swipe‑down to minimize to PiP (AVPlayer only).
- **Drawers become iOS sheets** (presentation: `formSheet` / `pageSheet`).
- Scope: ~40% new view code, 100% hook/state/IPC reuse. Estimated ~2 weeks.

React side uses `usePlatform()` (already exists, already detects iOS/Android + derives `layoutMode`) to select between `<DesktopShell>`, `<TabletPortraitShell>`, `<PhoneShell>`.

---

## 4. Feature Scope — Phased

### Phase 1 — MVP (sideload ship)

Xtream Codes login · M3U URL import · Live TV · VoD/Series · Search + categories · Favorites (SQLite) · Recently played · Provider CRUD · Basic subtitle toggle · PiP (AVPlayer path) · Background audio.

### Phase 2 — after stable

M3U file import (UIDocumentPicker) · EPG / XMLTV timeline · OMDB / MDBList / Whatson drawers · OpenSubtitles download + overlay for AVPlayer · Subtitle styling / delay / pos (MPV only) · Group hierarchy mgmt · AirPlay (free on AV, work on MPV) · Watch‑history sync via CloudKit · iPad keyboard / trackpad shortcuts · External display / Stage Manager · Remux bridge (rtmp/ts/udp/mkv → HLS) **only for App Store build**.

### Skipped / deferred

Donation popup (App Store IAP rules) · Updater plugin (App Store handles it; sideload keeps Tauri updater) · Install‑info / package manager · Fallback player window (no separate windows on iOS) · ASS/SSA styled subs on AVPlayer (no libass path) · Multi‑window in‑app (Stage Manager only).

**Gemini categorization on mobile** — _open question._ Leaning "keep user‑key flow for power users, gate behind a settings toggle; no hosted endpoint in v1."

---

## 5. Distribution Strategy

### Phase A — Sideload via AltStore / SideStore

- Build: `cargo tauri ios build --target aarch64-apple-ios` + xcodebuild sign with personal team cert.
- Includes MPV engine (`--features mpv-engine`).
- Distributed as `.ipa` on GitHub Releases alongside desktop artifacts.
- 7‑day re‑sign cadence for free Apple IDs; AltStore handles refresh.

### Phase B — Apple Developer Program ($99/yr)

- Same binary as Phase A, signed with paid cert → 1‑year re‑sign cadence.
- Enables TestFlight for beta testers (up to 10k users).

### Phase C — App Store

- Separate target: `cargo build --no-default-features --features ios-appstore` — strips libmpv entirely.
- AVPlayer‑only feature set. Protocol selector in settings surfaces "For RTMP/RTSP/TS/UDP/MKV, use the sideload build."
- Remux bridge (Phase 2 feature) lands here to recover .ts / rtmp support for App Store users without GPL code.
- LGPL ffmpeg build only; no libmpv, no mpv Lua scripts, no mpv‑specific subs.

CI: GitHub Actions runner `macos-14` (Apple silicon) builds both targets on tag push; draft release attaches both `.ipa` variants.

---

## 6. File‑Level Changes (preview)

- **New:** `crates/tauri-plugin-player/` (renamed from `tauri-plugin-mpv`) with `engine.rs` (trait), `engine_av.rs`, `engine_mpv.rs`, `router.rs`, `ios.rs` (Metal + CAMetalLayer), `ios_av.swift` (iOS plugin bridge).
- **New:** `scripts/build-libmpv-ios.sh` — cross‑compile for `aarch64-apple-ios`.
- **New:** `apps/desktop/src-tauri/gen/apple/` — Tauri mobile Xcode project (auto‑generated by `tauri ios init`).
- **New:** `apps/desktop/src/layouts/PhoneShell.tsx`, `TabletPortraitShell.tsx`, `DesktopShell.tsx` (wraps existing `App.tsx` body).
- **New:** `apps/desktop/src/components/channels/ChannelCard.tsx` (iPhone Live tab).
- **New:** `apps/desktop/src/components/epg/EpgChannelDrawer.tsx` (A‑half of hybrid).
- **Modified:** `apps/desktop/src-tauri/tauri.conf.json` — add `bundle.iOS` block, `identifier`, `minimumSystemVersion: "17.0"`, iOS build config.
- **Modified:** `apps/desktop/src-tauri/Cargo.toml` — add `tauri-plugin-player` dep with `mpv-engine` feature, iOS target deps.
- **Modified:** `apps/desktop/src/App.tsx` — platform switch via `usePlatform().layoutMode`.
- **Modified:** `apps/desktop/src/lib/tauri.ts` — rename MPV namespace calls to `plugin:player|*`, keep same shapes.

---

## 7. Risks & Open Questions

1. **libmpv Metal integration is novel territory.** OpenGL ES path works today but is deprecated. Mitigation: SW‑render fallback documented; pivot is ~3 days of work.
2. **WKWebView transparency under AVPlayerLayer** — same trick as macOS NSView; should work, but needs confirmation on first `tauri ios dev` run.
3. **App Store review risk for sideload‑adjacent language** — README for App Store build must not mention sideloading / MPV / RTMP as _App Store_ features.
4. **Binary size** — MPV sideload build estimated 35–50 MB `.ipa`. Under App Store 200 MB cellular threshold (App Store build is far smaller — no MPV).
5. **Tauri Mobile maturity** — v2 mobile is stable but less battle‑tested than desktop. Risk: hot reload quirks, plugin bridge edge cases. Mitigation: lots of `#[cfg(debug_assertions)]` logging in the iOS plugin.
6. **Open: Gemini categorization on mobile** — user-key / hosted / drop. Leaning user‑key + toggle.
7. **Open: iCloud sync for Favorites/History** — deferred to Phase 2; CloudKit vs custom sync endpoint decision not made.

---

## 8. Success Criteria

- `cargo tauri ios dev` launches on iPhone 15 simulator, loads an Xtream provider, plays an HLS channel via AVPlayer with < 2s start‑to‑first‑frame.
- Same build plays an RTMP stream via MPV engine on a physical device.
- iPad landscape: visually indistinguishable from desktop at 1366×1024.
- iPhone: bottom tab bar + Live tab renders 500+ channel cards at 60fps (virtualized list).
- App Store build target compiles clean with `--no-default-features --features ios-appstore` and `cargo deny check` passes (no GPL deps).

---

## Non‑Goals (explicitly)

- Android port — separate future spec.
- tvOS / Fire Stick — different UI paradigm (focus engine), out of scope here.
- macOS Catalyst — not worth the tradeoffs given we already have a native macOS build.
- Custom video filters / shaders on iOS — mpv scripts banned on App Store, and AVPlayer doesn't expose shader hooks.
- Rewriting anything in Swift beyond the thin AVPlayer bridge. React + Rust core stay authoritative.
