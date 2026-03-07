//! macOS MPV embedding — NSOpenGLView with OpenGL Core 3.2 render context.
//!
//! Root cause fixes vs previous implementation:
//! - NSOpenGLProfileVersion3_2Core (not Legacy): source-built libmpv rejects the legacy profile
//! - NSOpenGLView manages its own context ([glView openGLContext]): no separate NSOpenGLContext alloc
//! - [glView prepareOpenGL] called in attach(): establishes CGL drawable; without it,
//!   mpv_render_context_create returns Unsupported
//!
//! Threading: ALL NSView/OpenGL calls must be on the main thread via dispatch::Queue::main().

#![allow(deprecated)] // cocoa crate uses deprecated NSOpenGL APIs; migration to objc2 deferred

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
use tauri::{AppHandle, Manager, Runtime};

/// Wrapper to pass non-Send types through dispatch queue boundaries.
/// Safety: caller guarantees main-thread-only access.
struct UnsafeSend<T>(T);
unsafe impl<T> Send for UnsafeSend<T> {}

/// Raw value for NSOpenGLProfileVersion3_2Core (not always exposed by cocoa 0.26).
const NS_OPENGL_PROFILE_VERSION_3_2_CORE: u32 = 0x3200;

// ---------------------------------------------------------------------------
// CGLGetProcAddress — resolves OpenGL function pointers for libmpv
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
// Render callback inner state (heap-stable, accessed from main thread only)
// ---------------------------------------------------------------------------

struct RenderInner {
    ctx: RenderContext,
    gl_view: *mut c_void,
    gl_context: *mut c_void,
}

unsafe impl Send for RenderInner {}

// ---------------------------------------------------------------------------
// MacosGlRenderer
// ---------------------------------------------------------------------------

/// Embeds libmpv video inside the Tauri window using NSOpenGLView + OpenGL Core 3.2.
///
/// Obj-C pointer fields are only accessed via dispatch::Queue::main(), ensuring
/// all AppKit calls happen on the main thread. The `valid` flag prevents callbacks
/// from firing after detach().
pub struct MacosGlRenderer {
    gl_view: *mut c_void,
    gl_context: *mut c_void,
    content_view: *mut c_void,
    valid: Arc<AtomicBool>,
    /// Heap-stable render state. Box address is captured in the update callback.
    render_inner: Option<Box<RenderInner>>,
}

unsafe impl Send for MacosGlRenderer {}
unsafe impl Sync for MacosGlRenderer {}

impl MacosGlRenderer {
    /// Create NSOpenGLView on the main thread and add it below the Tauri WKWebView.
    pub fn new<R: Runtime>(app: &AppHandle<R>) -> Result<Self, String> {
        let window = app
            .get_webview_window("main")
            .ok_or_else(|| "Window 'main' not found".to_string())?;

        let raw = window
            .window_handle()
            .map_err(|e| format!("window handle: {:?}", e))?
            .as_raw();

        let ns_view_ptr = match raw {
            RawWindowHandle::AppKit(h) => h.ns_view.as_ptr() as *mut c_void,
            _ => return Err("Expected AppKit window handle".to_string()),
        };

        let ns_view_addr = ns_view_ptr as usize;
        Queue::main().exec_sync(move || unsafe { Self::build_on_main(ns_view_addr as *mut c_void) })
    }

