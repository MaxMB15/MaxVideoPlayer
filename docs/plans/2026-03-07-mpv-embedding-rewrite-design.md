# MPV Embedding Rewrite — Design Document

**Date:** 2026-03-07
**Status:** Approved

## Problem

The current `tauri-plugin-mpv` always falls back to opening video in a separate mpv window instead of embedding inside the Tauri app window. Two root causes confirmed via logs:

1. **`vo=libmpv` render context fails with "Unsupported"** — Homebrew mpv on macOS is built Vulkan-only (no OpenGL GPU backend). The OpenGL render context (`mpv_render_context_create` with `MPV_RENDER_API_TYPE_OPENGL`) shares the same GPU infrastructure and therefore also returns Unsupported.
2. **`wid` fallback logs "success" but opens a new window** — on macOS, `vo=gpu` with `--wid` creates an mpv-owned NSWindow, not a true subview embed. It only appears to succeed.
3. **Secondary cause:** `transparent: true` in `tauri.conf.json` prevents `NSOpenGLView` from obtaining a CGL drawable (requires `macos-private-api` which is disabled). Transparency is not needed and will be removed.
4. **Load fires twice** — React-side bug causing `mpv_load` to be invoked twice per user action.

## Decisions

### libmpv Source
**Build from source** with `-Dgl=enabled`. Homebrew's libmpv cannot be used for the embedded render path. Building from source provides the OpenGL render context required by the `libmpv2` Rust crate, and incurs no meaningful performance penalty for IPTV:
- Video decode uses `hwdec=videotoolbox` (Apple hardware, unchanged by render path)
- The final frame blit via OpenGL→Metal on M4/Intel is sub-millisecond
- IPTV bottlenecks at network and decode, not rendering

For distribution: `dylibbundler` bundles the source-built `libmpv.dylib` + its transitive deps into the `.app`. Users install nothing.

Build script updated in `scripts/build-libmpv.sh` to clone mpv and build with meson (both GL and Vulkan enabled — GL for the embedded render context, Vulkan for the `vo=gpu` fallback window).

### Transparency
`transparent: true` removed from `tauri.conf.json`. It was never needed and breaks NSOpenGLView.

---

## Architecture

### File Layout (`crates/tauri-plugin-mpv/src/`)

```
lib.rs          plugin init, command registration
commands.rs     Tauri command handlers (public API unchanged)
engine.rs       MpvEngine — create/control libmpv, fully platform-agnostic
state.rs        MpvState — owns engine + renderer, coordinates load/fallback
renderer.rs     PlatformRenderer trait
macos.rs        MacosGlRenderer — NSOpenGLView + OpenGL Core 3.2 render context
windows.rs      WindowsRenderer — stub (HWND/wid, Win32 embeds correctly)
ios.rs          IosRenderer — stub (OpenGL ES render context, future)
android.rs      AndroidRenderer — stub (EGL/OpenGL ES, future)
```

### PlatformRenderer Trait (`renderer.rs`)

```rust
pub trait PlatformRenderer: Send + Sync {
    fn attach(&mut self, mpv: &mut Mpv, app: &AppHandle) -> Result<(), String>;
    fn resize(&mut self, width: u32, height: u32);
    fn detach(&mut self);
}
```

All `#[cfg(target_os)]` blocks are eliminated from `engine.rs`, `state.rs`, and the commands. They exist only inside each platform file.

### MpvState (`state.rs`)

```rust
pub struct MpvState {
    inner: Mutex<MpvEngine>,
    renderer: Mutex<Option<Box<dyn PlatformRenderer>>>,
    fallback_active: AtomicBool,
}
```

### Load Flow

```
mpv_load command (fires once — double-fire bug fixed)
  → MpvEngine::create_instance()       platform-agnostic: create mpv, set hwdec/audio opts
  → PlatformRenderer::attach()         platform file handles all platform-specific setup
      Ok  → embedded playback in Tauri window
      Err → FallbackRenderer::launch() → vo=gpu, native OSC, no --no-osc
              + emit Tauri event "mpv://render-fallback" { reason }
  → mpv.command("loadfile", url)
```

