# MPV Embedding Rewrite Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the broken 3-tier MPV rendering fallback with a clean embedded NSOpenGLView (OpenGL Core 3.2) path on macOS, backed by a single fallback to a native mpv window, using a PlatformRenderer trait that isolates all platform-specific code.

**Architecture:** `MpvEngine` is purely platform-agnostic (create/control libmpv). `PlatformRenderer` trait is implemented per-platform in its own file. `MpvState` coordinates: tries embedded render, on failure launches `vo=gpu` fallback window and emits a Tauri event to the frontend.

**Tech Stack:** Rust (libmpv2 crate, cocoa/objc/dispatch for macOS), React/TypeScript (Tauri events, `@tauri-apps/api`), meson/ninja for building libmpv from source.

---

## Prerequisites: Read Before Starting

Before touching any code, read:
- `docs/plans/2026-03-07-mpv-embedding-rewrite-design.md` — full design rationale
- `crates/tauri-plugin-mpv/src/desktop.rs` — current macOS code being replaced
- `crates/tauri-plugin-mpv/src/engine.rs` — current engine being rewritten
- `crates/tauri-plugin-mpv/src/mpv.rs` — current state being rewritten
- `crates/tauri-plugin-mpv/build.rs` — check linker path setup

Key context:
- `libmpv2` crate v5: `Mpv::with_initializer`, `RenderContext::new`, `RenderParam`, `RenderParamApiType::OpenGl`, `OpenGLInitParams`
- `dispatch` crate: `Queue::main().exec_sync(|| ...)` and `exec_async`
- `cocoa` crate 0.26: `NSOpenGLView`, `NSOpenGLPixelFormat`, `NSOpenGLContext`, `NSView`
- `objc` crate: `msg_send![obj, method]` macro
- All NSView/OpenGL calls MUST be on the main thread

---

## Task 1: Install Build Dependencies

**Files:** `scripts/build-libmpv.sh`

**Step 1: Check what deps are already installed**

```bash
brew list | grep -E "meson|ninja|pkg-config|ffmpeg|libass"
```

**Step 2: Install any missing deps**

```bash
brew install meson ninja pkg-config ffmpeg libass
```

Expected: all five packages present. `ffmpeg` provides libavcodec/libavformat/libavutil/libswscale/libswresample that mpv needs.

**Step 3: Verify meson version is recent**

```bash
meson --version
```

Expected: 1.0.0 or newer.

**Step 4: Commit nothing** — deps are system-level, not tracked.

---

## Task 2: Rewrite build-libmpv.sh (macOS section)

**Files:**
- Modify: `scripts/build-libmpv.sh`

**Step 1: Read the current script**

Open `scripts/build-libmpv.sh` and understand the existing `macos)` case block.

**Step 2: Replace the macOS case block**

Replace the entire `macos)` block with:

```bash
  macos)
    echo "==> Building libmpv from source for macOS..."
    mkdir -p "$LIBS_DIR/macos"

    # Check build tools
    for tool in meson ninja pkg-config; do
      if ! command -v "$tool" &>/dev/null; then
        echo "Error: $tool not found. Run: brew install meson ninja pkg-config"
        exit 1
      fi
    done

    if ! pkg-config --exists libavcodec; then
      echo "Error: ffmpeg not found. Run: brew install ffmpeg"
      exit 1
    fi

    # Clone mpv source (shallow, latest stable)
    MPV_SRC="$LIBS_DIR/mpv-src"
    if [[ ! -d "$MPV_SRC/.git" ]]; then
      echo "    Cloning mpv source..."
      git clone https://github.com/mpv-player/mpv.git --depth=1 "$MPV_SRC"
    else
      echo "    Updating mpv source..."
      git -C "$MPV_SRC" pull --ff-only || true
    fi

    # Build
    BUILD_DIR="$MPV_SRC/build-macos"
    echo "    Running meson setup..."
    meson setup "$BUILD_DIR" "$MPV_SRC" \
      --buildtype=release \
      --wipe \
      -Dlibmpv=true \
      -Dgl=enabled \
      -Dvulkan=enabled \
      -Dcocoa=enabled \
      -Daudiounit=enabled \
      -Davcodec=enabled \
      -Davformat=enabled \
      -Davutil=enabled \
      -Dswscale=enabled \
      -Dswresample=enabled \
      -Dlibass=enabled \
      --prefix="$LIBS_DIR/macos/install"

    echo "    Building mpv (this takes a few minutes)..."
    ninja -C "$BUILD_DIR"

    # Copy dylib to libs/macos/
    DYLIB=$(find "$BUILD_DIR" -name "libmpv*.dylib" | head -1)
    if [[ -z "$DYLIB" ]]; then
      echo "Error: libmpv.dylib not found after build"
      exit 1
    fi
    cp "$DYLIB" "$LIBS_DIR/macos/libmpv.dylib"
    echo "    Built libmpv -> $LIBS_DIR/macos/libmpv.dylib"
    echo "    Done."
    ;;
```

**Step 3: Add libs/mpv-src to .gitignore**

```bash
echo "libs/mpv-src/" >> /Users/maxboksem/Code/Github/Multi/MaxVideoPlayer/.gitignore
echo "libs/macos/" >> /Users/maxboksem/Code/Github/Multi/MaxVideoPlayer/.gitignore
```

**Step 4: Run the build script**

```bash
cd /Users/maxboksem/Code/Github/Multi/MaxVideoPlayer
./scripts/build-libmpv.sh macos
```

Expected: Takes 3-8 minutes. Ends with `Built libmpv -> libs/macos/libmpv.dylib`.

**Step 5: Verify the dylib has OpenGL symbols**

```bash
nm -g libs/macos/libmpv.dylib | grep -i "mpv_render_context_create"
```

Expected: a line like `T _mpv_render_context_create` — confirms the render context API is compiled in.

**Step 6: Verify --gpu-api includes opengl**

```bash
DYLD_LIBRARY_PATH=libs/macos libs/macos/install/bin/mpv --gpu-api=help 2>&1 || true
```

If the install binary exists, `opengl` should appear in the list alongside vulkan.

**Step 7: Commit**

