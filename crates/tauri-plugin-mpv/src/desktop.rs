//! macOS-specific MPV embedding using NSOpenGLView.
//! Creates an OpenGL view below the webview for video rendering via libmpv's render API.

#![allow(deprecated)] // cocoa crate deprecated in favor of objc2-*; migration deferred

use cocoa::appkit::{NSOpenGLContext, NSOpenGLPixelFormat, NSOpenGLView};
use cocoa::base::nil;
use cocoa::foundation::NSAutoreleasePool;
use dispatch::Queue;
use libmpv2::{
    render::{
        mpv_render_update, OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType,
    },
    Mpv,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::ffi::{c_char, c_void, CString};

fn cgl_get_proc_address(name: *const c_char) -> *mut c_void {
    // Use dlsym to resolve CGLGetProcAddress at runtime (avoids link issues on some macOS setups)
    type CGLGetProcAddressFn = unsafe extern "C" fn(*const c_char) -> *mut c_void;
    static mut CGL_GET_PROC_ADDRESS: Option<CGLGetProcAddressFn> = None;
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let opengl =
            std::ffi::CString::new("/System/Library/Frameworks/OpenGL.framework/OpenGL").unwrap();
        let handle = unsafe { libc::dlopen(opengl.as_ptr(), libc::RTLD_LAZY) };
        if !handle.is_null() {
            let sym = std::ffi::CString::new("CGLGetProcAddress").unwrap();
            let addr = unsafe { libc::dlsym(handle, sym.as_ptr()) };
            if !addr.is_null() {
                unsafe {
                    CGL_GET_PROC_ADDRESS = Some(std::mem::transmute(addr));
                }
            }
        }
    });
    match unsafe { CGL_GET_PROC_ADDRESS } {
        Some(f) => unsafe { f(name) },
        None => std::ptr::null_mut(),
    }
}

use cocoa::appkit::NSView;
use cocoa::foundation::NSRect;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Manager;

#[derive(Clone, Copy)]
#[repr(i64)]
enum NSWindowOrderingMode {
    NSWindowOut = 0,
    NSWindowAbove = 1,
    NSWindowBelow = -1,
}

/// macOS video surface - NSOpenGLView + render context.
pub struct MacosSurface {
    gl_view: *mut c_void,
    gl_context: *mut c_void,
    gl_pixel_format: *mut c_void,
    content_view: *mut c_void,
}

unsafe impl Send for MacosSurface {}
unsafe impl Sync for MacosSurface {}

impl MacosSurface {
    /// Create a macOS rendering surface and add it below the webview.
    /// Must be called on the main thread.
    pub fn create<R: tauri::Runtime>(
        app: &tauri::AppHandle<R>,
        window_label: &str,
    ) -> Result<Self, String> {
        let window = app
            .get_webview_window(window_label)
            .ok_or_else(|| format!("Window '{}' not found", window_label))?;

        let raw = window
            .window_handle()
            .map_err(|e| format!("Failed to get window handle: {:?}", e))?
            .as_raw();

        let ns_view = match raw {
            RawWindowHandle::AppKit(handle) => handle.ns_view.as_ptr(),
            _ => return Err("Unsupported platform: expected AppKit".to_string()),
        };

        if ns_view.is_null() {
            return Err("NSView pointer is null".to_string());
        }

        unsafe { Self::create_from_ns_view(ns_view) }
    }

