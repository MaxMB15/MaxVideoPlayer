//! Linux MPV embedding — EGL + OpenGL Core 3.2 render context.
//!
//! Architecture mirrors macos.rs:
//! - X11: creates a child window via XCreateWindow within the Tauri parent window
//!        (like NSOpenGLView addSubview:positioned:relativeTo:).
//! - Wayland: creates a wl_subsurface + wl_egl_window within the parent wl_surface
//!            (the same conceptual "child window" for Wayland sessions).
//! - EGL context for OpenGL rendering (like NSOpenGLContext on macOS).
//! - mpv_render_context with MPV_RENDER_API_TYPE_OPENGL (same API as macOS).
//! - Render callbacks dispatched to the GLib main thread (like dispatch::Queue::main()).

use crate::renderer::PlatformRenderer;
use khronos_egl as egl;
use libmpv2::{
    render::{
        mpv_render_update, OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType,
    },
    Mpv,
};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use std::ffi::c_void;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, OnceLock,
};
use tauri::{AppHandle, Manager, Runtime};

// ---------------------------------------------------------------------------
// Wayland protocol imports
// ---------------------------------------------------------------------------

use wayland_client::{
    backend::{Backend, ObjectId},
    globals::{registry_queue_init, GlobalListContents},
    protocol::{
        wl_compositor::WlCompositor,
        wl_registry::WlRegistry,
        wl_subcompositor::WlSubcompositor,
        wl_subsurface::WlSubsurface,
        wl_surface::WlSurface,
    },
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
};
use wayland_egl::WlEglSurface;

// ---------------------------------------------------------------------------
// EGL instance (cached, loaded once per process)
// ---------------------------------------------------------------------------

/// Load `libEGL.so` and cache the outcome (success or error string) for the process.
///
/// [`LinuxGlRenderer::new`] calls this first and returns `Err` on failure so the app can
/// fall back (e.g. separate mpv window) without panicking. `egl_instance()` reads the same
/// cache; it panics only if EGL never loaded successfully—an invariant normally established
/// by a successful `new`.
fn try_load_egl() -> Result<&'static egl::DynamicInstance<egl::EGL1_4>, String> {
    static INSTANCE: OnceLock<Result<egl::DynamicInstance<egl::EGL1_4>, String>> = OnceLock::new();
    let result = INSTANCE.get_or_init(|| unsafe {
        egl::DynamicInstance::<egl::EGL1_4>::load_required()
            .map_err(|e| format!("Failed to load EGL (libEGL.so): {}", e))
    });
    result.as_ref().map_err(|e| e.clone())
}

fn egl_instance() -> &'static egl::DynamicInstance<egl::EGL1_4> {
    match try_load_egl() {
        Ok(instance) => instance,
        Err(err) => panic!(
            "egl_instance() called when EGL is unavailable (expected after successful LinuxGlRenderer::new): {}",
            err
        ),
    }
}

// ---------------------------------------------------------------------------
// GL proc address resolver for libmpv
// ---------------------------------------------------------------------------

fn gl_get_proc_address(name: &str) -> *mut c_void {
    let egl = egl_instance();
    egl.get_proc_address(name)
        .map_or(std::ptr::null_mut(), |f| f as *mut c_void)
}

// ---------------------------------------------------------------------------
// Render callback inner state (heap-stable, accessed from glib main thread)
// ---------------------------------------------------------------------------

/// Pending resize request queued by `set_frame()` and applied by `render_frame()`
/// on the GLib main thread, ensuring wl_egl_window_resize never races with EGL calls.
struct PendingResize {
    w: i32,
    h: i32,
}

struct RenderInner {
    ctx: RenderContext,
    egl_display: egl::Display,
    egl_surface: egl::Surface,
    egl_context: egl::Context,
    /// Called once when the first video frame is rendered, then cleared.
    first_frame_cb: Option<Box<dyn FnOnce() + Send>>,
    /// Shared with LinuxGlRenderer::set_visible; skips GPU calls when false.
    video_active: Arc<AtomicBool>,
    /// Pending wl_egl_window resize — set by set_frame (command thread),
    /// consumed by render_frame (GLib main thread). Protected by Mutex so
    /// wl_egl_window_resize never races with eglSwapBuffers.
    pending_resize: Arc<Mutex<Option<PendingResize>>>,
    /// Pointer to the WlEglSurface so render_frame can call resize() on the
    /// GLib main thread. Only valid while LinuxGlRenderer is alive.
    wl_egl_surface_ptr: usize,
    /// X11 only: display pointer and child window for stacking enforcement.
    /// render_frame periodically re-lowers the video window to keep it
    /// below WebKit's GdkWindow (GTK event processing can restack siblings).
    x11_display_ptr: usize,
    x11_child_window: u64,
}

unsafe impl Send for RenderInner {}

// ---------------------------------------------------------------------------
// Cached Xlib for stacking operations from the GLib render thread
// ---------------------------------------------------------------------------

struct SyncXlib(x11_dl::xlib::Xlib);
unsafe impl Send for SyncXlib {}
unsafe impl Sync for SyncXlib {}

/// Lazily load Xlib for X11 stacking operations called from render_frame().
/// Safe to cache: libX11.so is already loaded by GTK and stays resident.
fn xlib_for_stacking() -> Option<&'static SyncXlib> {
    static XLIB: OnceLock<Option<SyncXlib>> = OnceLock::new();
    XLIB.get_or_init(|| x11_dl::xlib::Xlib::open().ok().map(SyncXlib))
        .as_ref()
}

/// Extract the X11 window ID (XID) from a GDK window, if running under X11.
/// Uses `dlsym` at runtime so we don't create a hard link-time dependency on
/// libgdkx11-3.so (absent on pure-Wayland systems).
fn gdk_x11_window_xid(gdk_win: &gdk::Window) -> Option<u64> {
    use gdk::prelude::*;
    use gdk::glib::translate::ToGlibPtr;

    type GdkX11WindowGetXid = unsafe extern "C" fn(*mut gdk::ffi::GdkWindow) -> u64;

    let is_x11 = gdk::Display::default()
        .map(|d| d.type_().name() == "GdkX11Display")
        .unwrap_or(false);
    if !is_x11 {
        return None;
    }

    let func: GdkX11WindowGetXid = unsafe {
        let sym = libc::dlsym(
            libc::RTLD_DEFAULT,
            b"gdk_x11_window_get_xid\0".as_ptr() as *const _,
        );
        if sym.is_null() {
            return None;
        }
        std::mem::transmute(sym)
    };

    unsafe { Some(func(gdk_win.to_glib_none().0)) }
}

// ---------------------------------------------------------------------------
// Wayland-specific state
// ---------------------------------------------------------------------------

/// Minimal dispatcher for getting Wayland globals.
/// All events are silently ignored — we only need the request/bind half.
struct WlGlobals;

impl Dispatch<WlRegistry, GlobalListContents> for WlGlobals {
    fn event(
        _state: &mut Self,
        _registry: &WlRegistry,
        _event: wayland_client::protocol::wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Handled internally by GlobalListContents
    }
}

wayland_client::delegate_noop!(WlGlobals: ignore WlCompositor);
wayland_client::delegate_noop!(WlGlobals: ignore WlSubcompositor);
wayland_client::delegate_noop!(WlGlobals: ignore WlSurface);
wayland_client::delegate_noop!(WlGlobals: ignore WlSubsurface);

/// Holds all Wayland-specific resources for the renderer. Stored as an
/// `Option` in `LinuxGlRenderer`; `None` on X11 sessions.
///
/// Field declaration order determines drop order. The EGL window and subsurface
/// must be destroyed before the surface they reference, and all protocol objects
/// before the connection/queue that owns them.
struct WaylandState {
    /// wl_egl_window wrapper — dropped first so libEGL releases the surface buffer.
    wl_egl_surface: WlEglSurface,
    /// Subsurface controller — dropped before the child surface it references.
    subsurface: WlSubsurface,
    /// The child wl_surface that mpv renders into.
    child_surface: WlSurface,
    /// Event queue — must be periodically dispatched to drain compositor events
    /// (buffer release, surface enter/leave) and avoid protocol errors.
    queue: EventQueue<WlGlobals>,
    /// Keeps the Wayland connection alive (owns the fd reference). Dropped last.
    conn: Connection,
    /// Last known frame rect so we can restore position after un-hide.
    last_frame: (i32, i32, i32, i32),
}

// WlEglSurface wraps a *mut wl_egl_window which is not Send by default.
// Safety: we only access it from the glib main thread (set_frame / set_visible / detach).
unsafe impl Send for WaylandState {}
unsafe impl Sync for WaylandState {}

// ---------------------------------------------------------------------------
// LinuxGlRenderer
// ---------------------------------------------------------------------------

/// Embeds libmpv video inside the Tauri window using EGL/OpenGL.
///
/// On X11 sessions: uses an X11 child window (XCreateWindow) as the EGL surface.
/// On Wayland sessions: uses a wl_subsurface + wl_egl_window as the EGL surface.
///
/// Raw pointer fields and EGL/X11 calls are only touched via glib main-thread
/// dispatch, which provides the same safety guarantee as macOS's main-thread queue.
pub struct LinuxGlRenderer {
    egl_display: egl::Display,
    egl_surface: egl::Surface,
    egl_context: egl::Context,
    egl_config: egl::Config,