```bash
git add scripts/build-libmpv.sh .gitignore
git commit -m "build: rewrite build-libmpv.sh to compile mpv from source with OpenGL enabled

Homebrew mpv is Vulkan-only; the OpenGL render context (vo=libmpv) requires
a source build with -Dgl=enabled. Users still get a fully bundled .app.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 3: Remove Window Transparency

**Files:**
- Modify: `apps/desktop/src-tauri/tauri.conf.json`

**Step 1: Open the file and find the window config**

The `app.windows[0]` object currently has `"transparent": true`.

**Step 2: Remove transparent and fix the window label**

Change the windows array entry to:

```json
{
  "label": "main",
  "title": "MaxVideoPlayer",
  "width": 1280,
  "height": 720,
  "resizable": true,
  "fullscreen": false,
  "decorations": true
}
```

(`transparent` removed. `label: "main"` added explicitly so Rust code can look it up by label reliably.)

**Step 3: Run the app briefly to confirm no startup warning**

```bash
cd apps/desktop && npx tauri dev 2>&1 | head -20
```

Expected: the `macos-private-api` transparency warning is gone.

**Step 4: Commit**

```bash
git add apps/desktop/src-tauri/tauri.conf.json
git commit -m "fix: remove window transparency that prevented NSOpenGLView from obtaining a CGL drawable

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 4: Define PlatformRenderer Trait

**Files:**
- Create: `crates/tauri-plugin-mpv/src/renderer.rs`

**Step 1: Create renderer.rs**

```rust
//! PlatformRenderer trait — the contract each OS must implement.
//! All platform code lives in its own file (macos.rs / windows.rs / etc.)
//! and is selected at compile time via #[cfg]. This file has no #[cfg] blocks.

use libmpv2::Mpv;

/// Handles embedding the libmpv video surface into the native window.
///
/// Implementations:
/// - macOS  → `macos::MacosGlRenderer`  (NSOpenGLView + OpenGL Core 3.2 render context)
/// - Windows → `windows::WindowsRenderer` (stub; HWND wid works on Win32)
/// - iOS    → `ios::IosRenderer`         (stub)
/// - Android → `android::AndroidRenderer` (stub)
pub trait PlatformRenderer: Send + Sync {
    /// Set up the video surface and attach it to the provided mpv instance.
    ///
    /// Called once per stream load. `mpv` has been created with the platform's
    /// preferred options (`vo=libmpv`, hwdec, audio) but `loadfile` has NOT
    /// been called yet.
    ///
    /// On success the renderer is responsible for pumping frames until `detach`.
    fn attach(&mut self, mpv: &mut Mpv) -> Result<(), String>;

    /// Update the render surface dimensions. Called on window resize.
    fn resize(&mut self, width: u32, height: u32);

    /// Tear down the surface. Called before dropping the renderer.
    /// Must be idempotent.
    fn detach(&mut self);
}
```

**Step 2: Add renderer module to lib.rs**