    /// Create surface from raw NSView pointer. Used when we already have the handle.
    pub unsafe fn create_from_ns_view(ns_view: *mut c_void) -> Result<Self, String> {
        if ns_view.is_null() {
            return Err("NSView is null".to_string());
        }

        let _pool = NSAutoreleasePool::new(nil);

        let content_view = ns_view as *mut objc::runtime::Object;

        // Get content view bounds for our GL view
        let bounds: NSRect = NSView::bounds(content_view);
        let frame = bounds;

        // OpenGL 2.1 Legacy profile - mpv supports it, and macOS/Homebrew libmpv
        // often rejects 3.2 Core at mpv_render_context_create (Unsupported).
        let attrs = [
            cocoa::appkit::NSOpenGLPFAOpenGLProfile as u32,
            cocoa::appkit::NSOpenGLProfileVersionLegacy as u32,
            cocoa::appkit::NSOpenGLPFADoubleBuffer as u32,
            cocoa::appkit::NSOpenGLPFAAccelerated as u32,
            0_u32,
        ];
        let pixel_format = NSOpenGLPixelFormat::alloc(nil);
        let pixel_format = NSOpenGLPixelFormat::initWithAttributes_(pixel_format, &attrs);
        if pixel_format == nil {
            return Err("Failed to create NSOpenGLPixelFormat".to_string());
        }

        // Create OpenGL context
        let gl_context = NSOpenGLContext::alloc(nil);
        let gl_context = NSOpenGLContext::initWithFormat_shareContext_(gl_context, pixel_format, nil);
        if gl_context == nil {
            return Err("Failed to create NSOpenGLContext".to_string());
        }

        // Create NSOpenGLView
        let gl_view = NSOpenGLView::alloc(nil);
        let gl_view = NSOpenGLView::initWithFrame_pixelFormat_(gl_view, frame, pixel_format);
        if gl_view == nil {
            return Err("Failed to create NSOpenGLView".to_string());
        }

        // Set the context on the view
        NSOpenGLView::setOpenGLContext_(gl_view, gl_context);

        // Add our view below the webview (at the back)
        let _: () = msg_send![
            content_view,
            addSubview: gl_view
            positioned: NSWindowOrderingMode::NSWindowBelow
            relativeTo: nil
        ];

        Ok(Self {
            gl_view: gl_view as *mut c_void,
            gl_context: gl_context as *mut c_void,
            gl_pixel_format: pixel_format as *mut c_void,
            content_view: content_view as *mut c_void,
        })
    }

    /// Create mpv RenderContext for the given Mpv instance.
    /// Caller must be on main thread; GL context must be current (we ensure that here).
    pub fn create_render_context(&self, mpv: &mut Mpv) -> Result<RenderContext, String> {
        fn get_proc_address(_ctx: &*mut c_void, name: &str) -> *mut c_void {
            let c_name = match CString::new(name) {
                Ok(s) => s,
                Err(_) => return std::ptr::null_mut(),
            };
            cgl_get_proc_address(c_name.as_ptr())
        }

        // GL context must be current when mpv creates its render context
        let _ = self.make_current_and_get_size()?;

        let ctx = self.gl_context;
        RenderContext::new(
            unsafe { mpv.ctx.as_mut() },
            vec![
                RenderParam::ApiType(RenderParamApiType::OpenGl),
                RenderParam::InitParams(OpenGLInitParams {
                    get_proc_address: get_proc_address,
                    ctx,
                }),
            ],
        )
        .map_err(|e| crate::engine::format_mpv_error(&e))
    }

    /// Make the OpenGL context current and return the view dimensions.
    pub fn make_current_and_get_size(&self) -> Result<(i32, i32), String> {
        unsafe {
            let ctx = self.gl_context as *mut objc::runtime::Object;
            let view = self.gl_view as *mut objc::runtime::Object;
            NSOpenGLContext::setView_(ctx, view);
            NSOpenGLContext::makeCurrentContext(ctx);

            let bounds: NSRect = NSView::bounds(view);
            let window: *mut objc::runtime::Object = msg_send![view, window];
            let scale: f64 = msg_send![window, backingScaleFactor];
            let width = (bounds.size.width * scale) as i32;
            let height = (bounds.size.height * scale) as i32;
            Ok((width.max(1), height.max(1)))
        }
    }

    /// Swap buffers to present the frame.
    pub fn swap_buffers(&self) {
        unsafe {
            NSOpenGLContext::flushBuffer(self.gl_context as *mut objc::runtime::Object);
        }
    }

    /// Update the GL view frame to match the content view (e.g. on resize).
    pub fn update_frame(&self) {
        unsafe {
            let view = self.gl_view as *mut objc::runtime::Object;
            let bounds: NSRect = NSView::bounds(self.content_view as *mut objc::runtime::Object);
            let _: () = msg_send![view, setFrameSize: bounds.size];
            let _: () = msg_send![view, setFrameOrigin: bounds.origin];
        }
    }

    /// Get the raw NSView pointer for reference.
    pub fn ns_view_ptr(&self) -> *mut c_void {
        self.gl_view
    }

    /// Get the native view ID for mpv's wid option (embedding via vo=gpu).
    pub fn wid(&self) -> isize {
        self.gl_view as isize
    }
}

impl Drop for MacosSurface {
    fn drop(&mut self) {
        if !self.gl_view.is_null() {
            unsafe {
                let view = self.gl_view as *mut objc::runtime::Object;
                let _: () = msg_send![view, removeFromSuperview];
            }
        }
    }
}