    unsafe fn build_on_main(content_view_ptr: *mut c_void) -> Result<Self, String> {
        let _pool = NSAutoreleasePool::new(nil);
        let content_view = content_view_ptr as *mut objc::runtime::Object;
        let bounds: NSRect = NSView::bounds(content_view);

        // Core 3.2 profile. Source-built libmpv (with -Dgl=enabled) requires this.
        // NSOpenGLProfileVersionLegacy (0x1000) was the previous bug — libmpv rejects it.
        let attrs: [u32; 5] = [
            cocoa::appkit::NSOpenGLPFAOpenGLProfile as u32,
            NS_OPENGL_PROFILE_VERSION_3_2_CORE,
            cocoa::appkit::NSOpenGLPFADoubleBuffer as u32,
            cocoa::appkit::NSOpenGLPFAAccelerated as u32,
            0,
        ];
        let pixel_format = NSOpenGLPixelFormat::alloc(nil);
        let pixel_format = NSOpenGLPixelFormat::initWithAttributes_(pixel_format, &attrs);
        if pixel_format == nil {
            return Err(
                "NSOpenGLPixelFormat (Core 3.2) init failed — check OpenGL availability"
                    .to_string(),
            );
        }

        // Let NSOpenGLView create and manage its own OpenGL context.
        // Do NOT alloc a separate NSOpenGLContext — that was the second bug.
        let gl_view = NSOpenGLView::alloc(nil);
        let gl_view = NSOpenGLView::initWithFrame_pixelFormat_(gl_view, bounds, pixel_format);
        if gl_view == nil {
            return Err("NSOpenGLView initWithFrame:pixelFormat: failed".to_string());
        }

        // Retrieve the view's internally managed context.
        let gl_context: *mut objc::runtime::Object = msg_send![gl_view, openGLContext];
        if gl_context.is_null() {
            return Err("NSOpenGLView returned nil openGLContext".to_string());
        }

        // Position our view BELOW the WKWebView (NSWindowBelow = -1).
        let _: () = msg_send![
            content_view,
            addSubview: gl_view
            positioned: -1i64
            relativeTo: nil
        ];

        tracing::info!("[macOS renderer] NSOpenGLView created (Core 3.2 profile)");
        Ok(Self {
            gl_view: gl_view as *mut c_void,
            gl_context: gl_context as *mut c_void,
            content_view: content_view as *mut c_void,
            valid: Arc::new(AtomicBool::new(true)),
            render_inner: None,
        })
    }
}

impl PlatformRenderer for MacosGlRenderer {
    fn attach(&mut self, mpv: &mut Mpv) -> Result<(), String> {
        let gl_view = self.gl_view;
        let gl_context = self.gl_context;

        // All OpenGL setup must be on the main thread.
        // RenderContext is not Send, so we wrap it in UnsafeSend to cross the dispatch boundary.
        let gl_view_ptr = gl_view as usize;
        let gl_context_ptr = gl_context as usize;
        // SAFETY: mpv.ctx is valid for the duration of this sync call; we're on main thread.
        let mpv_ctx_addr = mpv.ctx.as_ptr() as usize;

        let result: UnsafeSend<Result<RenderContext, String>> =
            Queue::main().exec_sync(move || -> UnsafeSend<Result<RenderContext, String>> {
                let view = gl_view_ptr as *mut objc::runtime::Object;
                let ctx = gl_context_ptr as *mut objc::runtime::Object;
                // Type inferred from RenderContext::new signature; no need to name libmpv2_sys.
                let mpv_ctx = mpv_ctx_addr as *mut _;

                unsafe {
                    // prepareOpenGL establishes the CGL drawable.
                    // This was the critical missing step: without it, makeCurrentContext
                    // is a no-op and mpv_render_context_create returns Unsupported.
                    let _: () = msg_send![view, prepareOpenGL];

                    // Associate context with view and make it current.
                    cocoa::appkit::NSOpenGLContext::setView_(ctx, view);
                    cocoa::appkit::NSOpenGLContext::makeCurrentContext(ctx);
                }

                fn get_proc_address(_ctx: &*mut c_void, name: &str) -> *mut c_void {
                    match CString::new(name) {
                        Ok(c) => cgl_get_proc_address(c.as_ptr()),
                        Err(_) => std::ptr::null_mut(),
                    }
                }

                UnsafeSend(
                    RenderContext::new(
                        unsafe { &mut *mpv_ctx },
                        vec![
                            RenderParam::ApiType(RenderParamApiType::OpenGl),
                            RenderParam::InitParams(OpenGLInitParams {
                                get_proc_address,
                                ctx: std::ptr::null_mut(),
                            }),
                        ],
                    )
                    .map_err(|e| format!("mpv_render_context_create: {}", e)),
                )
            });
        let render_ctx: RenderContext = result.0?;

        // Heap-allocate the inner state for a stable address captured by the callback.
        let mut inner = Box::new(RenderInner {
            ctx: render_ctx,
            gl_view,
            gl_context,
        });

        // The raw pointer into the Box contents is stable (heap address never changes).
        let inner_ptr = &*inner as *const RenderInner as usize;
        let valid = self.valid.clone();

        inner.ctx.set_update_callback(move || {
            let v = valid.clone();
            Queue::main().exec_async(move || {
                if !v.load(Ordering::Acquire) {
                    return;
                }
                unsafe { render_frame(inner_ptr) };
            });
        });

        self.render_inner = Some(inner);
        tracing::info!("[macOS renderer] render context attached");
        Ok(())
    }