Open `crates/tauri-plugin-mpv/src/lib.rs` and add `mod renderer;` alongside the other mods. (Do not pub-export it — it's internal.)

**Step 3: No tests yet** — the trait is just a definition. Tests come after implementations.

**Step 4: Confirm it compiles**

```bash
cd /Users/maxboksem/Code/Github/Multi/MaxVideoPlayer
cargo check -p tauri-plugin-mpv
```

Expected: no errors (the module is empty except the trait).

**Step 5: Commit**

```bash
git add crates/tauri-plugin-mpv/src/renderer.rs crates/tauri-plugin-mpv/src/lib.rs
git commit -m "feat(plugin-mpv): add PlatformRenderer trait for cross-platform video embedding

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 5: Rewrite engine.rs (Platform-Agnostic)

**Files:**
- Modify: `crates/tauri-plugin-mpv/src/engine.rs`

**Step 1: Replace the entire file**

The new engine has zero `#[cfg]` blocks. Platform options are passed in by the caller.

```rust
//! Platform-agnostic libmpv instance management.
//! Options (vo, hwdec, ao) are passed in by the caller; this file has no #[cfg].

use libmpv2::Mpv;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    pub is_playing: bool,
    pub is_paused: bool,
    pub current_url: Option<String>,
    pub volume: f64,
    pub position: f64,
    pub duration: f64,
}

pub struct MpvEngine {
    mpv: Option<Mpv>,
    current_url: Option<String>,
}

impl MpvEngine {
    pub fn new() -> Self {
        Self { mpv: None, current_url: None }
    }

    /// Create a new Mpv instance with the provided options.
    /// Drops any existing instance first.
    /// Returns a mutable reference so the caller can attach a render context
    /// before calling `loadfile`.
    pub fn create(&mut self, options: &[(&str, &str)]) -> Result<&mut Mpv, String> {
        self.stop();
        let opts: Vec<(String, String)> = options
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let mpv = Mpv::with_initializer(move |init| {
            for (k, v) in &opts {
                init.set_option(k.as_str(), v.as_str())?;
            }
            Ok(())
        })
        .map_err(|e| format!("mpv init: {}", e))?;
        self.mpv = Some(mpv);
        Ok(self.mpv.as_mut().unwrap())
    }

    /// Issue the loadfile command to start playback.
    /// Must be called AFTER `create` and AFTER the render context is attached.
    pub fn loadfile(&self, url: &str) -> Result<(), String> {
        let mpv = self.mpv.as_ref().ok_or("no mpv instance")?;
        mpv.command("loadfile", &[url, "replace"])
            .map_err(|e| format!("loadfile: {}", e))?;
        Ok(())
    }

    /// Record the current URL (called by MpvState after loadfile succeeds).
    pub fn set_current_url(&mut self, url: &str) {
        self.current_url = Some(url.to_string());
    }

    /// Stop playback and destroy the mpv instance.
    pub fn stop(&mut self) {
        if let Some(ref mpv) = self.mpv {
            let _ = mpv.command("stop", &[]);
        }
        self.mpv = None;
        self.current_url = None;
    }

    pub fn play(&self) -> Result<(), String> {
        let mpv = self.mpv.as_ref().ok_or("no mpv instance")?;
        mpv.set_property("pause", false).map_err(|e| e.to_string())
    }

    pub fn pause(&self) -> Result<(), String> {
        let mpv = self.mpv.as_ref().ok_or("no mpv instance")?;
        mpv.set_property("pause", true).map_err(|e| e.to_string())
    }

    pub fn seek(&self, position: f64) -> Result<(), String> {
        let mpv = self.mpv.as_ref().ok_or("no mpv instance")?;
        mpv.command("seek", &[&position.to_string(), "absolute"])
            .map_err(|e| e.to_string())
    }

    pub fn set_volume(&self, volume: f64) -> Result<(), String> {
        let mpv = self.mpv.as_ref().ok_or("no mpv instance")?;
        let v = volume.clamp(0.0, 150.0);
        mpv.set_property("volume", v).map_err(|e| e.to_string())
    }

    pub fn get_state(&self) -> PlayerState {
        let mut state = PlayerState {
            current_url: self.current_url.clone(),
            volume: 100.0,
            ..Default::default()
        };
        if let Some(ref mpv) = self.mpv {
            state.position = mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
            state.duration = mpv.get_property::<f64>("duration").unwrap_or(0.0);
            state.is_paused = mpv.get_property::<bool>("pause").unwrap_or(false);
            state.volume = mpv.get_property::<f64>("volume").unwrap_or(100.0);
            state.is_playing = !state.is_paused && state.current_url.is_some();
        }
        state
    }
}
```

**Step 2: Check it compiles**

```bash
cargo check -p tauri-plugin-mpv
```

Expected: errors about `engine` usages in `mpv.rs` (old state file) — that's fine; we fix that in Task 8.

**Step 3: Commit**

```bash
git add crates/tauri-plugin-mpv/src/engine.rs
git commit -m "refactor(plugin-mpv): make MpvEngine fully platform-agnostic

Accepts options slice from caller; no #[cfg] blocks. Render context
attachment happens via PlatformRenderer before loadfile is issued.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 6: Rewrite macos.rs

**Files:**
- Modify: `crates/tauri-plugin-mpv/src/desktop.rs` → rename to `macos.rs`

**Step 1: Rename the file**

```bash
mv crates/tauri-plugin-mpv/src/desktop.rs crates/tauri-plugin-mpv/src/macos.rs
```

**Step 2: Replace the entire content of macos.rs**

Key fixes vs current code:
- Pixel format: `NSOpenGLProfileVersion3_2Core` (not Legacy)
- Use `[glView openGLContext]` — let the view manage its own context
- Call `[glView prepareOpenGL]` in `attach` before creating render context
- No separate `NSOpenGLContext::alloc` + `initWithFormat`

```rust
//! macOS MPV embedding — NSOpenGLView with OpenGL Core 3.2 render context.
//!
//! All NSView/OpenGL calls must run on the main thread. Use `dispatch::Queue::main()`.
//!
//! Key fix over previous implementation:
//!   - Pixel format uses Core 3.2 profile (not Legacy — Homebrew source-built libmpv
//!     rejects legacy profile).
//!   - NSOpenGLView manages its own NSOpenGLContext (no separate alloc).
//!   - [glView prepareOpenGL] is called before mpv_render_context_create, which
//!     establishes the CGL drawable without which libmpv returns Unsupported.

#![allow(deprecated)] // cocoa crate deprecated in favour of objc2-*; migration deferred

use crate::renderer::PlatformRenderer;
use cocoa::appkit::{NSOpenGLPixelFormat, NSOpenGLView, NSView};
use cocoa::base::nil;
use cocoa::foundation::{NSAutoreleasePool, NSRect};
use dispatch::Queue;
use libmpv2::{
    render::{
        mpv_render_update, OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType,
    },
    Mpv,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::ffi::{c_char, c_void, CString};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{AppHandle, Runtime};

// NSOpenGLProfileVersion3_2Core — may not be exposed by the cocoa crate; use raw value.
const NS_OPENGL_PROFILE_VERSION_3_2_CORE: u32 = 0x3200;

// ---------------------------------------------------------------------------
// CGLGetProcAddress resolver — used by libmpv to look up OpenGL functions.
// ---------------------------------------------------------------------------

fn cgl_get_proc_address(name: *const c_char) -> *mut c_void {
    type CGLGetProcAddressFn = unsafe extern "C" fn(*const c_char) -> *mut c_void;
    static mut FUNC: Option<CGLGetProcAddressFn> = None;
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let lib =
            CString::new("/System/Library/Frameworks/OpenGL.framework/OpenGL").unwrap();
        let handle = unsafe { libc::dlopen(lib.as_ptr(), libc::RTLD_LAZY) };
        if !handle.is_null() {
            let sym = CString::new("CGLGetProcAddress").unwrap();
            let addr = unsafe { libc::dlsym(handle, sym.as_ptr()) };
            if !addr.is_null() {
                unsafe { FUNC = Some(std::mem::transmute(addr)) };
            }
        }
    });
    match unsafe { FUNC } {
        Some(f) => unsafe { f(name) },
        None => std::ptr::null_mut(),
    }
}

// ---------------------------------------------------------------------------
// MacosGlRenderer
// ---------------------------------------------------------------------------

/// Holds raw Obj-C pointers. All access must be on the main thread.
/// `unsafe impl Send` is safe here: we only ever access these pointers
/// via `Queue::main().exec_sync/async`, which serialises access.
pub struct MacosGlRenderer {
    /// The NSOpenGLView added as a subview of the Tauri content view.
    gl_view: *mut c_void,
    /// NSOpenGLContext retrieved from [gl_view openGLContext].
    gl_context: *mut c_void,
    /// The Tauri window's content view (parent).
    content_view: *mut c_void,
    /// Set to false in Drop; prevents queued render callbacks from firing after cleanup.
    valid: Arc<AtomicBool>,
    /// Render context — Some after attach, None before/after.
    render_ctx: Option<RenderContext>,
}

unsafe impl Send for MacosGlRenderer {}
unsafe impl Sync for MacosGlRenderer {}

impl MacosGlRenderer {
    /// Create the NSOpenGLView and add it below the WKWebView.
    /// Must be called on the main thread (enforced by `Queue::main().exec_sync`).
    pub fn new<R: Runtime>(app: &AppHandle<R>) -> Result<Self, String> {
        Queue::main().exec_sync(|| Self::create_on_main(app))
    }

    fn create_on_main<R: Runtime>(app: &AppHandle<R>) -> Result<Self, String> {
        let window = app
            .get_webview_window("main")
            .ok_or_else(|| "Window 'main' not found".to_string())?;

        let raw = window
            .window_handle()
            .map_err(|e| format!("window handle: {:?}", e))?
            .as_raw();

        let ns_view_ptr = match raw {
            RawWindowHandle::AppKit(h) => h.ns_view.as_ptr(),
            _ => return Err("Expected AppKit window handle".to_string()),
        };

        unsafe { Self::build_gl_view(ns_view_ptr) }
    }