---

## macOS Renderer (`macos.rs`) — Primary Path

### Setup (`attach`)

1. Get Tauri window's `NSView` (contentView) via `raw-window-handle`
2. Create `NSOpenGLView` with pixel format:
   - `NSOpenGLPFADoubleBuffer`
   - `NSOpenGLProfileVersion3_2Core` (**not** Legacy — Legacy was the root cause of "Unsupported")
3. Add `NSOpenGLView` as subview of contentView, positioned below WKWebView
4. Retrieve the view's internally managed context via `[glView openGLContext]` — do NOT create a separate `NSOpenGLContext`
5. Call `[glView prepareOpenGL]` — **required**; without it the context has no CGL drawable and `mpv_render_context_create` returns Unsupported
6. Make context current
7. Call `mpv_render_context_create` with `MPV_RENDER_API_TYPE_OPENGL`
8. Set update callback → dispatches render steps to main thread

### Render Loop (per-frame, main thread)

```
update_callback fires (mpv internal thread)
  → dispatch_async(main_queue)
      → [glContext makeCurrentContext]
      → mpv_render_context_render(fbo=0, width, height, flip_y=true)
      → [glContext flushBuffer]
      → mpv_render_context_report_swap()
```

### Resize

On Tauri window resize:
- `[glView setFrame: newBounds]`
- `[glContext update]`

### Teardown (`detach`)

Drop `RenderContext` first, then `[glView removeFromSuperview]`.

---

## Fallback Path (`state.rs`)

If `PlatformRenderer::attach()` returns `Err`:
- Launch `vo=gpu --gpu-api=auto` — Vulkan/MoltenVK on macOS (native OSC active, controls available in mpv window)
- Emit Tauri event `mpv://render-fallback { reason: String }` to frontend
- Set `fallback_active = true` on `MpvState`
- `useMpv` polling continues unchanged — `mpv_get_state` still works

---

## Frontend Changes

### `useMpv.ts`
- Listen for `mpv://render-fallback` event → set `fallbackActive: true`
- Add `loading` ref guard to prevent double-invocation of `mpv_load`

### `VideoPlayer.tsx`
- When `fallbackActive`: hide player controls + show dismissible warning banner explaining video is in a separate window with native controls

---

## Cross-Platform Renderer Map

| Platform | File | Strategy |
|---|---|---|
| macOS | `macos.rs` | NSOpenGLView + OpenGL Core 3.2 render context |
| Windows | `windows.rs` | HWND + `wid` (`vo=gpu` Win32 embeds correctly) |
| iOS | `ios.rs` | Stub → OpenGL ES render context (future) |
| Android | `android.rs` | Stub → EGL/OpenGL ES render context (future) |

---

## Build Script Changes (`scripts/build-libmpv.sh`)

macOS path changes from copying Homebrew dylib to building from source:

```bash
git clone https://github.com/mpv-player/mpv.git --depth=1
meson setup build \
  --buildtype=release \
  -Dlibmpv=true \
  -Dgl=enabled \
  -Dvulkan=enabled \
  -Dcocoa=enabled \
  -Daudiounit=enabled \
  -Davcodec=enabled \
  -Davformat=enabled \
  -Davutil=enabled \
  -Dswscale=enabled
ninja -C build
cp build/libmpv.dylib ../../libs/macos/
```

Required brew deps for building: `meson ninja pkg-config ffmpeg libass`

---

## Changes to `tauri.conf.json`

- Remove `"transparent": true`
- Remove `"macos-private-api"` if present

---

## Out of Scope

- Metal render context (`MTKView`) — future improvement once OpenGL path is stable
- Vulkan render context for embedded path — requires Vulkan setup code beyond what `libmpv2` crate exposes; deferred
- iOS/Android actual implementation — stubs only