    fn resize(&mut self, _width: u32, _height: u32) {
        // Cast to usize so the closure is Send (raw pointers are not Send).
        let gl_view_ptr = self.gl_view as usize;
        let gl_context_ptr = self.gl_context as usize;
        let content_view_ptr = self.content_view as usize;
        Queue::main().exec_async(move || unsafe {
            let view = gl_view_ptr as *mut objc::runtime::Object;
            let ctx = gl_context_ptr as *mut objc::runtime::Object;
            let parent = content_view_ptr as *mut objc::runtime::Object;
            let bounds: NSRect = NSView::bounds(parent);
            let _: () = msg_send![view, setFrame: bounds];
            // Required after resize to update the GL context's drawable geometry.
            let _: () = msg_send![ctx, update];
        });
    }

    fn detach(&mut self) {
        // Signal all queued callbacks to bail before we free the render state.
        self.valid.store(false, Ordering::Release);

        // Drop RenderContext first — calls mpv_render_context_free internally.
        self.render_inner = None;

        // Remove the GL view from the window hierarchy on the main thread.
        let gl_view = self.gl_view;
        if !gl_view.is_null() {
            let gl_view_ptr = gl_view as usize;
            // exec_sync ensures removal completes before we return (no dangling).
            Queue::main().exec_sync(move || unsafe {
                let view = gl_view_ptr as *mut objc::runtime::Object;
                let _: () = msg_send![view, removeFromSuperview];
            });
            self.gl_view = std::ptr::null_mut();
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
// Per-frame rendering — main thread only
// ---------------------------------------------------------------------------

/// Render one frame. Called on the main thread by the update callback.
/// Safety: caller must verify `valid = true`; `inner_ptr` must be live (owned by MacosGlRenderer).
unsafe fn render_frame(inner_ptr: usize) {
    let inner = &*(inner_ptr as *const RenderInner);
    let view = inner.gl_view as *mut objc::runtime::Object;
    let ctx = inner.gl_context as *mut objc::runtime::Object;
    let rc = &inner.ctx;

    cocoa::appkit::NSOpenGLContext::setView_(ctx, view);
    cocoa::appkit::NSOpenGLContext::makeCurrentContext(ctx);

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
            // Present the frame.
            cocoa::appkit::NSOpenGLContext::flushBuffer(ctx);
            rc.report_swap();
        }
        Err(e) => tracing::trace!("[macOS renderer] update error: {}", e),
    }
}

// ---------------------------------------------------------------------------
// MPV option sets for macOS
// ---------------------------------------------------------------------------

/// Options for embedded playback via OpenGL render context (vo=libmpv).
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

/// Options for fallback separate window (vo=gpu, native OSC shown automatically).
pub fn fallback_options() -> Vec<(&'static str, &'static str)> {
    vec![
        ("hwdec", "videotoolbox"),
        ("ao", "coreaudio"),
        ("video-sync", "display-resample"),
        ("cache", "yes"),
        ("demuxer-max-bytes", "150MiB"),
        ("demuxer-max-back-bytes", "75MiB"),
        // vo=gpu is the default; do NOT add --no-osc so native controls are visible.
    ]
}