    unsafe fn build_gl_view(content_view_ptr: *mut c_void) -> Result<Self, String> {
        let _pool = NSAutoreleasePool::new(nil);
        let content_view = content_view_ptr as *mut objc::runtime::Object;

        let bounds: NSRect = NSView::bounds(content_view);

        // Core 3.2 profile — Homebrew source-built libmpv requires this.
        // Legacy (0x1000) causes mpv_render_context_create to return Unsupported.
        let attrs: [u32; 5] = [
            cocoa::appkit::NSOpenGLPFAOpenGLProfile as u32,
            NS_OPENGL_PROFILE_VERSION_3_2_CORE,
            cocoa::appkit::NSOpenGLPFADoubleBuffer as u32,
            cocoa::appkit::NSOpenGLPFAAccelerated as u32,
            0,
        ];
        let pixel_format = NSOpenGLPixelFormat::alloc(nil);
        let pixel_format =
            NSOpenGLPixelFormat::initWithAttributes_(pixel_format, &attrs);
        if pixel_format == nil {
            return Err(
                "Failed to create Core 3.2 pixel format. Check that OpenGL is available."
                    .to_string(),
            );
        }

        // NSOpenGLView creates and manages its own context internally.
        // Do NOT create a separate NSOpenGLContext — that was the bug.
        let gl_view = NSOpenGLView::alloc(nil);
        let gl_view =
            NSOpenGLView::initWithFrame_pixelFormat_(gl_view, bounds, pixel_format);
        if gl_view == nil {
            return Err("Failed to create NSOpenGLView".to_string());
        }

        // Retrieve the view's internally managed context.
        let gl_context: *mut objc::runtime::Object = msg_send![gl_view, openGLContext];
        if gl_context.is_null() {
            return Err("NSOpenGLView returned nil openGLContext".to_string());
        }

        // Add our view BELOW the WKWebView in the window hierarchy.
        let _: () = msg_send![
            content_view,
            addSubview: gl_view
            positioned: -1i64   // NSWindowBelow
            relativeTo: nil
        ];

        tracing::info!("[macOS renderer] NSOpenGLView created (Core 3.2)");
        Ok(Self {
            gl_view: gl_view as *mut c_void,
            gl_context: gl_context as *mut c_void,
            content_view: content_view as *mut c_void,
            valid: Arc::new(AtomicBool::new(true)),
            render_ctx: None,
        })
    }

    fn get_view_size(&self) -> (i32, i32) {
        unsafe {
            let view = self.gl_view as *mut objc::runtime::Object;
            let bounds: NSRect = NSView::bounds(view);
            let window: *mut objc::runtime::Object = msg_send![view, window];
            let scale: f64 = if window.is_null() {
                1.0
            } else {
                msg_send![window, backingScaleFactor]
            };
            let w = (bounds.size.width * scale) as i32;
            let h = (bounds.size.height * scale) as i32;
            (w.max(1), h.max(1))
        }
    }
}

impl PlatformRenderer for MacosGlRenderer {
    fn attach(&mut self, mpv: &mut Mpv) -> Result<(), String> {
        let gl_view = self.gl_view;
        let gl_context = self.gl_context;
        let valid = self.valid.clone();

        // All OpenGL setup MUST happen on the main thread.
        let render_ctx = Queue::main().exec_sync(|| -> Result<RenderContext, String> {
            unsafe {
                let view = gl_view as *mut objc::runtime::Object;
                let ctx = gl_context as *mut objc::runtime::Object;

                // prepareOpenGL establishes the CGL drawable.
                // Without this, makeCurrentContext is a no-op and libmpv
                // returns Unsupported from mpv_render_context_create.
                let _: () = msg_send![view, prepareOpenGL];

                // Set the view on the context and make it current.
                cocoa::appkit::NSOpenGLContext::setView_(ctx, view);
                cocoa::appkit::NSOpenGLContext::makeCurrentContext(ctx);
            }

            // get_proc_address: libmpv calls this to resolve OpenGL symbols.
            fn get_proc_address(_ctx: &*mut c_void, name: &str) -> *mut c_void {
                let c = match CString::new(name) {
                    Ok(s) => s,
                    Err(_) => return std::ptr::null_mut(),
                };
                cgl_get_proc_address(c.as_ptr())
            }

            RenderContext::new(
                unsafe { mpv.ctx.as_mut() },
                vec![
                    RenderParam::ApiType(RenderParamApiType::OpenGl),
                    RenderParam::InitParams(OpenGLInitParams {
                        get_proc_address,
                        ctx: std::ptr::null_mut(),
                    }),
                ],
            )
            .map_err(|e| format!("mpv_render_context_create: {}", e))
        })?;

        // Set the per-frame update callback.
        // mpv calls this on its internal thread when a new frame is ready.
        // We dispatch the actual render to the main thread.
        let gl_view_addr = self.gl_view as usize;
        let gl_ctx_addr = self.gl_context as usize;
        // Store render_ctx pointer for callback. The `valid` flag prevents
        // access after `detach` drops the RenderContext.
        let render_ctx_ptr = &render_ctx as *const RenderContext as usize;

        render_ctx.set_update_callback({
            let valid = valid.clone();
            move || {
                let v = valid.clone();
                let view_addr = gl_view_addr;
                let ctx_addr = gl_ctx_addr;
                let rc_addr = render_ctx_ptr;
                Queue::main().exec_async(move || {
                    if !v.load(Ordering::Acquire) {
                        return;
                    }
                    unsafe {
                        render_frame(view_addr, ctx_addr, rc_addr);
                    }
                });
            }
        });

        self.render_ctx = Some(render_ctx);
        tracing::info!("[macOS renderer] render context attached");
        Ok(())
    }

    fn resize(&mut self, _width: u32, _height: u32) {
        let gl_view = self.gl_view;
        let gl_context = self.gl_context;
        let content_view = self.content_view;
        Queue::main().exec_async(move || unsafe {
            let view = gl_view as *mut objc::runtime::Object;
            let ctx = gl_context as *mut objc::runtime::Object;
            let parent = content_view as *mut objc::runtime::Object;
            let bounds: NSRect = NSView::bounds(parent);
            let _: () = msg_send![view, setFrame: bounds];
            let _: () = msg_send![ctx, update]; // required after resize
        });
    }

