//! Linux MPV embedding — X11 child window with EGL/OpenGL render context.
//!
//! Architecture mirrors macos.rs:
//! - X11 child window created within the Tauri parent (like NSOpenGLView addSubview:)
//! - EGL context for OpenGL rendering (like NSOpenGLContext)
//! - mpv_render_context with MPV_RENDER_API_TYPE_OPENGL
//! - Render callback dispatched to GLib main thread (like dispatch::Queue::main())
//!
//! Wayland: not yet implemented; set GDK_BACKEND=x11 for XWayland fallback.

use crate::renderer::PlatformRenderer;
use khronos_egl as egl;
use libmpv2::{
    render::{
        mpv_render_update, OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType,
    },
    Mpv,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::ffi::{c_void, CString};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, OnceLock,
};
use tauri::{AppHandle, Manager, Runtime};

// ---------------------------------------------------------------------------
// EGL instance (cached, loaded once)
// ---------------------------------------------------------------------------

/// Dynamically-loaded EGL instance, cached for the process lifetime.
fn egl_instance() -> &'static egl::DynamicInstance<egl::EGL1_4> {
    static INSTANCE: OnceLock<egl::DynamicInstance<egl::EGL1_4>> = OnceLock::new();
    INSTANCE.get_or_init(|| unsafe {
        egl::DynamicInstance::<egl::EGL1_4>::load_required()
            .expect("Failed to load EGL — is libEGL.so installed?")
    })
}

// ---------------------------------------------------------------------------
// GL proc address resolver for libmpv
// ---------------------------------------------------------------------------

fn gl_get_proc_address(name: &str) -> *mut c_void {
    let egl = egl_instance();
    match CString::new(name) {
        Ok(c) => egl.get_proc_address(c.as_c_str()).map_or(std::ptr::null_mut(), |f| f as *mut c_void),
        Err(_) => std::ptr::null_mut(),
    }
}

// ---------------------------------------------------------------------------
// Render callback inner state (heap-stable, accessed from glib main thread)
// ---------------------------------------------------------------------------

struct RenderInner {
    ctx: RenderContext,
    egl_display: egl::Display,
    egl_surface: egl::Surface,
    egl_context: egl::Context,
    /// Called once when the first frame is rendered, then cleared.
    first_frame_cb: Option<Box<dyn FnOnce() + Send>>,
    /// Shared with LinuxGlRenderer::set_visible; skips GPU calls when false.
    video_active: Arc<AtomicBool>,
}

unsafe impl Send for RenderInner {}

// ---------------------------------------------------------------------------
// LinuxGlRenderer
// ---------------------------------------------------------------------------

/// Embeds libmpv video inside the Tauri window using an X11 child window + EGL/OpenGL.
///
/// Raw pointer fields are only accessed via glib main-thread dispatch, ensuring
/// all X11/EGL calls happen on the correct thread. The `valid` flag prevents
/// callbacks from firing after detach().
pub struct LinuxGlRenderer {
    egl_display: egl::Display,
    egl_surface: egl::Surface,
    egl_context: egl::Context,
    egl_config: egl::Config,
    x11_display: Option<*mut c_void>,
    x11_child_window: u64,
    x11_parent_window: u64,
    xlib: Option<x11_dl::xlib::Xlib>,
    valid: Arc<AtomicBool>,
    /// Heap-stable render state. Box address is captured in the update callback.
    render_inner: Option<Box<RenderInner>>,
    /// Called once on first rendered frame; moved into RenderInner during attach().
    first_frame_cb: Option<Box<dyn FnOnce() + Send>>,
    /// Controls whether GPU rendering happens. Toggled by set_visible().
    /// Starts true (video visible when player page is first entered).
    video_active: Arc<AtomicBool>,
}

unsafe impl Send for LinuxGlRenderer {}
unsafe impl Sync for LinuxGlRenderer {}

impl LinuxGlRenderer {
    /// Create an X11 child window + EGL context within the Tauri parent window.
    pub fn new<R: Runtime>(app: &AppHandle<R>) -> Result<Self, String> {
        let window = app
            .get_webview_window("main")
            .ok_or_else(|| "Window 'main' not found".to_string())?;

        let raw = window
            .window_handle()
            .map_err(|e| format!("window handle: {:?}", e))?
            .as_raw();

        match raw {
            RawWindowHandle::Xlib(h) => {
                let parent_window = h.window;
                let x11_display_ptr = h.display.map(|d| d.as_ptr()).unwrap_or(std::ptr::null_mut());
                if x11_display_ptr.is_null() {
                    return Err("X11 display pointer is null".to_string());
                }
                Self::build_x11(parent_window, x11_display_ptr)
            }
            RawWindowHandle::Xcb(h) => {
                // XCB handle — open our own Xlib connection for XCreateSimpleWindow etc.
                let parent_window = h.window.get() as u64;
                let xlib = x11_dl::xlib::Xlib::open()
                    .map_err(|e| format!("Failed to open Xlib: {}", e))?;
                let display = unsafe { (xlib.XOpenDisplay)(std::ptr::null()) };
                if display.is_null() {
                    return Err("XOpenDisplay returned null".to_string());
                }
                Self::build_x11_with_xlib(parent_window, display as *mut c_void, xlib, true)
            }
            RawWindowHandle::Wayland(_) => {
                Err("Wayland embedded renderer not yet implemented — set GDK_BACKEND=x11 for XWayland fallback".to_string())
            }
            _ => Err(format!("Unsupported window handle type on Linux: {:?}", raw)),
        }
    }