/// Inner state for the render loop - holds surface and ctx. Accessed from main thread only.
struct MacosRenderJobInner {
    surface_ptr: *const MacosSurface,
    ctx: RenderContext,
}

/// Wraps raw ptr so the exec_async closure is Send. Access is main-thread only.
#[derive(Clone, Copy)]
struct SendableRenderPtr(usize); // stores ptr as usize for Send
unsafe impl Send for SendableRenderPtr {}

impl SendableRenderPtr {
    fn from_ptr(p: *mut MacosRenderJobInner) -> Self {
        Self(p as usize)
    }
    fn as_ptr(self) -> *mut MacosRenderJobInner {
        self.0 as *mut MacosRenderJobInner
    }
}

/// macOS render job - connects libmpv RenderContext to the surface via a main-thread dispatch.
/// The update callback runs on mpv's thread; the actual render runs on main (required for AppKit/OpenGL).
pub struct MacosRenderJob {
    /// Set to false in Drop before freeing inner; callbacks check this to avoid use-after-free.
    valid: Arc<AtomicBool>,
    inner: Box<MacosRenderJobInner>,
}

unsafe impl Send for MacosRenderJob {}

impl Drop for MacosRenderJob {
    fn drop(&mut self) {
        // Prevent any in-flight or queued callbacks from dereferencing the inner.
        self.valid.store(false, Ordering::Release);
    }
}

impl MacosRenderJob {
    /// Create a render context and set up the update callback.
    /// `surface` must remain valid for the lifetime of the returned job.
    pub fn new(mpv: &mut Mpv, surface: &MacosSurface) -> Result<Self, String> {
        let ctx = surface.create_render_context(mpv)?;
        let valid = Arc::new(AtomicBool::new(true));
        let valid_clone = valid.clone();
        let inner = Box::new(MacosRenderJobInner {
            surface_ptr: surface as *const MacosSurface,
            ctx,
        });
        let inner_ptr = Box::into_raw(inner);
        let ptr_addr = inner_ptr as usize;
        unsafe {
            (*inner_ptr).ctx.set_update_callback(move || {
                let send_ptr = SendableRenderPtr(ptr_addr);
                let v = valid_clone.clone();
                Queue::main().exec_async(move || {
                    if !v.load(Ordering::Acquire) {
                        return;
                    }
                    MacosRenderJob::render_step_from_sendable(send_ptr);
                });
            });
        }
        Ok(Self {
            valid,
            inner: unsafe { Box::from_raw(inner_ptr) },
        })
    }

    fn render_step_from_sendable(ptr: SendableRenderPtr) {
        let inner_ptr = ptr.as_ptr();
        if inner_ptr.is_null() {
            return;
        }
        unsafe {
            let inner = &*inner_ptr;
            let surface = &*inner.surface_ptr;
            let (w, h) = match surface.make_current_and_get_size() {
                Ok(sz) => sz,
                Err(e) => {
                tracing::trace!("[MPV render] make_current failed: {}", e);
                return;
            }
        };
        match inner.ctx.update() {
            Ok(flags) => {
                if flags & mpv_render_update::Frame != 0 {
                    let _ = inner.ctx.render::<*mut c_void>(0, w, h, true);
                }
                surface.swap_buffers();
                inner.ctx.report_swap();
            }
            Err(e) => {
                tracing::trace!("[MPV render] update failed: {}", e);
            }
        }
        }
    }
}
pub fn macos_mpv_options() -> Vec<(&'static str, &'static str)> {
    vec![
        ("hwdec", "videotoolbox"),
        ("vo", "libmpv"),
        ("ao", "coreaudio"),
        ("video-sync", "display-resample"),
        ("interpolation", "yes"),
        ("cache", "yes"),
        ("demuxer-max-bytes", "150MiB"),
        ("demuxer-max-back-bytes", "75MiB"),
    ]
}

/// Options for vo=gpu (wid embedding or separate window). Excludes vo=libmpv.
pub fn macos_mpv_options_for_gpu() -> Vec<(&'static str, &'static str)> {
    vec![
        ("hwdec", "videotoolbox"),
        ("ao", "coreaudio"),
        ("video-sync", "display-resample"),
        ("interpolation", "yes"),
        ("cache", "yes"),
        ("demuxer-max-bytes", "150MiB"),
        ("demuxer-max-back-bytes", "75MiB"),
    ]
}