    fn detach(&mut self) {
        // Stop callbacks first, then drop the render context.
        self.valid.store(false, Ordering::Release);
        // Drop render_ctx — this destroys mpv_render_context internally.
        self.render_ctx = None;

        let gl_view = self.gl_view;
        if !gl_view.is_null() {
            Queue::main().exec_sync(|| unsafe {
                let view = gl_view as *mut objc::runtime::Object;
                let _: () = msg_send![view, removeFromSuperview];
            });
        }
        tracing::info!("[macOS renderer] detached");
    }
}

impl Drop for MacosGlRenderer {
    fn drop(&mut self) {
        self.detach();
    }
}

// ---------------------------------------------------------------------------
// Frame rendering (main thread only)
// ---------------------------------------------------------------------------

/// Called on the main thread by the update callback.
/// `rc_addr` is the address of the RenderContext owned by MacosGlRenderer.
/// Safety: caller must ensure `valid` flag is true and `rc_addr` is live.
unsafe fn render_frame(view_addr: usize, ctx_addr: usize, rc_addr: usize) {
    let view = view_addr as *mut objc::runtime::Object;
    let ctx = ctx_addr as *mut objc::runtime::Object;
    let rc = &*(rc_addr as *const RenderContext);

    // Make GL context current for this thread.
    cocoa::appkit::NSOpenGLContext::setView_(ctx, view);
    cocoa::appkit::NSOpenGLContext::makeCurrentContext(ctx);

    // Get pixel dimensions.
    let bounds: NSRect = NSView::bounds(view);
    let window: *mut objc::runtime::Object = msg_send![view, window];
    let scale: f64 = if window.is_null() {
        1.0
    } else {
        msg_send![window, backingScaleFactor]
    };
    let w = (bounds.size.width * scale) as i32;
    let h = (bounds.size.height * scale) as i32;
    if w < 1 || h < 1 {
        return;
    }

    match rc.update() {
        Ok(flags) => {
            if flags & mpv_render_update::Frame != 0 {
                // fbo=0 = default framebuffer. flip_y=true corrects GL's inverted Y axis.
                if let Err(e) = rc.render::<*mut c_void>(0, w, h, true) {
                    tracing::trace!("[macOS renderer] render error: {}", e);
                    return;
                }
            }
            // Swap buffers to display the frame.
            cocoa::appkit::NSOpenGLContext::flushBuffer(ctx);
            rc.report_swap();
        }
        Err(e) => {
            tracing::trace!("[macOS renderer] update error: {}", e);
        }
    }
}

// ---------------------------------------------------------------------------
// MPV options for macOS
// ---------------------------------------------------------------------------

/// Options for embedded playback (vo=libmpv + render context).
pub fn embedded_options() -> Vec<(&'static str, &'static str)> {
    vec![
        ("vo", "libmpv"),
        ("hwdec", "videotoolbox"),
        ("ao", "coreaudio"),
        ("video-sync", "display-resample"),
        ("cache", "yes"),
        ("demuxer-max-bytes", "150MiB"),
        ("demuxer-max-back-bytes", "75MiB"),
    ]
}

/// Options for the fallback window (vo=gpu, native OSC visible).
pub fn fallback_options() -> Vec<(&'static str, &'static str)> {
    vec![
        ("hwdec", "videotoolbox"),
        ("ao", "coreaudio"),
        ("video-sync", "display-resample"),
        ("cache", "yes"),
        ("demuxer-max-bytes", "150MiB"),
        ("demuxer-max-back-bytes", "75MiB"),
        // vo=gpu is the default; native OSC is shown automatically.
        // Do NOT add --no-osc here.
    ]
}
```

**Step 3: Check compile**

```bash
cargo check -p tauri-plugin-mpv 2>&1 | head -40
```

Fix any import/type errors. Common issues:
- `cocoa::appkit::NSOpenGLContext::setView_` — verify the exact method name in cocoa 0.26 docs
- `rc.render::<*mut c_void>(0, w, h, true)` — check libmpv2 v5 signature (may be `render(fbo, w, h, flip_y)`)

**Step 4: Commit**

```bash
git add crates/tauri-plugin-mpv/src/macos.rs
git commit -m "feat(plugin-mpv): rewrite macOS renderer with NSOpenGLView Core 3.2

Fix: use Core 3.2 profile instead of Legacy (Homebrew source-built libmpv
rejects Legacy). Fix: use view's internally managed context. Fix: call
prepareOpenGL before render context creation.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 7: Update Platform Stubs

**Files:**
- Modify: `crates/tauri-plugin-mpv/src/windows.rs` (create if absent)
- Modify: `crates/tauri-plugin-mpv/src/ios.rs`
- Modify: `crates/tauri-plugin-mpv/src/android.rs`

**Step 1: Replace ios.rs**

```rust
//! iOS MPV renderer stub — full implementation pending.
//!
//! When implemented: UIView + CAMetalLayer + OpenGL ES render context.

use crate::renderer::PlatformRenderer;
use libmpv2::Mpv;

pub struct IosRenderer;

impl IosRenderer {
    pub fn new() -> Self { Self }
}

impl PlatformRenderer for IosRenderer {
    fn attach(&mut self, _mpv: &mut Mpv) -> Result<(), String> {
        Err("iOS embedded rendering not yet implemented".to_string())
    }
    fn resize(&mut self, _w: u32, _h: u32) {}
    fn detach(&mut self) {}
}
```

**Step 2: Replace android.rs**

```rust
//! Android MPV renderer stub — full implementation pending.
//!
//! When implemented: SurfaceView (JNI) + EGL/OpenGL ES render context.
//! Fire Stick: hwdec=mediacodec, ao=opensles.

use crate::renderer::PlatformRenderer;
use libmpv2::Mpv;

pub struct AndroidRenderer;

impl AndroidRenderer {
    pub fn new() -> Self { Self }
}

impl PlatformRenderer for AndroidRenderer {
    fn attach(&mut self, _mpv: &mut Mpv) -> Result<(), String> {
        Err("Android embedded rendering not yet implemented".to_string())
    }
    fn resize(&mut self, _w: u32, _h: u32) {}
    fn detach(&mut self) {}
}
```

**Step 3: Create/replace windows.rs**