    // X11-specific — None on Wayland sessions
    x11_display: Option<*mut c_void>,
    x11_child_window: u64,
    x11_parent_window: u64,
    x11_colormap: u64,
    xlib: Option<x11_dl::xlib::Xlib>,
    /// Whether we opened the X11 display ourselves (XCB path) and must close it.
    owns_display: bool,

    // Wayland-specific — None on X11 sessions
    wayland: Option<WaylandState>,

    /// Guards against double-cleanup of EGL resources in detach().
    egl_cleaned_up: bool,

    valid: Arc<AtomicBool>,
    /// Heap-stable render state. Box address is captured in the update callback.
    render_inner: Option<Box<RenderInner>>,
    /// Called once on first rendered frame; moved into RenderInner during attach().
    first_frame_cb: Option<Box<dyn FnOnce() + Send>>,
    /// Controls whether GPU rendering happens. Toggled by set_visible().
    video_active: Arc<AtomicBool>,
    /// Shared with RenderInner so set_frame can queue resizes for the GLib thread.
    pending_resize: Arc<Mutex<Option<PendingResize>>>,
    /// CSD offsets in pixels: (x, y) from the parent wl_surface origin to the
    /// WebView content area origin. Includes shadow margin + header bar height.
    /// Added to subsurface coordinates in set_frame() because frontend coords
    /// (getBoundingClientRect) are relative to the WebView viewport, but
    /// set_position() is relative to the parent wl_surface.
    csd_offset: (i32, i32),

    /// X11 only: raises all GTK-managed GdkWindows (including WebKit's rendering
    /// surface) above the raw Xlib video child, from the GLib main thread.
    ///
    /// Background: GDK owns X11 stacking for GTK windows. XLowerWindow called
    /// from the Tauri command thread races with GTK's own GLib-thread event
    /// processing and can be silently undone. Also, XMapWindow in set_visible(true)
    /// re-stacks the video child to the top with no compensating lower. Dispatching
    /// via the GLib main thread serialises with GTK and reliably keeps WebKit above
    /// the video, making React controls visible on top of embedded video.
    x11_webkit_raise: Option<Box<dyn Fn() + Send + Sync>>,
}

unsafe impl Send for LinuxGlRenderer {}
unsafe impl Sync for LinuxGlRenderer {}

impl LinuxGlRenderer {
    /// Query the CSD (Client-Side Decoration) offset from the top-left of the
    /// parent wl_surface to the top-left of the WebView content area.
    ///
    /// On Wayland with CSD, the wl_surface includes:
    ///  1. Shadow/decoration margin (typically 10px Adwaita theme)
    ///  2. Header bar (typically ~47px)
    ///  3. Any container padding (0 for Tauri)
    ///
    /// We query the decoration margin from the GTK style context and the
    /// WebView offset via `translate_coordinates()`. Must dispatch to the
    /// GLib main thread because GTK widget APIs require it.
    ///
    /// Returns (x_offset, y_offset) in pixels; (0, 0) on X11 or failure.
    fn query_csd_offsets<R: Runtime>(app: &AppHandle<R>) -> (i32, i32) {
        use gtk::prelude::*;

        // Verify the window exists before dispatching to the GLib main thread.
        if app.get_webview_window("main").is_none() {
            return (0, 0);
        }

        let (tx, rx) = std::sync::mpsc::channel();
        let app_handle = app.clone();

        glib::idle_add_once(move || {
            let window = match app_handle.get_webview_window("main") {
                Some(w) => w,
                None => { let _ = tx.send((0, 0)); return; }
            };

            let result = (|| -> Option<(i32, i32)> {
                let gtk_win = window.gtk_window().ok()?;

                // Use the default_vbox (Tauri's content container that holds the
                // WebView) and translate_coordinates to get its exact position
                // within the GtkWindow. This accounts for:
                //  - CSD shadow margins (decoration CSS margin, ~10px Adwaita)
                //  - Header bar height (~47px)
                //  - Any container padding (0 for Tauri)
                let vbox = window.default_vbox().ok()?;
                let (vbox_x, vbox_y) = match vbox.translate_coordinates(&gtk_win, 0, 0) {
                    Some(coords) => coords,
                    None => {
                        // Fallback: header bar height only (no shadow info).
                        let header_h = gtk_win
                            .titlebar()
                            .map(|tb| tb.allocated_height())
                            .unwrap_or(0);
                        return Some((0, header_h));
                    }
                };

                // translate_coordinates gives the vbox position within the
                // GtkWindow widget's allocation. On GTK3/CSD, the GdkWindow
                // backing the GtkWindow includes the shadow area, but the
                // GtkWindow widget is allocated inside (after) the shadow.
                // So vbox_y includes the header bar offset but NOT the shadow.
                //
                // Get the shadow margin from GdkWindow size vs GtkWidget allocation.
                let gdk_win = gtk_win.window()?;
                let (gdk_w, gdk_h) = (gdk_win.width(), gdk_win.height());
                let alloc = gtk_win.allocation();
                // Shadow = (GdkWindow size - GtkWidget allocation) / 2 on each side
                let shadow_x = (gdk_w - alloc.width()).max(0) / 2;
                let shadow_y = (gdk_h - alloc.height()).max(0) / 2;

                tracing::info!(
                    "[Linux renderer] CSD breakdown: vbox_translate=({},{}) gdk={}x{} alloc={}x{} shadow=({},{}) total=({},{})",
                    vbox_x, vbox_y, gdk_w, gdk_h, alloc.width(), alloc.height(),
                    shadow_x, shadow_y,
                    shadow_x + vbox_x, shadow_y + vbox_y
                );
                Some((shadow_x + vbox_x, shadow_y + vbox_y))
            })();

            let _ = tx.send(result.unwrap_or((0, 0)));
        });

        match rx.recv_timeout(std::time::Duration::from_secs(2)) {
            Ok(offsets) => {
                tracing::info!(
                    "[Linux renderer] CSD offsets: x={}px y={}px (shadow + header bar)",
                    offsets.0, offsets.1
                );
                offsets
            }
            Err(_) => {
                tracing::warn!("[Linux renderer] timed out querying CSD offsets, defaulting to (0,0)");
                (0, 0)
            }
        }
    }