    fn build_x11(parent_window: u64, x11_display_ptr: *mut c_void) -> Result<Self, String> {
        let xlib = x11_dl::xlib::Xlib::open()
            .map_err(|e| format!("Failed to open Xlib: {}", e))?;
        Self::build_x11_with_xlib(parent_window, x11_display_ptr, xlib, false)
    }

    fn build_x11_with_xlib(
        parent_window: u64,
        x11_display_ptr: *mut c_void,
        xlib: x11_dl::xlib::Xlib,
        owns_display: bool,
    ) -> Result<Self, String> {
        let egl = egl_instance();

        // --- EGL setup ---
        let egl_display = unsafe {
            egl.get_display(x11_display_ptr)
        }.ok_or("eglGetDisplay failed")?;

        egl.initialize(egl_display)
            .map_err(|e| format!("eglInitialize: {:?}", e))?;

        // Choose an EGL config with OpenGL ES 3.0 (compatible with mpv's GL requirements)
        let config_attribs = [
            egl::RED_SIZE, 8,
            egl::GREEN_SIZE, 8,
            egl::BLUE_SIZE, 8,
            egl::ALPHA_SIZE, 8,
            egl::DEPTH_SIZE, 0,
            egl::STENCIL_SIZE, 0,
            egl::RENDERABLE_TYPE, egl::OPENGL_BIT,
            egl::SURFACE_TYPE, egl::WINDOW_BIT,
            egl::NONE,
        ];

        let config = egl.choose_first_config(egl_display, &config_attribs)
            .map_err(|e| format!("eglChooseConfig: {:?}", e))?
            .ok_or("No suitable EGL config found")?;

        // Bind OpenGL API (not GLES)
        egl.bind_api(egl::OPENGL_API)
            .map_err(|e| format!("eglBindApi(OPENGL_API): {:?}", e))?;

        // --- X11 child window ---
        let x_display = x11_display_ptr as *mut x11_dl::xlib::Display;

        // Get visual info from the EGL config's native visual ID
        let mut native_visual_id: i32 = 0;
        egl.get_config_attrib(egl_display, config, egl::NATIVE_VISUAL_ID, &mut native_visual_id)
            .map_err(|e| format!("eglGetConfigAttrib(NATIVE_VISUAL_ID): {:?}", e))?;

        let screen = unsafe { (xlib.XDefaultScreen)(x_display) };

        // Create a child window within the parent (Tauri) window
        let child_window = unsafe {
            let root = (xlib.XRootWindow)(x_display, screen);
            let black = (xlib.XBlackPixel)(x_display, screen);

            // Get parent geometry to size child window to fill it initially
            let mut root_ret: u64 = 0;
            let mut x_ret: i32 = 0;
            let mut y_ret: i32 = 0;
            let mut w_ret: u32 = 0;
            let mut h_ret: u32 = 0;
            let mut border_ret: u32 = 0;
            let mut depth_ret: u32 = 0;
            (xlib.XGetGeometry)(
                x_display,
                parent_window,
                &mut root_ret,
                &mut x_ret, &mut y_ret,
                &mut w_ret, &mut h_ret,
                &mut border_ret, &mut depth_ret,
            );

            let child = (xlib.XCreateSimpleWindow)(
                x_display,
                parent_window, // parent
                0, 0,          // x, y
                w_ret.max(1), h_ret.max(1), // width, height
                0,             // border width
                black,         // border color
                black,         // background color
            );

            if child == 0 {
                return Err("XCreateSimpleWindow failed".to_string());
            }

            // Map the child window (make it visible)
            (xlib.XMapWindow)(x_display, child);

            // Lower the child window below all siblings so the WebView overlay stays on top.
            // This mirrors macOS's addSubview:positioned:NSWindowBelow.
            (xlib.XLowerWindow)(x_display, child);

            (xlib.XFlush)(x_display);

            child
        };

        // --- EGL surface on the child window ---
        let egl_surface = unsafe {
            egl.create_window_surface(egl_display, config, child_window as egl::NativeWindowType, None)
        }.map_err(|e| format!("eglCreateWindowSurface: {:?}", e))?;

        // --- EGL context ---
        let context_attribs = [
            egl::CONTEXT_MAJOR_VERSION, 3,
            egl::CONTEXT_MINOR_VERSION, 2,
            egl::CONTEXT_OPENGL_PROFILE_MASK, egl::CONTEXT_OPENGL_CORE_PROFILE_BIT,
            egl::NONE,
        ];

        let egl_context = egl.create_context(egl_display, config, None, &context_attribs)
            .map_err(|e| format!("eglCreateContext: {:?}", e))?;

        tracing::info!("[Linux renderer] X11 child window + EGL context created (OpenGL Core 3.2)");

        Ok(Self {
            egl_display,
            egl_surface,
            egl_context,
            egl_config: config,
            x11_display: Some(x11_display_ptr),
            x11_child_window: child_window,
            x11_parent_window: parent_window,
            xlib: Some(xlib),
            valid: Arc::new(AtomicBool::new(true)),
            render_inner: None,
            first_frame_cb: None,
            video_active: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Set a callback that fires exactly once when the first video frame is rendered.
    pub fn set_first_frame_callback(&mut self, cb: Box<dyn FnOnce() + Send>) {
        self.first_frame_cb = Some(cb);
    }
}

impl PlatformRenderer for LinuxGlRenderer {
    fn attach(&mut self, mpv: &mut Mpv) -> Result<(), String> {
        let egl = egl_instance();

        // Make EGL context current before creating render context
        egl.make_current(self.egl_display, Some(self.egl_surface), Some(self.egl_surface), Some(self.egl_context))
            .map_err(|e| format!("eglMakeCurrent: {:?}", e))?;

        // Create mpv render context with OpenGL
        fn get_proc_address(_ctx: &*mut c_void, name: &str) -> *mut c_void {
            gl_get_proc_address(name)
        }

        let render_ctx = RenderContext::new(
            unsafe { &mut *mpv.ctx.as_ptr() },
            vec![
                RenderParam::ApiType(RenderParamApiType::OpenGl),
                RenderParam::InitParams(OpenGLInitParams {
                    get_proc_address,
                    ctx: std::ptr::null_mut(),
                }),
            ],
        )
        .map_err(|e| format!("mpv_render_context_create: {}", e))?;

        // Heap-allocate the inner state for a stable address captured by the callback.
        let mut inner = Box::new(RenderInner {
            ctx: render_ctx,
            egl_display: self.egl_display,
            egl_surface: self.egl_surface,
            egl_context: self.egl_context,
            first_frame_cb: self.first_frame_cb.take(),
            video_active: self.video_active.clone(),
        });

        // The raw pointer into the Box contents is stable (heap address never changes).
        let inner_ptr = &*inner as *const RenderInner as usize;
        let valid = self.valid.clone();

        inner.ctx.set_update_callback(move || {
            let v = valid.clone();
            // Dispatch to GLib main thread (mirrors macOS dispatch::Queue::main().exec_async)
            glib::idle_add_once(move || {
                if !v.load(Ordering::Acquire) {
                    return;
                }
                unsafe { render_frame(inner_ptr) };
            });
        });

        self.render_inner = Some(inner);
        tracing::info!("[Linux renderer] render context attached");
        Ok(())
    }

    fn resize(&mut self, _width: u32, _height: u32) {
        // No-op: set_frame() handles all positioning/sizing.
    }

    fn set_frame(&mut self, x: f64, y: f64, w: f64, h: f64) {
        // Linux uses top-left origin (same as CSS) — no Y-flip needed.
        if let (Some(ref xlib), Some(display)) = (&self.xlib, self.x11_display) {
            let x_display = display as *mut x11_dl::xlib::Display;
            unsafe {
                (xlib.XMoveResizeWindow)(
                    x_display,
                    self.x11_child_window,
                    x as i32,
                    y as i32,
                    (w as u32).max(1),
                    (h as u32).max(1),
                );
                (xlib.XFlush)(x_display);
            }
        }
    }

    fn set_visible(&mut self, visible: bool) {
        // Stop GPU rendering immediately (atomic, no dispatch needed).
        self.video_active.store(visible, Ordering::Release);

        if let (Some(ref xlib), Some(display)) = (&self.xlib, self.x11_display) {
            let x_display = display as *mut x11_dl::xlib::Display;
            unsafe {
                if visible {
                    (xlib.XMapWindow)(x_display, self.x11_child_window);
                } else {
                    (xlib.XUnmapWindow)(x_display, self.x11_child_window);
                }
                (xlib.XFlush)(x_display);
            }
        }
    }

    fn detach(&mut self) {
        // Signal all queued callbacks to bail before we free the render state.
        self.valid.store(false, Ordering::Release);

        let egl = egl_instance();

        // Make context current so mpv_render_context_free() can clean up GL resources.
        let _ = egl.make_current(
            self.egl_display,
            Some(self.egl_surface),
            Some(self.egl_surface),
            Some(self.egl_context),
        );

        // Drop RenderContext (-> mpv_render_context_free) with GL context current.
        let render_inner = self.render_inner.take();
        drop(render_inner);

        // Clean up EGL resources
        let _ = egl.make_current(self.egl_display, None, None, None);
        let _ = egl.destroy_surface(self.egl_display, self.egl_surface);
        let _ = egl.destroy_context(self.egl_display, self.egl_context);

        // Destroy X11 child window
        if let (Some(ref xlib), Some(display)) = (&self.xlib, self.x11_display) {
            let x_display = display as *mut x11_dl::xlib::Display;
            if self.x11_child_window != 0 {
                unsafe {
                    (xlib.XDestroyWindow)(x_display, self.x11_child_window);
                    (xlib.XFlush)(x_display);
                }
            }
        }

        self.x11_child_window = 0;
        tracing::info!("[Linux renderer] detached");
    }
}

impl Drop for LinuxGlRenderer {
    fn drop(&mut self) {
        self.detach();
    }
}

// ---------------------------------------------------------------------------
// Per-frame rendering — glib main thread only
// ---------------------------------------------------------------------------

/// Render one frame. Called on the glib main thread by the update callback.
/// Safety: caller must verify `valid = true`; `inner_ptr` must be live (owned by LinuxGlRenderer).
unsafe fn render_frame(inner_ptr: usize) {
    static FRAME_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let inner = &mut *(inner_ptr as *mut RenderInner);

    // Skip GPU work when the player page is not active (set_visible(false) clears this flag).
    if !inner.video_active.load(Ordering::Acquire) {
        return;
    }

    let egl = egl_instance();

    // Make EGL context current
    if egl.make_current(
        inner.egl_display,
        Some(inner.egl_surface),
        Some(inner.egl_surface),
        Some(inner.egl_context),
    ).is_err() {
        return;
    }

    // Query surface dimensions
    let mut w: i32 = 0;
    let mut h: i32 = 0;
    let _ = egl.query_surface(inner.egl_display, inner.egl_surface, egl::WIDTH, &mut w);
    let _ = egl.query_surface(inner.egl_display, inner.egl_surface, egl::HEIGHT, &mut h);
    if w < 1 || h < 1 {
        return;
    }

    let rc = &inner.ctx as *const RenderContext;
    let rc: &RenderContext = &*rc;
    match rc.update() {
        Ok(flags) => {
            if flags & mpv_render_update::Frame != 0 {
                // fbo=0 = default framebuffer. flip_y=true corrects GL framebuffer orientation.
                if let Err(e) = rc.render::<*mut c_void>(0, w, h, true) {
                    tracing::trace!("[Linux renderer] render error: {}", e);
                    return;
                }
                // Present the frame
                let _ = egl.swap_buffers(inner.egl_display, inner.egl_surface);
                rc.report_swap();
                let n = FRAME_COUNT.fetch_add(1, Ordering::Relaxed);
                if n < 5 || n % 60 == 0 {
                    tracing::debug!("[Linux renderer] frame presented (#{n})");
                }
                // Notify on first frame so the frontend can switch from opaque to transparent.
                if let Some(cb) = inner.first_frame_cb.take() {
                    cb();
                }
            }
        }
        Err(e) => tracing::trace!("[Linux renderer] update error: {}", e),
    }
}

// ---------------------------------------------------------------------------
// MPV option sets for Linux
// ---------------------------------------------------------------------------

/// Options for embedded playback via OpenGL render context (vo=libmpv).
pub fn embedded_options() -> Vec<(&'static str, &'static str)> {
    vec![
        ("vo", "libmpv"),
        ("hwdec", "auto"),
        ("ao", "pulse,alsa"),
        ("video-sync", "display-resample"),
        ("cache", "yes"),
        ("demuxer-max-bytes", "150MiB"),
        ("demuxer-max-back-bytes", "75MiB"),
        ("keep-open", "yes"),
    ]
}

/// Options for fallback separate window (vo=gpu, native OSC shown automatically).
pub fn fallback_options() -> Vec<(&'static str, &'static str)> {
    vec![
        ("hwdec", "auto"),
        ("ao", "pulse,alsa"),
        ("video-sync", "display-resample"),
        ("cache", "yes"),
        ("demuxer-max-bytes", "150MiB"),
        ("demuxer-max-back-bytes", "75MiB"),
        ("keep-open", "yes"),
    ]
}