```rust
//! Windows MPV renderer — vo=gpu with HWND wid embedding.
//!
//! On Windows, vo=gpu with --wid=<HWND> genuinely embeds into the provided
//! window (unlike macOS where it creates a peer window). This makes it
//! reliable without needing the render context API.

use crate::renderer::PlatformRenderer;
use libmpv2::Mpv;

pub struct WindowsRenderer;

impl WindowsRenderer {
    pub fn new() -> Self { Self }
}

impl PlatformRenderer for WindowsRenderer {
    fn attach(&mut self, _mpv: &mut Mpv) -> Result<(), String> {
        // TODO: get HWND from AppHandle, set --wid, run vo=gpu
        Err("Windows embedded rendering not yet implemented".to_string())
    }
    fn resize(&mut self, _w: u32, _h: u32) {}
    fn detach(&mut self) {}
}
```

**Step 4: Compile check**

```bash
cargo check -p tauri-plugin-mpv
```

**Step 5: Commit**

```bash
git add crates/tauri-plugin-mpv/src/ios.rs crates/tauri-plugin-mpv/src/android.rs crates/tauri-plugin-mpv/src/windows.rs
git commit -m "refactor(plugin-mpv): convert platform stubs to PlatformRenderer trait

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 8: Rewrite state.rs (MpvState)

**Files:**
- Modify: `crates/tauri-plugin-mpv/src/mpv.rs`

**Step 1: Replace the entire file**

```rust
//! MpvState — Tauri managed state for the MPV plugin.
//!
//! Owns the MpvEngine and the active PlatformRenderer.
//! Coordinates the load flow: try embedded → on failure, fallback window.

use crate::engine::{MpvEngine, PlayerState};
use crate::renderer::PlatformRenderer;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{AppHandle, Emitter, Runtime};

struct Inner {
    engine: MpvEngine,
    renderer: Option<Box<dyn PlatformRenderer>>,
}

pub struct MpvState {
    inner: Mutex<Inner>,
    pub fallback_active: AtomicBool,
}

impl MpvState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                engine: MpvEngine::new(),
                renderer: None,
            }),
            fallback_active: AtomicBool::new(false),
        }
    }

    /// Load a URL for playback.
    ///
    /// Returns Ok(()) regardless of whether embedded or fallback rendering is used.
    /// If fallback is used, emits "mpv://render-fallback" to the frontend.
    pub fn load<R: Runtime>(&self, url: &str, app: &AppHandle<R>) -> Result<(), String> {
        let mut inner = self.inner.lock().map_err(|e| e.to_string())?;

        // Tear down any existing renderer + stop previous playback.
        if let Some(mut r) = inner.renderer.take() {
            r.detach();
        }
        inner.engine.stop();

        // Platform-specific setup.
        #[cfg(target_os = "macos")]
        let (opts, make_renderer) = {
            let app_clone = app.clone();
            let opts = crate::macos::embedded_options();
            let make: Box<dyn FnOnce() -> Result<Box<dyn PlatformRenderer>, String>> =
                Box::new(move || {
                    crate::macos::MacosGlRenderer::new(&app_clone)
                        .map(|r| Box::new(r) as Box<dyn PlatformRenderer>)
                });
            (opts, make)
        };

        #[cfg(target_os = "windows")]
        let (opts, make_renderer) = {
            let opts = vec![];
            let make: Box<dyn FnOnce() -> Result<Box<dyn PlatformRenderer>, String>> =
                Box::new(|| Ok(Box::new(crate::windows::WindowsRenderer::new())));
            (opts, make)
        };

        #[cfg(target_os = "ios")]
        let (opts, make_renderer) = {
            let opts = vec![];
            let make: Box<dyn FnOnce() -> Result<Box<dyn PlatformRenderer>, String>> =
                Box::new(|| Ok(Box::new(crate::ios::IosRenderer::new())));
            (opts, make)
        };

        #[cfg(target_os = "android")]
        let (opts, make_renderer) = {
            let opts = vec![];
            let make: Box<dyn FnOnce() -> Result<Box<dyn PlatformRenderer>, String>> =
                Box::new(|| Ok(Box::new(crate::android::AndroidRenderer::new())));
            (opts, make)
        };

        // Try embedded rendering.
        let opts_refs: Vec<(&str, &str)> = opts.iter().map(|(k, v)| (*k, *v)).collect();
        let mpv = inner.engine.create(&opts_refs)?;

        let embed_result = make_renderer().and_then(|mut renderer| {
            renderer.attach(mpv).map(|()| renderer)
        });

        match embed_result {
            Ok(renderer) => {
                inner.engine.loadfile(url)?;
                inner.engine.set_current_url(url);
                inner.renderer = Some(renderer);
                self.fallback_active.store(false, Ordering::SeqCst);
                tracing::info!("[MpvState] embedded playback started url={}", url);
            }
            Err(embed_err) => {
                tracing::warn!(
                    "[MpvState] embedded rendering failed: {}. Launching fallback window.",
                    embed_err
                );

                // Recreate mpv with fallback (vo=gpu) options — no render context.
                inner.engine.stop();

                #[cfg(target_os = "macos")]
                let fallback_opts = crate::macos::fallback_options();
                #[cfg(not(target_os = "macos"))]
                let fallback_opts: Vec<(&str, &str)> = vec![];

                let fallback_refs: Vec<(&str, &str)> =
                    fallback_opts.iter().map(|(k, v)| (*k, *v)).collect();
                inner.engine.create(&fallback_refs)?;
                inner.engine.loadfile(url)?;
                inner.engine.set_current_url(url);

                self.fallback_active.store(true, Ordering::SeqCst);

                // Inform the frontend.
                let _ = app.emit(
                    "mpv://render-fallback",
                    serde_json::json!({ "reason": embed_err }),
                );
                tracing::info!("[MpvState] fallback window playback started url={}", url);
            }
        }

        Ok(())
    }

    pub fn play(&self) -> Result<(), String> {
        self.inner.lock().map_err(|e| e.to_string())?.engine.play()
    }

    pub fn pause(&self) -> Result<(), String> {
        self.inner.lock().map_err(|e| e.to_string())?.engine.pause()
    }

    pub fn stop(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            if let Some(mut r) = inner.renderer.take() {
                r.detach();
            }
            inner.engine.stop();
        }
        self.fallback_active.store(false, Ordering::SeqCst);
    }

    pub fn seek(&self, position: f64) -> Result<(), String> {
        self.inner.lock().map_err(|e| e.to_string())?.engine.seek(position)
    }

    pub fn set_volume(&self, volume: f64) -> Result<(), String> {
        self.inner.lock().map_err(|e| e.to_string())?.engine.set_volume(volume)
    }

    pub fn get_state(&self) -> PlayerState {
        self.inner
            .lock()
            .map(|g| g.engine.get_state())
            .unwrap_or_default()
    }

    /// Forward window resize to the active renderer.
    pub fn resize(&self, width: u32, height: u32) {
        if let Ok(mut inner) = self.inner.lock() {
            if let Some(ref mut r) = inner.renderer {
                r.resize(width, height);
            }
        }
    }
}
```

**Step 2: Compile check**

```bash
cargo check -p tauri-plugin-mpv 2>&1 | head -60
```

Common issues:
- Missing `pub use` for `PlayerState` in lib.rs — add it
- `serde_json` not in plugin Cargo.toml — add `serde_json = { workspace = true }` to dependencies

**Step 3: Add serde_json to plugin Cargo.toml if missing**

```toml
serde_json = { workspace = true }
```

**Step 4: Commit**

```bash
git add crates/tauri-plugin-mpv/src/mpv.rs crates/tauri-plugin-mpv/Cargo.toml
git commit -m "feat(plugin-mpv): rewrite MpvState with PlatformRenderer trait and clean fallback