    /// Create an EGL-backed renderer within the Tauri window.
    /// Dispatches to the X11 or Wayland path based on the raw window handle.
    pub fn new<R: Runtime>(app: &AppHandle<R>) -> Result<Self, String> {
        // Allow users to force fallback rendering via environment variable.
        if std::env::var("MVP_DISABLE_EMBEDDED_RENDERER").unwrap_or_default() == "1" {
            return Err("Embedded renderer disabled via MVP_DISABLE_EMBEDDED_RENDERER=1".to_string());
        }

        tracing::info!(
            "[Linux renderer] initializing (bundled_build={}, APPIMAGE={}, exe={:?})",
            is_bundled_build(),
            std::env::var("APPIMAGE").unwrap_or_default(),
            std::env::current_exe().unwrap_or_default(),
        );

        // Dump GPU-related shared libraries from /proc/self/maps so we can
        // see exactly which Mesa/EGL/GL libs are loaded and from where.
        Self::log_gpu_library_map();

        // Verify EGL is available before doing any work. This returns Err
        // instead of panicking, so the caller can fall back gracefully.
        try_load_egl().map_err(|e| {
            tracing::error!("[Linux renderer] {}", e);
            e
        })?;

        let csd_offset = Self::query_csd_offsets(app);

        let window = app
            .get_webview_window("main")
            .ok_or_else(|| "Window 'main' not found".to_string())?;

        let raw_window = window
            .window_handle()
            .map_err(|e| format!("window handle: {:?}", e))?
            .as_raw();

        let raw_display = window
            .display_handle()
            .map_err(|e| format!("display handle: {:?}", e))?
            .as_raw();

        let mut renderer = match raw_window {
            RawWindowHandle::Xlib(h) => {
                let parent_window = h.window;
                let x11_display_ptr = match raw_display {
                    RawDisplayHandle::Xlib(dh) => {
                        dh.display.map(|d| d.as_ptr()).unwrap_or(std::ptr::null_mut())
                    }
                    _ => std::ptr::null_mut(),
                };
                if x11_display_ptr.is_null() {
                    return Err("X11 display pointer is null".to_string());
                }
                Self::build_x11(parent_window, x11_display_ptr)
            }
            RawWindowHandle::Xcb(h) => {
                // XCB handle — open our own Xlib connection for XCreateWindow etc.
                let parent_window = h.window.get() as u64;
                let xlib = x11_dl::xlib::Xlib::open()
                    .map_err(|e| format!("Failed to open Xlib: {}", e))?;
                let display = unsafe { (xlib.XOpenDisplay)(std::ptr::null()) };
                if display.is_null() {
                    return Err("XOpenDisplay returned null".to_string());
                }
                Self::build_x11_with_xlib(parent_window, display as *mut c_void, xlib, true)
            }
            RawWindowHandle::Wayland(wh) => {
                let wl_surface_ptr = wh.surface.as_ptr();
                let wl_display_ptr = match raw_display {
                    RawDisplayHandle::Wayland(dh) => dh.display.as_ptr(),
                    _ => {
                        return Err(
                            "Got Wayland window handle but non-Wayland display handle".to_string(),
                        )
                    }
                };
                Self::build_wayland(wl_surface_ptr, wl_display_ptr)
            }
            _ => Err(format!(
                "Unsupported window handle type on Linux: {:?}",
                raw_window
            )),
        }?;
        renderer.csd_offset = csd_offset;

        // X11 path: install a GLib-main-thread stacking callback that ensures
        // the raw Xlib video child stays BELOW all GTK/WebKit GdkWindows.
        //
        // The callback does two things atomically on the GLib main thread:
        //  1. XLowerWindow on the video child — pushes it to the bottom of
        //     the sibling stack (below all GDK-managed windows).
        //  2. gdk_window_raise on every GDK child — raises WebKit surfaces
        //     above any other siblings (belt-and-suspenders).
        //
        // Both operations run on the GLib thread to serialise with GTK's own
        // event processing, which can silently re-stack X11 windows in
        // response to ConfigureNotify, expose, or resize events.
        //
        // Fires at: construction, set_visible(true), set_frame, and
        // periodically from render_frame (every ~30 frames).
        if renderer.wayland.is_none() {
            let app_handle = app.clone();
            let child_window = renderer.x11_child_window;
            let parent_window = renderer.x11_parent_window;
            let display_usize = renderer.x11_display.map(|p| p as usize).unwrap_or(0);

            // One-time X11 window hierarchy diagnostic (runs once on GLib thread).
            {
                let app_diag = app.clone();
                glib::idle_add_once(move || {
                    Self::log_x11_window_hierarchy(display_usize, parent_window, child_window, &app_diag);
                });
            }

            let raise_cb: Box<dyn Fn() + Send + Sync> = Box::new(move || {
                use gtk::prelude::*;
                let app_clone = app_handle.clone();
                glib::idle_add_once(move || {
                    // 1. Lower the video child via raw X11.
                    if display_usize != 0 {
                        if let Some(xlib) = xlib_for_stacking() {
                            unsafe {
                                (xlib.0.XLowerWindow)(
                                    display_usize as *mut x11_dl::xlib::Display,
                                    child_window,
                                );
                                (xlib.0.XFlush)(
                                    display_usize as *mut x11_dl::xlib::Display,
                                );
                            }
                        }
                    }

                    // 2. Raise every GTK-managed GdkWindow child.
                    if let Some(win) = app_clone.get_webview_window("main") {
                        if let Ok(gtk_win) = win.gtk_window() {
                            if let Some(root_gdk) = gtk_win.window() {
                                let children = root_gdk.children();
                                for child in children {
                                    child.raise();
                                }
                            }
                        }
                    }
                });
            });
            raise_cb(); // initial raise right after construction
            renderer.x11_webkit_raise = Some(raise_cb);
        }

        // Check for blocklisted GPU drivers AFTER construction so that if we
        // return Err, the renderer is dropped and its Drop impl cleans up
        // EGL/X11/Wayland resources properly (no leaks).
        Self::check_gpu_blocklist(&renderer)?;

        Ok(renderer)
    }

    /// Check the GL renderer string against known-bad drivers that crash during
    /// mpv's OpenGL render pipeline (texture upload, shader dispatch). Simple GL
    /// operations (glClear, glSwapBuffers) pass on these drivers, but mpv rendering
    /// segfaults. Returns Err to trigger fallback to a separate mpv window.
    fn check_gpu_blocklist(renderer: &Self) -> Result<(), String> {
        if std::env::var("MVP_FORCE_EMBEDDED_RENDERER").unwrap_or_default() == "1" {
            tracing::info!("[Linux renderer] MVP_FORCE_EMBEDDED_RENDERER=1, skipping blocklist");
            return Ok(());
        }

        let egl = egl_instance();
        if egl.make_current(
            renderer.egl_display,
            Some(renderer.egl_surface),
            Some(renderer.egl_surface),
            Some(renderer.egl_context),
        ).is_err() {
            return Err("Cannot make EGL context current for blocklist check".into());
        }

        gl::load_with(|name| gl_get_proc_address(name) as *const _);
        let renderer_str = unsafe {
            let s = gl::GetString(gl::RENDERER);
            if s.is_null() { String::new() }
            else { std::ffi::CStr::from_ptr(s as *const _).to_string_lossy().into_owned() }
        };
        let _ = egl.make_current(renderer.egl_display, None, None, None);

        let renderer_lower = renderer_str.to_lowercase();
        let blocklist: &[(&str, &str)] = &[
            ("llvmpipe", "Software rasterizer (llvmpipe) -- too slow and unstable for embedded video"),
            ("swrast", "Software rasterizer (swrast) -- no GPU acceleration available"),
            ("softpipe", "Software rasterizer (softpipe) -- no GPU acceleration available"),
        ];
        for (pattern, reason) in blocklist {
            if renderer_lower.contains(pattern) {
                let msg = format!(
                    "GL renderer blocklisted for embedded rendering: {} ({}). \
                     Falling back to separate mpv window. \
                     Set MVP_FORCE_EMBEDDED_RENDERER=1 to override.",
                    renderer_str, reason
                );
                tracing::warn!("[Linux renderer] {}", msg);
                return Err(msg);
            }
        }
        Ok(())
    }

    /// Dump GPU-related .so paths from /proc/self/maps.
    /// Helps distinguish system vs bundled Mesa/EGL/driver libraries.
    fn log_gpu_library_map() {
        let Ok(maps) = std::fs::read_to_string("/proc/self/maps") else {
            tracing::debug!("[Linux renderer] cannot read /proc/self/maps");
            return;
        };
        let keywords = [
            "libEGL", "libGL", "libgbm", "libdrm", "mesa", "swrast",
            "dri/", "i965", "iris", "radeonsi", "nouveau", "vmwgfx",
            "svga", "gallium", "libvulkan", "libva", "libvdpau",
        ];
        let mut seen = std::collections::HashSet::new();
        for line in maps.lines() {
            // Only look at mapped .so files (lines containing "/").
            let Some(path_start) = line.rfind('/') else { continue };
            let path = &line[path_start..];
            let lower = path.to_lowercase();
            if keywords.iter().any(|kw| lower.contains(&kw.to_lowercase())) {
                if seen.insert(path.to_string()) {
                    tracing::info!("[GPU lib] {path}");
                }
            }
        }
    }