Single embedded attempt → one fallback (vo=gpu native window + Tauri event).
No more 3-tier fallback chain. Engine is platform-agnostic.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 9: Update Plugin lib.rs and commands.rs

**Files:**
- Modify: `crates/tauri-plugin-mpv/src/lib.rs`
- Modify: `crates/tauri-plugin-mpv/src/commands.rs`

**Step 1: Rewrite lib.rs**

```rust
#![allow(dead_code)]

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

mod commands;
mod engine;
pub mod mpv;
mod renderer;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "ios")]
mod ios;
#[cfg(target_os = "android")]
mod android;

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use mpv::MpvState;
pub use engine::PlayerState;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("mpv")
        .invoke_handler(tauri::generate_handler![
            commands::mpv_load,
            commands::mpv_play,
            commands::mpv_pause,
            commands::mpv_stop,
            commands::mpv_seek,
            commands::mpv_set_volume,
            commands::mpv_get_state,
        ])
        .setup(|app, _api| {
            app.manage(MpvState::new());
            tracing::info!("MPV plugin initialized");
            Ok(())
        })
        .build()
}
```

**Step 2: Update commands.rs**

The `mpv_load` command must call `state.load(url, app)` (new signature):

```rust
use crate::mpv::{MpvState, PlayerState};
use tauri::{command, AppHandle, Runtime, State};

#[command]
pub async fn mpv_load<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, MpvState>,
    url: String,
) -> Result<(), String> {
    tracing::info!("[MPV cmd] load url={}", url);
    state.load(&url, &app)
}

#[command]
pub async fn mpv_play<R: Runtime>(
    _app: AppHandle<R>,
    state: State<'_, MpvState>,
) -> Result<(), String> {
    state.play()
}

#[command]
pub async fn mpv_pause<R: Runtime>(
    _app: AppHandle<R>,
    state: State<'_, MpvState>,
) -> Result<(), String> {
    state.pause()
}

#[command]
pub async fn mpv_stop<R: Runtime>(
    _app: AppHandle<R>,
    state: State<'_, MpvState>,
) -> Result<(), String> {
    state.stop();
    Ok(())
}

#[command]
pub async fn mpv_seek<R: Runtime>(
    _app: AppHandle<R>,
    state: State<'_, MpvState>,
    position: f64,
) -> Result<(), String> {
    state.seek(position)
}

#[command]
pub async fn mpv_set_volume<R: Runtime>(
    _app: AppHandle<R>,
    state: State<'_, MpvState>,
    volume: f64,
) -> Result<(), String> {
    state.set_volume(volume)
}

#[command]
pub async fn mpv_get_state<R: Runtime>(
    _app: AppHandle<R>,
    state: State<'_, MpvState>,
) -> Result<PlayerState, String> {
    Ok(state.get_state())
}
```

**Step 3: Full compile check**

```bash
cargo check -p tauri-plugin-mpv
cargo check -p max-video-player
```

Fix all errors.

**Step 4: Commit**

```bash
git add crates/tauri-plugin-mpv/src/lib.rs crates/tauri-plugin-mpv/src/commands.rs
git commit -m "refactor(plugin-mpv): update lib.rs and commands.rs for new MpvState API

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 10: Update apps/desktop/src-tauri/src/lib.rs

**Files:**
- Modify: `apps/desktop/src-tauri/src/lib.rs`

**Step 1: Replace the macOS window event handler**

The old code called `state.update_surface_frame()` (which no longer exists). Replace it with `state.resize(w, h)`:

Find the `#[cfg(target_os = "macos")]` block and change it to:

```rust
#[cfg(target_os = "macos")]
{
    if let Some(window) = app.get_webview_window("main") {
        let handle = app.handle().clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::Resized(size) = event {
                if let Some(state) =
                    handle.try_state::<tauri_plugin_mpv::MpvState>()
                {
                    state.resize(size.width, size.height);
                }
            }
        });
    }
}
```

**Step 2: Compile check**

```bash
cargo check -p max-video-player
```

**Step 3: Commit**

```bash
git add apps/desktop/src-tauri/src/lib.rs
git commit -m "fix(desktop): use MpvState::resize instead of removed update_surface_frame

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 11: Update Plugin Cargo.toml

**Files:**
- Modify: `crates/tauri-plugin-mpv/Cargo.toml`

**Step 1: Remove unused macOS dependencies**

`objc2` and `core-graphics` are not used in the new code. Remove them:

```toml
[target.'cfg(target_os = "macos")'.dependencies]
cocoa = "0.26"
objc = "0.2"
dispatch = "0.2"
libc = "0.2"
```

(Remove `objc2` and `core-graphics` from this block.)

**Step 2: Compile check**

```bash
cargo check -p tauri-plugin-mpv
```

**Step 3: Commit**

```bash
git add crates/tauri-plugin-mpv/Cargo.toml
git commit -m "chore(plugin-mpv): remove unused objc2 and core-graphics dependencies

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 12: Fix Double-Load Bug in useMpv.ts

**Files:**
- Modify: `apps/desktop/src/hooks/useMpv.ts`

**Step 1: Add a loading guard ref**

Add `const loadingRef = useRef(false)` and guard the `load` function:

```typescript
export function useMpv() {
  const [state, setState] = useState<PlayerState>(DEFAULT_STATE);
  const [error, setError] = useState<string | null>(null);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const loadingRef = useRef(false); // guard against concurrent load calls

  const load = useCallback(async (url: string) => {
    if (loadingRef.current) return; // debounce
    loadingRef.current = true;
    setError(null);
    try {
      await mpvLoad(url);
      setState((s) => ({ ...s, currentUrl: url, isPlaying: true, isPaused: false }));
    } catch (e) {
      const msg = String(e);
      setError(msg);
      throw e;
    } finally {
      loadingRef.current = false;
    }
  }, []);

  // ... rest unchanged
```

**Step 2: Start the app and play a channel. Verify only ONE set of load logs appears**

```bash
cd apps/desktop && npx tauri dev
```

Check terminal — should see a single `[MPV cmd] load url=...` not two.

**Step 3: Commit**

```bash
git add apps/desktop/src/hooks/useMpv.ts
git commit -m "fix(frontend): prevent concurrent mpv_load calls with loading guard ref

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 13: Add Fallback UI in VideoPlayer.tsx

**Files:**
- Modify: `apps/desktop/src/components/player/VideoPlayer.tsx`

**Step 1: Import Tauri event listener**

Add to existing imports:

```typescript
import { listen } from "@tauri-apps/api/event";
```

**Step 2: Add fallback state and event listener**

Add inside `PlayerView()`:

```typescript
const [fallbackReason, setFallbackReason] = useState<string | null>(null);

useEffect(() => {
  const unlisten = listen<{ reason: string }>(
    "mpv://render-fallback",
    (event) => {
      setFallbackReason(event.payload.reason);
    }
  );
  return () => { unlisten.then((fn) => fn()); };
}, []);

// Clear fallback banner when a new stream loads successfully
useEffect(() => {
  if (mpv.state.isPlaying && !mpv.error) {
    // Only clear if we didn't just get a fallback event
    // (the event arrives after state.isPlaying, so check fallbackActive on MpvState)
    // Simple approach: clear on stop
  }
}, []);
```

**Step 3: Add fallback banner and hide controls when in fallback**

Inside the JSX, add the banner above `<Controls>`:

```tsx
{fallbackReason && (
  <div className="absolute top-12 left-4 right-4 z-50 bg-yellow-900/90 border border-yellow-600 rounded-md p-3 flex items-start justify-between gap-3">
    <div>
      <p className="text-yellow-200 text-sm font-medium">
        Video playing in a separate window
      </p>
      <p className="text-yellow-400 text-xs mt-0.5">
        Use the controls in that window. Reason: {fallbackReason}
      </p>
    </div>
    <button
      onClick={() => setFallbackReason(null)}
      className="text-yellow-400 hover:text-yellow-200 text-xs shrink-0"
    >
      Dismiss
    </button>
  </div>
)}
```

Wrap `<Controls>` to hide when in fallback:

```tsx
{!fallbackReason && (
  <Controls
    state={{ ... }}
    visible={showControls}
    ...
  />
)}
```

**Step 4: Verify in the browser: play a stream**

If embedding works, no banner. If fallback triggers, yellow banner appears and controls hide.

**Step 5: Commit**

```bash
git add apps/desktop/src/components/player/VideoPlayer.tsx
git commit -m "feat(frontend): show fallback warning banner when mpv uses separate window

Hides app controls when fallback active since mpv's native OSC handles them.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 14: End-to-End Test

**Step 1: Build libmpv if not already done**

```bash
ls libs/macos/libmpv.dylib
```

If missing: `./scripts/build-libmpv.sh macos`

**Step 2: Set DYLD_LIBRARY_PATH and run dev**

```bash
cd apps/desktop
DYLD_LIBRARY_PATH=/Users/maxboksem/Code/Github/Multi/MaxVideoPlayer/libs/macos npx tauri dev
```

**Step 3: Load a stream and check logs**

Expected log sequence (success path):

```
INFO tauri_plugin_mpv: MPV plugin initialized
INFO tauri_plugin_mpv::mpv: [MpvState] trying embedded rendering...
INFO tauri_plugin_mpv::macos: [macOS renderer] NSOpenGLView created (Core 3.2)
INFO tauri_plugin_mpv::macos: [macOS renderer] render context attached
INFO tauri_plugin_mpv::mpv: [MpvState] embedded playback started url=...
```

**NOT expected:** `render API failed`, `wid embedding`, `fallback window`.

**Step 4: Verify video renders inside the Tauri window** (not in a new window)

**Step 5: Verify controls work** — play/pause/seek/volume via the in-app controls.

**Step 6: Update CLAUDE.md**

Add to the macOS Setup section:

```markdown
### macOS Setup (required before first build)

```bash
# Install build dependencies
brew install meson ninja pkg-config ffmpeg libass dylibbundler

# Build libmpv from source (required — Homebrew mpv is Vulkan-only and
# cannot embed via the OpenGL render context API)
./scripts/build-libmpv.sh macos

# Set library path for tauri dev
export DYLD_LIBRARY_PATH="$PWD/libs/macos"
```

> **Why from source:** Homebrew's mpv formula uses MoltenVK (Vulkan-only).
> The OpenGL render context (`vo=libmpv`) requires a source build with `-Dgl=enabled`.
> The compiled dylib is bundled into the `.app` via `dylibbundler` for distribution.
```

**Step 7: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md with source-build libmpv requirement and DYLD setup

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Troubleshooting

**`cargo check` fails: "can't find crate for libmpv2"**
→ The libmpv2-sys crate needs to link against libmpv. Set:
```bash
export LIBRARY_PATH="$PWD/libs/macos:$LIBRARY_PATH"
```

**mpv_render_context_create still returns Unsupported after fix**
→ Run with `RUST_LOG=trace` and check the exact error. Possible causes:
- `prepareOpenGL` returned before the view was in a window → ensure the Tauri window is fully visible before calling `load`
- Core 3.2 pixel format returned nil → macOS version is too old (need 10.7+)

**Video appears but outside Tauri window (separate window)**
→ The fallback path is triggering. Check logs for the specific error from `attach`. If `prepareOpenGL` path is reached, the render context creation itself failed.

**Double-load still happening**
→ Check if `useEffect` deps include `navState?.url` — if it's re-evaluated on every render, add a `loadedUrl` ref to track what was already loaded.