    /// Diagnostic: dump the X11 window tree under the parent to understand
    /// why WebKit controls may be hidden behind the video child.
    fn log_x11_window_hierarchy<R: tauri::Runtime>(
        display_usize: usize,
        parent_window: u64,
        video_child: u64,
        app: &tauri::AppHandle<R>,
    ) {
        use gtk::prelude::*;

        if display_usize == 0 || parent_window == 0 {
            tracing::warn!("[X11 diag] no display/parent — skipping hierarchy dump");
            return;
        }

        let xlib = match x11_dl::xlib::Xlib::open() {
            Ok(x) => x,
            Err(e) => {
                tracing::warn!("[X11 diag] cannot open Xlib: {e}");
                return;
            }
        };

        let dpy = display_usize as *mut x11_dl::xlib::Display;

        // Query children of the parent window.
        let mut root_ret: u64 = 0;
        let mut parent_ret: u64 = 0;
        let mut children_ptr: *mut u64 = std::ptr::null_mut();
        let mut nchildren: u32 = 0;
        let ok = unsafe {
            (xlib.XQueryTree)(
                dpy,
                parent_window,
                &mut root_ret,
                &mut parent_ret,
                &mut children_ptr,
                &mut nchildren,
            )
        };
        if ok == 0 {
            tracing::warn!("[X11 diag] XQueryTree failed for parent 0x{parent_window:x}");
            return;
        }

        tracing::info!(
            "[X11 diag] parent=0x{:x} has {} children (root=0x{:x})",
            parent_window, nchildren, root_ret
        );

        let children = if children_ptr.is_null() || nchildren == 0 {
            Vec::new()
        } else {
            let slice = unsafe { std::slice::from_raw_parts(children_ptr, nchildren as usize) };
            let v = slice.to_vec();
            unsafe { (xlib.XFree)(children_ptr as *mut _) };
            v
        };

        // Children are in bottom-to-top stacking order.
        for (i, &win) in children.iter().enumerate() {
            let mut attrs: x11_dl::xlib::XWindowAttributes = unsafe { std::mem::zeroed() };
            unsafe { (xlib.XGetWindowAttributes)(dpy, win, &mut attrs) };

            let is_video = win == video_child;
            let mut label = if is_video {
                "VIDEO-CHILD".to_string()
            } else {
                "unknown".to_string()
            };

            // Check if this is a GDK window.
            if let Some(main_win) = app.get_webview_window("main") {
                if let Ok(gtk_win) = main_win.gtk_window() {
                    if let Some(root_gdk) = gtk_win.window() {
                        for gdk_child in root_gdk.children() {
                            if let Some(xid) = gdk_x11_window_xid(&gdk_child) {
                                if xid == win {
                                    label = format!("GDK (type={:?})", gdk_child.window_type());
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            tracing::info!(
                "[X11 diag]   [{i}] xid=0x{win:x} {w}x{h}+{x}+{y} map={map} depth={depth} class={cls} — {label}",
                w = attrs.width,
                h = attrs.height,
                x = attrs.x,
                y = attrs.y,
                map = attrs.map_state,
                depth = attrs.depth,
                cls = attrs.class,
            );
        }

        // Check if a compositor is active (RGBA blending between siblings).
        let comp_atom_name = std::ffi::CString::new(format!(
            "_NET_WM_CM_S{}",
            unsafe { (xlib.XDefaultScreen)(dpy) }
        ))
        .unwrap();
        let comp_atom =
            unsafe { (xlib.XInternAtom)(dpy, comp_atom_name.as_ptr(), 0) };
        let comp_owner = unsafe { (xlib.XGetSelectionOwner)(dpy, comp_atom) };
        tracing::info!(
            "[X11 diag] compositor active: {} (owner=0x{:x})",
            comp_owner != 0,
            comp_owner
        );
    }

    // -----------------------------------------------------------------------
    // X11 construction path (unchanged from original)
    // -----------------------------------------------------------------------

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
        // Enable Xlib thread safety. Tauri command handlers run on a thread pool,
        // so set_frame/set_visible may race with GTK's own X11 usage without this.
        // Safe to call multiple times — subsequent calls are no-ops.
        unsafe { (xlib.XInitThreads)() };

        let egl = egl_instance();

        let egl_display = unsafe { egl.get_display(x11_display_ptr) }
            .ok_or("eglGetDisplay failed")?;

        egl.initialize(egl_display)
            .map_err(|e| format!("eglInitialize: {:?}", e))?;

        let config_attribs = [
            egl::RED_SIZE,
            8,
            egl::GREEN_SIZE,
            8,
            egl::BLUE_SIZE,
            8,
            egl::ALPHA_SIZE,
            8,
            egl::DEPTH_SIZE,
            0,
            egl::STENCIL_SIZE,
            0,
            egl::RENDERABLE_TYPE,
            egl::OPENGL_BIT,
            egl::SURFACE_TYPE,
            egl::WINDOW_BIT,
            egl::NONE,
        ];

        let config = egl
            .choose_first_config(egl_display, &config_attribs)
            .map_err(|e| format!("eglChooseConfig: {:?}", e))?
            .ok_or("No suitable EGL config found")?;

        egl.bind_api(egl::OPENGL_API)
            .map_err(|e| format!("eglBindApi(OPENGL_API): {:?}", e))?;

        let x_display = x11_display_ptr as *mut x11_dl::xlib::Display;

        let native_visual_id: i32 = egl
            .get_config_attrib(egl_display, config, egl::NATIVE_VISUAL_ID)
            .map_err(|e| format!("eglGetConfigAttrib(NATIVE_VISUAL_ID): {:?}", e))?;

        let (child_window, x11_colormap) = unsafe {
            let mut vi_template: x11_dl::xlib::XVisualInfo = std::mem::zeroed();
            vi_template.visualid = native_visual_id as u64;
            let mut nitems: i32 = 0;
            let vi_ptr = (xlib.XGetVisualInfo)(
                x_display,
                x11_dl::xlib::VisualIDMask,
                &mut vi_template,
                &mut nitems,
            );
            if vi_ptr.is_null() || nitems < 1 {
                return Err(format!(
                    "XGetVisualInfo failed for visual ID {}",
                    native_visual_id
                ));
            }
            let vi = *vi_ptr;
            (xlib.XFree)(vi_ptr as *mut c_void);

            let colormap = (xlib.XCreateColormap)(x_display, parent_window, vi.visual, 0);

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
                &mut x_ret,
                &mut y_ret,
                &mut w_ret,
                &mut h_ret,
                &mut border_ret,
                &mut depth_ret,
            );

            let mut attrs: x11_dl::xlib::XSetWindowAttributes = std::mem::zeroed();
            attrs.colormap = colormap;
            attrs.background_pixel = 0;
            attrs.border_pixel = 0;
            attrs.event_mask = 0;

            let child = (xlib.XCreateWindow)(
                x_display,
                parent_window,
                0,
                0,
                w_ret.max(1),
                h_ret.max(1),
                0,
                vi.depth,
                1,
                vi.visual,
                0x0002 | 0x0008 | 0x0800 | 0x2000,
                &mut attrs,
            );

            if child == 0 {
                return Err("XCreateWindow failed".to_string());
            }

            (xlib.XMapWindow)(x_display, child);
            (xlib.XLowerWindow)(x_display, child);
            (xlib.XFlush)(x_display);

            (child, colormap)
        };

        let egl_surface = unsafe {
            egl.create_window_surface(
                egl_display,
                config,
                child_window as egl::NativeWindowType,
                None,
            )
        }
        .map_err(|e| format!("eglCreateWindowSurface: {:?}", e))?;

        let context_attribs = [
            egl::CONTEXT_MAJOR_VERSION,
            3,
            egl::CONTEXT_MINOR_VERSION,
            2,
            egl::CONTEXT_OPENGL_PROFILE_MASK,
            egl::CONTEXT_OPENGL_CORE_PROFILE_BIT,
            egl::NONE,
        ];

        let egl_context = egl
            .create_context(egl_display, config, None, &context_attribs)
            .map_err(|e| format!("eglCreateContext: {:?}", e))?;

        // Log EGL/GL diagnostics to help debug driver issues.
        // Blocklist check is deferred to new() so Drop cleans up EGL resources.
        Self::log_egl_diagnostics(egl, egl_display, egl_surface, egl_context, "X11")
            .unwrap_or_else(|e| tracing::warn!("[Linux renderer] X11 diagnostics issue: {}", e));

        tracing::info!(
            "[Linux renderer] X11 child window + EGL context created (OpenGL Core 3.2)"
        );

        Ok(Self {
            egl_display,
            egl_surface,
            egl_context,
            egl_config: config,
            x11_display: Some(x11_display_ptr),
            x11_child_window: child_window,
            x11_parent_window: parent_window,
            x11_colormap,
            xlib: Some(xlib),
            owns_display,
            wayland: None,
            egl_cleaned_up: false,
            valid: Arc::new(AtomicBool::new(true)),
            render_inner: None,
            first_frame_cb: None,
            video_active: Arc::new(AtomicBool::new(true)),
            pending_resize: Arc::new(Mutex::new(None)),
            csd_offset: (0, 0),
            x11_webkit_raise: None, // set by new() which has AppHandle
        })
    }

    // -----------------------------------------------------------------------
    // Wayland construction path
    // -----------------------------------------------------------------------

    fn build_wayland(
        parent_surface_ptr: *mut c_void,
        wl_display_ptr: *mut c_void,
    ) -> Result<Self, String> {
        let egl = egl_instance();

        // --- Wayland protocol setup ---
        //
        // We connect to the same wl_display that GTK/GDK is already using.
        // Backend::from_foreign_display creates a secondary reference to the
        // display fd — it can send protocol requests on the same namespace
        // without interfering with GTK's own event dispatch.
        let backend = unsafe {
            Backend::from_foreign_display(wl_display_ptr as *mut _)
        };
        let conn = Connection::from_backend(backend);

        let (globals, mut queue) = registry_queue_init::<WlGlobals>(&conn)
            .map_err(|e| format!("Wayland registry_queue_init: {}", e))?;
        let qh = queue.handle();

        let compositor: WlCompositor = globals
            .bind(&qh, 4..=5, ())
            .map_err(|e| format!("Wayland: bind wl_compositor: {}", e))?;
        let subcompositor: WlSubcompositor = globals
            .bind(&qh, 1..=1, ())
            .map_err(|e| format!("Wayland: bind wl_subcompositor: {}", e))?;

        let mut state = WlGlobals;
        queue
            .roundtrip(&mut state)
            .map_err(|e| format!("Wayland roundtrip: {}", e))?;

        // Wrap the parent surface pointer (owned by GTK/GDK) as a Rust proxy
        // so we can pass it to get_subsurface().
        //
        // Safety rationale for cross-connection wrapping:
        // - `from_foreign_display` shares the same fd and server-side object namespace
        //   as GTK's connection — object IDs are valid across both.
        // - `ObjectId::from_ptr` extracts the ID from the C-level wl_proxy; the server
        //   recognizes it because it's the same connection underneath.
        // - Dropping this `WlSurface` proxy does NOT send `wl_surface_destroy` — it
        //   only releases the Rust wrapper. GTK retains ownership of the actual surface.
        // - We only use `parent_surface` as an argument to `get_subsurface()`, never to
        //   receive events or manage its lifetime.
        let parent_id = unsafe {
            ObjectId::from_ptr(WlSurface::interface(), parent_surface_ptr as *mut _)
        }
        .map_err(|_| "Wayland: invalid parent wl_surface pointer from window handle")?;
        let parent_surface = WlSurface::from_id(&conn, parent_id)
            .map_err(|_| "Wayland: cannot create proxy for parent surface")?;

        // Create the child surface that mpv renders into.
        let child_surface = compositor.create_surface(&qh, ());

        // Create a subsurface — this places child_surface as a child of the
        // parent (Tauri's WKView equivalent on Wayland).
        let subsurface =
            subcompositor.get_subsurface(&child_surface, &parent_surface, &qh, ());

        // Place the video subsurface BELOW the parent surface (WebView).
        // Without this, the subsurface defaults to above the parent, covering
        // the transparent WebView controls. This mirrors macOS's
        // addSubview:positioned:NSWindowBelow:relativeTo: pattern.
        subsurface.place_below(&parent_surface);

        // Desync: the subsurface can be committed independently of the parent.
        // This is the correct mode for video — we don't want to wait for GTK's
        // frame cycle before presenting each decoded frame.
        subsurface.set_desync();

        // Commit the child surface to apply the desync state.
        child_surface.commit();
        conn.flush().map_err(|e| format!("Wayland flush: {}", e))?;

        // --- EGL setup on Wayland ---
        //
        // Prefer eglGetPlatformDisplayEXT(EGL_PLATFORM_WAYLAND_EXT, ...) for
        // spec-correct Wayland EGL. Fall back to eglGetDisplay which Mesa
        // also auto-detects as Wayland when given a wl_display*.
        const EGL_PLATFORM_WAYLAND_EXT: u32 = 0x31D8;
        let egl_display = Self::get_wayland_egl_display(egl, wl_display_ptr, EGL_PLATFORM_WAYLAND_EXT)?;

        // On Wayland, GTK/GDK may share the same EGLDisplay. eglInitialize is
        // idempotent per spec (bumps refcount), but some Mesa driver versions
        // crash on double-init of a shared display. Check if already initialized
        // by querying EGL_VERSION; only initialize if not yet done.
        match egl.query_string(Some(egl_display), egl::VERSION) {
            Ok(ver) => {
                tracing::debug!(
                    "[Linux renderer] EGL display already initialized (version {:?}), skipping eglInitialize",
                    ver
                );
            }
            Err(e) => {
                tracing::debug!(
                    "[Linux renderer] EGL display not yet initialized ({:?}), calling eglInitialize",
                    e
                );
                egl.initialize(egl_display)
                    .map_err(|e| format!("Wayland eglInitialize: {:?}", e))?;
            }
        }

        let config_attribs = [
            egl::RED_SIZE,
            8,
            egl::GREEN_SIZE,
            8,
            egl::BLUE_SIZE,
            8,
            egl::ALPHA_SIZE,
            8,
            egl::DEPTH_SIZE,
            0,
            egl::STENCIL_SIZE,
            0,
            egl::RENDERABLE_TYPE,
            egl::OPENGL_BIT,
            egl::SURFACE_TYPE,
            egl::WINDOW_BIT,
            egl::NONE,
        ];

        let config = egl
            .choose_first_config(egl_display, &config_attribs)
            .map_err(|e| format!("Wayland eglChooseConfig: {:?}", e))?
            .ok_or("Wayland: no suitable EGL config found")?;

        egl.bind_api(egl::OPENGL_API)
            .map_err(|e| format!("Wayland eglBindApi(OPENGL_API): {:?}", e))?;

        // Create wl_egl_window — this is the EGL-side handle to our wl_surface.
        // Initial size of 1×1; actual size set by the first set_frame() call.
        let wl_egl_surface = WlEglSurface::new(child_surface.id(), 1, 1)
            .map_err(|e| format!("Wayland: wl_egl_window_create failed: {:?}", e))?;

        let egl_surface = unsafe {
            egl.create_window_surface(
                egl_display,
                config,
                wl_egl_surface.ptr() as egl::NativeWindowType,
                None,
            )
        }
        .map_err(|e| format!("Wayland eglCreateWindowSurface: {:?}", e))?;

        let context_attribs = [
            egl::CONTEXT_MAJOR_VERSION,
            3,
            egl::CONTEXT_MINOR_VERSION,
            2,
            egl::CONTEXT_OPENGL_PROFILE_MASK,
            egl::CONTEXT_OPENGL_CORE_PROFILE_BIT,
            egl::NONE,
        ];

        let egl_context = egl
            .create_context(egl_display, config, None, &context_attribs)
            .map_err(|e| format!("Wayland eglCreateContext: {:?}", e))?;

        // Log EGL/GL diagnostics to help debug driver issues.
        // Blocklist check is deferred to new() so Drop cleans up EGL resources.
        Self::log_egl_diagnostics(egl, egl_display, egl_surface, egl_context, "Wayland")
            .unwrap_or_else(|e| tracing::warn!("[Linux renderer] Wayland diagnostics issue: {}", e));

        tracing::info!(
            "[Linux renderer] Wayland subsurface + wl_egl_window + EGL context created (OpenGL Core 3.2)"
        );

        let wayland = WaylandState {
            wl_egl_surface,
            subsurface,
            child_surface,
            queue,
            conn,
            last_frame: (0, 0, 1, 1),
        };

        Ok(Self {
            egl_display,
            egl_surface,
            egl_context,
            egl_config: config,
            // X11 fields unused on Wayland
            x11_display: None,
            x11_child_window: 0,
            x11_parent_window: 0,
            x11_colormap: 0,
            xlib: None,
            owns_display: false,
            wayland: Some(wayland),
            egl_cleaned_up: false,
            valid: Arc::new(AtomicBool::new(true)),
            render_inner: None,
            first_frame_cb: None,
            video_active: Arc::new(AtomicBool::new(true)),
            pending_resize: Arc::new(Mutex::new(None)),
            csd_offset: (0, 0),
            x11_webkit_raise: None,
        })
    }

    /// Try eglGetPlatformDisplayEXT first (spec-correct for Wayland EGL), then
    /// fall back to plain eglGetDisplay (Mesa auto-detects Wayland).
    fn get_wayland_egl_display(
        egl: &egl::DynamicInstance<egl::EGL1_4>,
        wl_display_ptr: *mut c_void,
        platform_enum: u32,
    ) -> Result<egl::Display, String> {
        // The C return type EGLDisplay is void* — use *mut c_void then transmute
        // into egl::Display (a transparent newtype around the same pointer).
        type GetPlatformDisplayEXT =
            unsafe extern "C" fn(u32, *mut c_void, *const i32) -> *mut c_void;

        {
            if let Some(get_fn) = egl.get_proc_address("eglGetPlatformDisplayEXT") {
                let get_platform_display: GetPlatformDisplayEXT =
                    unsafe { std::mem::transmute(get_fn) };
                let d_ptr = unsafe {
                    get_platform_display(platform_enum, wl_display_ptr, std::ptr::null())
                };
                if !d_ptr.is_null() {
                    // SAFETY: egl::Display is a repr(transparent) wrapper over *mut c_void.
                    let d: egl::Display = unsafe { std::mem::transmute(d_ptr) };
                    tracing::debug!(
                        "[Linux renderer] Wayland EGL display via eglGetPlatformDisplayEXT"
                    );
                    return Ok(d);
                }
            }
        }

        // Fall back to eglGetDisplay — Mesa interprets a wl_display* correctly.
        tracing::debug!(
            "[Linux renderer] Wayland EGL: falling back to eglGetDisplay"
        );
        unsafe { egl.get_display(wl_display_ptr) }
            .ok_or_else(|| "Wayland: eglGetDisplay returned NO_DISPLAY".to_string())
    }

    /// Log EGL vendor/version and GL renderer info for diagnostics.
    /// Makes the context current temporarily, queries strings, then releases.
    ///
    /// Returns `Err` if making the EGL context current fails or if the GL
    /// renderer is a known software rasterizer (e.g. llvmpipe, swrast).
    /// The caller should bail to fallback in that case.
    fn log_egl_diagnostics(
        egl: &egl::DynamicInstance<egl::EGL1_4>,
        display: egl::Display,
        surface: egl::Surface,
        context: egl::Context,
        backend: &str,
    ) -> Result<(), String> {
        let egl_vendor = egl.query_string(Some(display), egl::VENDOR)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "unknown".into());
        let egl_version = egl.query_string(Some(display), egl::VERSION)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "unknown".into());
        let egl_apis = egl.query_string(Some(display), egl::CLIENT_APIS)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "unknown".into());

        tracing::info!(
            "[Linux renderer] EGL info ({}): vendor={} version={} apis={}",
            backend, egl_vendor, egl_version, egl_apis
        );

        // Temporarily make context current to query GL strings.
        if egl.make_current(display, Some(surface), Some(surface), Some(context)).is_ok() {
            gl::load_with(|name| gl_get_proc_address(name) as *const _);
            let renderer_str;
            let version_str;
            let vendor_str;
            unsafe {
                let gl_renderer = gl::GetString(gl::RENDERER);
                let gl_version = gl::GetString(gl::VERSION);
                let gl_vendor = gl::GetString(gl::VENDOR);

                renderer_str = if !gl_renderer.is_null() {
                    std::ffi::CStr::from_ptr(gl_renderer as *const _).to_string_lossy().into_owned()
                } else {
                    "null".into()
                };
                version_str = if !gl_version.is_null() {
                    std::ffi::CStr::from_ptr(gl_version as *const _).to_string_lossy().into_owned()
                } else {
                    "null".into()
                };
                vendor_str = if !gl_vendor.is_null() {
                    std::ffi::CStr::from_ptr(gl_vendor as *const _).to_string_lossy().into_owned()
                } else {
                    "null".into()
                };

                tracing::info!(
                    "[Linux renderer] GL info ({}): renderer={} version={} vendor={}",
                    backend, renderer_str, version_str, vendor_str
                );

                // Check for GL errors after context setup — a non-zero error
                // here indicates the context is in a bad state.
                let err = gl::GetError();
                if err != gl::NO_ERROR {
                    tracing::warn!(
                        "[Linux renderer] GL error after context setup: 0x{:04X}",
                        err
                    );
                }
            }
            let _ = egl.make_current(display, None, None, None);
            Ok(())
        } else {
            let msg = format!(
                "Could not make context current for GL diagnostics ({})",
                backend
            );
            tracing::warn!("[Linux renderer] {}", msg);
            Err(msg)
        }
    }

    /// Set a callback that fires exactly once when the first video frame is rendered.
    pub fn set_first_frame_callback(&mut self, cb: Box<dyn FnOnce() + Send>) {
        self.first_frame_cb = Some(cb);
    }
}

// ---------------------------------------------------------------------------
// PlatformRenderer impl
// ---------------------------------------------------------------------------

impl PlatformRenderer for LinuxGlRenderer {
    fn attach(&mut self, mpv: &mut Mpv) -> Result<(), String> {
        reset_frame_counters();
        let egl = egl_instance();
        let is_wayland = self.wayland.is_some();

        // Get wl_egl_surface pointer for render_frame to apply pending resizes.
        let wl_egl_surface_ptr = self
            .wayland
            .as_ref()
            .map(|wl| &wl.wl_egl_surface as *const WlEglSurface as usize)
            .unwrap_or(0);

        // On Wayland, ALL EGL context operations must be serialized on the GLib
        // main thread. Wayland EGL implementations are not thread-safe for
        // cross-thread context migration (unlike X11 with XInitThreads). Making
        // the context current from the Tauri command thread races with GTK's
        // own EGL operations and subsequent render_frame callbacks.
        //
        // On X11, the existing pattern is safe because XInitThreads enables
        // cross-thread usage, and attach() releases the context before
        // render_frame can acquire it.
        let render_ctx = if is_wayland {
            // Cast Mpv pointer to usize for cross-thread dispatch (same pattern
            // as macOS renderer). Safety: mpv lives in MpvState behind a Mutex,
            // held by the caller for the duration of attach().
            let mpv_raw = mpv.ctx.as_ptr() as usize;
            let egl_display_usize = self.egl_display.as_ptr() as usize;
            let egl_surface_usize = self.egl_surface.as_ptr() as usize;
            let egl_context_usize = self.egl_context.as_ptr() as usize;

            // RenderContext doesn't impl Send, but we need to return it from
            // the GLib thread. Safety: same as RenderInner's `unsafe impl Send`.
            struct SendableCtx(RenderContext);
            unsafe impl Send for SendableCtx {}

            let setup_egl = move || -> Result<SendableCtx, String> {
                let egl = egl_instance();

                // SAFETY: reconstruct EGL handles from usize (same as detach()).
                let display: egl::Display =
                    unsafe { std::mem::transmute(egl_display_usize as *mut c_void) };
                let surface: egl::Surface =
                    unsafe { std::mem::transmute(egl_surface_usize as *mut c_void) };
                let context: egl::Context =
                    unsafe { std::mem::transmute(egl_context_usize as *mut c_void) };

                egl.make_current(display, Some(surface), Some(surface), Some(context))
                    .map_err(|e| format!("eglMakeCurrent (GLib thread): {:?}", e))?;

                gl::load_with(|name| gl_get_proc_address(name) as *const _);

                fn get_proc_address(_ctx: &*mut c_void, name: &str) -> *mut c_void {
                    gl_get_proc_address(name)
                }

                let render_ctx = RenderContext::new(
                    unsafe { &mut *(mpv_raw as *mut _) },
                    vec![
                        RenderParam::ApiType(RenderParamApiType::OpenGl),
                        RenderParam::InitParams(OpenGLInitParams {
                            get_proc_address,
                            ctx: std::ptr::null_mut(),
                        }),
                    ],
                )
                .map_err(|e| format!("mpv_render_context_create: {}", e))?;

                // Clear to black before the first mpv frame arrives.
                unsafe {
                    gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                    gl::Clear(gl::COLOR_BUFFER_BIT);
                }
                let swap_ok = egl.swap_buffers(display, surface);

                // GL validation probe: check for errors after initial render setup.
                // A failure here means the GL pipeline is broken and we should bail
                // to fallback rather than waiting for a crash in render_frame.
                let gl_err = unsafe { gl::GetError() };
                if gl_err != gl::NO_ERROR {
                    let msg = format!(
                        "GL error after initial clear (Wayland): 0x{:04X}",
                        gl_err
                    );
                    tracing::error!("[Linux renderer] {}", msg);
                    let _ = egl.make_current(display, None, None, None);
                    return Err(msg);
                }
                if swap_ok.is_err() {
                    let egl_err = egl.get_error();
                    let msg = format!(
                        "eglSwapBuffers failed during attach probe (Wayland): EGL error {:?}",
                        egl_err
                    );
                    tracing::error!("[Linux renderer] {}", msg);
                    let _ = egl.make_current(display, None, None, None);
                    return Err(msg);
                }

                tracing::info!("[Linux renderer] GL validation probe passed (Wayland)");

                // Release context so render_frame callbacks can acquire it.
                let _ = egl.make_current(display, None, None, None);

                Ok(SendableCtx(render_ctx))
            };

            // If we're already on the GLib main thread (e.g. during GTK startup),
            // run inline to avoid deadlocking on our own callback. Otherwise
            // dispatch via MainContext::invoke (higher priority than idle_add_once,
            // won't be starved by pending idle callbacks).
            let main_ctx = glib::MainContext::default();
            if main_ctx.is_owner() {
                setup_egl().map(|s| s.0)?
            } else {
                let (tx, rx) = std::sync::mpsc::channel::<Result<SendableCtx, String>>();
                main_ctx.invoke(move || {
                    let _ = tx.send(setup_egl());
                });
                rx.recv_timeout(std::time::Duration::from_secs(5))
                    .map_err(|_| {
                        "Timed out waiting for GLib main thread EGL setup".to_string()
                    })?
                    .map(|s| s.0)?
            }
        } else {
            // X11 path — safe to run on Tauri command thread (XInitThreads).
            egl.make_current(
                self.egl_display,
                Some(self.egl_surface),
                Some(self.egl_surface),
                Some(self.egl_context),
            )
            .map_err(|e| format!("eglMakeCurrent: {:?}", e))?;

            gl::load_with(|name| gl_get_proc_address(name) as *const _);

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

            // Clear the framebuffer to black before the first mpv frame arrives.
            unsafe {
                gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                gl::Clear(gl::COLOR_BUFFER_BIT);
            }
            let swap_ok = egl.swap_buffers(self.egl_display, self.egl_surface);

            // GL validation probe: check for errors after initial render setup.
            let gl_err = unsafe { gl::GetError() };
            if gl_err != gl::NO_ERROR {
                let msg = format!(
                    "GL error after initial clear (X11): 0x{:04X}",
                    gl_err
                );
                tracing::error!("[Linux renderer] {}", msg);
                let _ = egl.make_current(self.egl_display, None, None, None);
                return Err(msg);
            }
            if swap_ok.is_err() {
                let egl_err = egl.get_error();
                let msg = format!(
                    "eglSwapBuffers failed during attach probe (X11): EGL error {:?}",
                    egl_err
                );
                tracing::error!("[Linux renderer] {}", msg);
                let _ = egl.make_current(self.egl_display, None, None, None);
                return Err(msg);
            }

            tracing::info!("[Linux renderer] GL validation probe passed (X11)");

            // Release the EGL context from this thread so the GLib main thread
            // can make it current in render_frame().
            let _ = egl.make_current(self.egl_display, None, None, None);

            render_ctx
        };

        // Common path: set up RenderInner and update callback.
        let x11_display_ptr = self.x11_display.map(|p| p as usize).unwrap_or(0);
        let x11_child_window = self.x11_child_window;

        let mut inner = Box::new(RenderInner {
            ctx: render_ctx,
            egl_display: self.egl_display,
            egl_surface: self.egl_surface,
            egl_context: self.egl_context,
            first_frame_cb: self.first_frame_cb.take(),
            video_active: self.video_active.clone(),
            pending_resize: self.pending_resize.clone(),
            wl_egl_surface_ptr,
            x11_display_ptr,
            x11_child_window,
        });

        let inner_ptr = &*inner as *const RenderInner as usize;
        let valid = self.valid.clone();

        inner.ctx.set_update_callback(move || {
            let v = valid.clone();
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
        // No-op: set_frame() handles all positioning and sizing.
    }

    fn set_frame(&mut self, x: f64, y: f64, w: f64, h: f64) {
        tracing::trace!("[Linux renderer] set_frame({}, {}, {}, {})", x, y, w, h);
        if let Some(ref mut wl) = self.wayland {
            // Wayland: position the subsurface and queue resize for GLib thread.
            //
            // wl_egl_window_resize is NOT thread-safe with respect to EGL calls
            // (eglMakeCurrent, eglSwapBuffers). Instead of resizing here on the
            // command thread, we queue the resize and let render_frame() apply it
            // on the GLib main thread — the same thread that does EGL rendering.
            // This mirrors macOS's pattern of dispatching set_frame to main queue.
            let wi = (w as i32).max(1);
            let hi = (h as i32).max(1);

            // Frontend coords are relative to the WebView viewport, but
            // subsurface position is relative to the parent wl_surface which
            // includes CSD shadow margins and header bar. Add both offsets.
            let adjusted_x = x as i32 + self.csd_offset.0;
            let adjusted_y = y as i32 + self.csd_offset.1;
            wl.last_frame = (adjusted_x, adjusted_y, wi, hi);

            // set_position is double-buffered (safe from any thread).
            wl.subsurface.set_position(adjusted_x, adjusted_y);

            // Queue the resize for the GLib main thread.
            if let Ok(mut pending) = self.pending_resize.lock() {
                *pending = Some(PendingResize { w: wi, h: hi });
            }

            // Manually trigger a render_frame dispatch. When paused, mpv doesn't
            // fire update callbacks, so the pending resize would never be applied.
            // This ensures resize is visible immediately (redraws last frame).
            if let Some(ref inner) = self.render_inner {
                let inner_ptr = &**inner as *const RenderInner as usize;
                let v = self.valid.clone();
                glib::idle_add_once(move || {
                    if !v.load(Ordering::Acquire) {
                        return;
                    }
                    unsafe { render_frame(inner_ptr) };
                });
            }

            wl.child_surface.commit();
            let _ = wl.conn.flush();

            // Drain pending compositor events (buffer release, surface enter/leave)
            // to keep the protocol state consistent and prevent queue overflow.
            let mut dummy = WlGlobals;
            if let Err(e) = wl.queue.dispatch_pending(&mut dummy) {
                tracing::warn!("[Linux renderer] Wayland dispatch_pending failed: {}", e);
            }
        } else if let (Some(ref xlib), Some(display)) = (&self.xlib, self.x11_display) {
            let x_display = display as *mut x11_dl::xlib::Display;
            let adjusted_x = x as i32 + self.csd_offset.0;
            let adjusted_y = y as i32 + self.csd_offset.1;
            tracing::trace!(
                "[Linux renderer] X11 set_frame: raw=({},{}) csd_offset=({},{}) adjusted=({},{})",
                x as i32, y as i32, self.csd_offset.0, self.csd_offset.1, adjusted_x, adjusted_y
            );
            unsafe {
                (xlib.XMoveResizeWindow)(
                    x_display,
                    self.x11_child_window,
                    adjusted_x,
                    adjusted_y,
                    (w as u32).max(1),
                    (h as u32).max(1),
                );
                (xlib.XFlush)(x_display);
            }
            // XMoveResizeWindow generates ConfigureNotify which may trigger GTK
            // to restack its GdkWindows. Re-enforce video-below-WebKit ordering.
            if let Some(ref raise) = self.x11_webkit_raise {
                raise();
            }
        }

    }

    fn set_visible(&mut self, visible: bool) {
        self.video_active.store(visible, Ordering::Release);

        if let Some(ref mut wl) = self.wayland {
            if visible {
                // Restore last known position and queue resize for GLib thread.
                let (lx, ly, lw, lh) = wl.last_frame;
                wl.subsurface.set_position(lx, ly);
                if let Ok(mut pending) = self.pending_resize.lock() {
                    *pending = Some(PendingResize { w: lw, h: lh });
                }
            } else {
                // Move subsurface far off-screen. This is the safe Wayland equivalent
                // of XUnmapWindow — we cannot call wl_surface_attach(NULL) on an
                // EGL-managed surface without risking protocol errors.
                wl.subsurface.set_position(-32000, -32000);
            }
            wl.child_surface.commit();
            let _ = wl.conn.flush();

            // Drain pending compositor events.
            let mut dummy = WlGlobals;
            if let Err(e) = wl.queue.dispatch_pending(&mut dummy) {
                tracing::warn!("[Linux renderer] Wayland dispatch_pending failed: {}", e);
            }
        } else if let (Some(ref xlib), Some(display)) = (&self.xlib, self.x11_display) {
            let x_display = display as *mut x11_dl::xlib::Display;
            unsafe {
                if visible {
                    (xlib.XMapWindow)(x_display, self.x11_child_window);
                    // XMapWindow always re-stacks the window at the top of its siblings.
                    // Lower it again immediately, then let the GLib-thread callback
                    // raise the WebKit GdkWindow to restore correct stacking.
                    (xlib.XLowerWindow)(x_display, self.x11_child_window);
                } else {
                    (xlib.XUnmapWindow)(x_display, self.x11_child_window);
                }
                (xlib.XFlush)(x_display);
            }
            if visible {
                if let Some(ref raise) = self.x11_webkit_raise {
                    raise();
                }
            }
        }
    }

    fn detach(&mut self) {
        // Signal all queued callbacks to bail before we free the render state.
        self.valid.store(false, Ordering::Release);

        let egl = egl_instance();

        // Drop the RenderContext with GL context current. If we're already on
        // the GLib main thread (e.g. Drop triggered during GTK teardown), run
        // cleanup inline to avoid deadlocking on our own idle callback.
        if let Some(render_inner) = self.render_inner.take() {
            // egl::Display/Surface/Context are newtype wrappers around *mut c_void
            // and don't implement Send. Transmute to usize for cross-thread dispatch
            // (same pattern as macOS raw pointer dispatch).
            let display_usize = self.egl_display.as_ptr() as usize;
            let surface_usize = self.egl_surface.as_ptr() as usize;
            let context_usize = self.egl_context.as_ptr() as usize;

            let do_drop = move |ri: Box<RenderInner>| {
                let egl = egl_instance();
                let display: egl::Display = unsafe { std::mem::transmute(display_usize as *mut c_void) };
                let surface: egl::Surface = unsafe { std::mem::transmute(surface_usize as *mut c_void) };
                let context: egl::Context = unsafe { std::mem::transmute(context_usize as *mut c_void) };
                let _ = egl.make_current(display, Some(surface), Some(surface), Some(context));
                drop(ri);
                let _ = egl.make_current(display, None, None, None);
            };

            if glib::MainContext::default().is_owner() {
                // Already on the main thread — run cleanup directly.
                do_drop(render_inner);
            } else {
                // Schedule on the GLib main thread and block until drained.
                let (tx, rx) = std::sync::mpsc::channel::<()>();
                glib::idle_add_once(move || {
                    do_drop(render_inner);
                    let _ = tx.send(());
                });
                if rx
                    .recv_timeout(std::time::Duration::from_secs(2))
                    .is_err()
                {
                    tracing::warn!("[Linux renderer] detach: timed out waiting for GLib idle drain");
                }
            }
        }

        // Clean up EGL resources (guarded by flag to prevent double-cleanup).
        if !self.egl_cleaned_up {
            self.egl_cleaned_up = true;
            let _ = egl.make_current(self.egl_display, None, None, None);
            let _ = egl.destroy_surface(self.egl_display, self.egl_surface);
            let _ = egl.destroy_context(self.egl_display, self.egl_context);
            // Only terminate the EGL display on X11 where we own it.
            // On Wayland, GTK shares the same EGLDisplay — terminating it
            // would crash the application.
            if self.wayland.is_none() {
                let _ = egl.terminate(self.egl_display);
            }
        }

        // Wayland cleanup: drop subsurface and surface (in correct order),
        // then let WaylandState drop the connection and event queue.
        if let Some(ref mut wl) = self.wayland {
            wl.subsurface.destroy();
            // Commit to make the compositor release the subsurface relationship.
            wl.child_surface.commit();
            let _ = wl.conn.flush();
            // wl_egl_surface, child_surface, _queue, conn drop here in field order.
        }
        self.wayland = None;

        // X11 cleanup.
        if let (Some(ref xlib), Some(display)) = (&self.xlib, self.x11_display) {
            let x_display = display as *mut x11_dl::xlib::Display;
            if self.x11_child_window != 0 {
                unsafe {
                    (xlib.XDestroyWindow)(x_display, self.x11_child_window);
                }
            }
            if self.x11_colormap != 0 {
                unsafe {
                    (xlib.XFreeColormap)(x_display, self.x11_colormap);
                }
                self.x11_colormap = 0;
            }
            unsafe { (xlib.XFlush)(x_display) };
            if self.owns_display {
                unsafe { (xlib.XCloseDisplay)(x_display) };
                self.x11_display = None;
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

static FRAME_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static CONSECUTIVE_FAILURES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Reset per-session frame diagnostics. Called at the start of each attach()
/// so logs for a new playback session start from frame #0.
fn reset_frame_counters() {
    FRAME_COUNT.store(0, Ordering::Relaxed);
    CONSECUTIVE_FAILURES.store(0, Ordering::Relaxed);
}

/// Render one frame. Called on the glib main thread by the update callback.
/// Safety: caller must verify `valid = true`; `inner_ptr` must be live.
unsafe fn render_frame(inner_ptr: usize) {
    let inner = &mut *(inner_ptr as *mut RenderInner);

    if !inner.video_active.load(Ordering::Acquire) {
        return;
    }

    let egl = egl_instance();

    // 1. Make the EGL context current FIRST — wl_egl_window_resize() requires
    //    the EGL surface to be current on the calling thread.
    //
    //    On Wayland, a single eglMakeCurrent failure can cascade (context lost
    //    state persists). Retry once after releasing the context, and log the
    //    EGL error code for diagnostics.
    let make_current = || {
        egl.make_current(
            inner.egl_display,
            Some(inner.egl_surface),
            Some(inner.egl_surface),
            Some(inner.egl_context),
        )
    };

    if make_current().is_err() {
        let err = egl.get_error();
        let failures = CONSECUTIVE_FAILURES.fetch_add(1, Ordering::Relaxed) + 1;
        tracing::warn!(
            "[Linux renderer] render_frame: eglMakeCurrent failed (EGL error {:?}, failure #{}) retrying",
            err, failures
        );
        // Release any stale context state before retrying.
        let _ = egl.make_current(inner.egl_display, None, None, None);
        if make_current().is_err() {
            let err2 = egl.get_error();
            tracing::error!(
                "[Linux renderer] render_frame: eglMakeCurrent retry failed (EGL error {:?}, failure #{}), skipping frame",
                err2, failures
            );
            return;
        }
    }

    // 2. Apply any pending wl_egl_window resize AFTER making context current.
    //    This ensures the resize and subsequent EGL calls happen on the same
    //    thread with the surface current, avoiding the race condition that
    //    caused color corruption. Also set glViewport and clear the buffer
    //    to avoid garbage from newly allocated regions.
    let mut did_resize = false;
    if inner.wl_egl_surface_ptr != 0 {
        if let Ok(mut pending) = inner.pending_resize.lock() {
            if let Some(resize) = pending.take() {
                let wl_egl = &*(inner.wl_egl_surface_ptr as *const WlEglSurface);
                wl_egl.resize(resize.w, resize.h, 0, 0);
                gl::Viewport(0, 0, resize.w, resize.h);
                gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                gl::Clear(gl::COLOR_BUFFER_BIT);
                did_resize = true;
            }
        }
    }

    let w = egl.query_surface(inner.egl_display, inner.egl_surface, egl::WIDTH).unwrap_or(0);
    let h = egl.query_surface(inner.egl_display, inner.egl_surface, egl::HEIGHT).unwrap_or(0);
    if w < 1 || h < 1 {
        return;
    }

    let rc = &inner.ctx as *const RenderContext;
    let rc: &RenderContext = &*rc;

    // 3. Check if mpv has a new frame or if we need to force a redraw after resize.
    //    When paused, mpv doesn't fire update callbacks, so resizes would never
    //    be visually applied. Calling render() unconditionally after resize
    //    redraws the last frame at the new size (mpv docs confirm this behavior).
    let should_render = match rc.update() {
        Ok(flags) => (flags & mpv_render_update::Frame != 0) || did_resize,
        Err(e) => {
            tracing::trace!("[Linux renderer] update error: {}", e);
            did_resize // Still render if we resized, even if update() fails
        }
    };

    if should_render {
        tracing::trace!("[Linux renderer] rendering frame (fbo=0, {}x{})...", w, h);
        if let Err(e) = rc.render::<*mut c_void>(0, w, h, true) {
            let failures = CONSECUTIVE_FAILURES.fetch_add(1, Ordering::Relaxed) + 1;
            tracing::error!("[Linux renderer] rc.render() failed (failure #{}): {}", failures, e);
            return;
        }
        if egl.swap_buffers(inner.egl_display, inner.egl_surface).is_err() {
            let egl_err = egl.get_error();
            let failures = CONSECUTIVE_FAILURES.fetch_add(1, Ordering::Relaxed) + 1;
            tracing::error!(
                "[Linux renderer] eglSwapBuffers failed (EGL error {:?}, failure #{})",
                egl_err, failures
            );
            return;
        }
        rc.report_swap();

        // Reset failure counter on success.
        CONSECUTIVE_FAILURES.store(0, Ordering::Relaxed);

        let n = FRAME_COUNT.fetch_add(1, Ordering::Relaxed);
        if n < 5 || n % 60 == 0 {
            tracing::debug!("[Linux renderer] frame presented (#{n})");
        }

        // X11: periodically re-lower the video window to keep it below WebKit.
        // GTK event processing (ConfigureNotify, expose) can silently restack
        // the video child above GDK-managed windows. Re-lowering on the first
        // 5 frames (immediate visual fix) and then every 30 frames (~0.5s at
        // 60fps) keeps overhead minimal while ensuring controls stay visible.
        if inner.x11_display_ptr != 0 && (n < 5 || n % 30 == 0) {
            if let Some(xlib) = xlib_for_stacking() {
                (xlib.0.XLowerWindow)(
                    inner.x11_display_ptr as *mut x11_dl::xlib::Display,
                    inner.x11_child_window,
                );
            }
        }

        if let Some(cb) = inner.first_frame_cb.take() {
            cb();
        }
    }
}

// ---------------------------------------------------------------------------
// MPV option sets
// ---------------------------------------------------------------------------

/// Detect whether we are running inside a bundled package (AppImage or deb install)
/// as opposed to a dev build. Bundled builds have libmpv and its transitive deps
/// alongside the binary, which can conflict with system GPU driver plugins.
fn is_bundled_build() -> bool {
    // AppImage sets APPIMAGE env var
    if std::env::var("APPIMAGE").is_ok() {
        return true;
    }
    // Deb/rpm install: binary lives under /usr or /opt, not in a cargo target dir
    if let Ok(exe) = std::env::current_exe() {
        let path = exe.to_string_lossy();
        if path.starts_with("/usr/") || path.starts_with("/opt/") {
            return true;
        }
    }
    false
}

/// Options for embedded playback via OpenGL render context (vo=libmpv).
pub fn embedded_options() -> Vec<(&'static str, &'static str)> {
    // In bundled builds (AppImage/deb), the bundled ffmpeg's hwdec paths may
    // load system VA-API/VDPAU driver plugins that were compiled against
    // different library versions, causing SIGSEGV. Default to hwdec=no for
    // safety. Users can override with MVP_HWDEC=auto-copy if their system
    // is compatible.
    let hwdec = match std::env::var("MVP_HWDEC") {
        Ok(val) => {
            tracing::info!("[Linux renderer] hwdec override via MVP_HWDEC={}", val);
            // Leak the string so we get a &'static str.
            // Only happens once per process, so the leak is negligible.
            &*Box::leak(val.into_boxed_str())
        }
        Err(_) => {
            if is_bundled_build() {
                tracing::info!(
                    "[Linux renderer] bundled build detected — using hwdec=no \
                     (set MVP_HWDEC=auto-copy to override)"
                );
                "no"
            } else {
                "auto-copy"
            }
        }
    };

    vec![
        ("vo", "libmpv"),
        ("hwdec", hwdec),
        ("video-sync", "audio"),
        ("cache", "yes"),
        ("demuxer-max-bytes", "150MiB"),
        ("demuxer-max-back-bytes", "75MiB"),
        ("keep-open", "yes"),
        ("terminal", "yes"),
        ("msg-level", "all=status"),
    ]
}

/// Options for fallback separate window (vo=gpu, native OSC shown automatically).
pub fn fallback_options() -> Vec<(&'static str, &'static str)> {
    let hwdec = if is_bundled_build() { "no" } else { "auto" };
    vec![
        ("hwdec", hwdec),
        ("video-sync", "display-resample"),
        ("cache", "yes"),
        ("demuxer-max-bytes", "150MiB"),
        ("demuxer-max-back-bytes", "75MiB"),
        ("keep-open", "yes"),
        ("terminal", "yes"),
        ("msg-level", "all=status"),
    ]
}

/// Software-safe fallback options for blocklisted software renderers (llvmpipe, swrast, softpipe).
/// Uses vo=x11 (pure software blitting) and hwdec=no to avoid GPU/GL calls
/// that fail on these drivers.
pub fn software_fallback_options() -> Vec<(&'static str, &'static str)> {
    vec![
        ("vo", "x11"),
        ("hwdec", "no"),
        // No "ao" — libmpv default auto-iterates compiled backends.
        ("cache", "yes"),
        ("demuxer-max-bytes", "150MiB"),
        ("demuxer-max-back-bytes", "75MiB"),
        ("keep-open", "yes"),
        ("terminal", "yes"),
        ("msg-level", "all=status"),
    ]
}
