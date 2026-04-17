//! Linux MPV embedding — Wayland subsurface + EGL + OpenGL render context.
//!
//! Architecture:
//! - Create a `wl_subsurface` + `wl_egl_window` underneath the Tauri `wl_surface`.
//! - Build an EGL (OpenGL Core 3.2) context on that window.
//! - Give libmpv an `MPV_RENDER_API_TYPE_OPENGL` render context and, crucially,
//!   the native `wl_display` pointer via `MPV_RENDER_PARAM_WL_DISPLAY` so
//!   libmpv can interoperate with the compositor (required for dmabuf/zwp
//!   colorspace and for correct presentation timing on Wayland).
//! - Every EGL / GL / `wl_egl_window_resize` call is dispatched to the GLib
//!   main thread: the same thread that owns the EGL context for the entire
//!   lifetime of the renderer. This is the Wayland-safe equivalent of the
//!   NSOpenGLView + main-thread pattern used on macOS.
//!
//! X11 is intentionally unsupported on this platform: X11 sessions fall through
//! to the separate-window fallback in `MpvState`.

use crate::renderer::PlatformRenderer;
use khronos_egl as egl;
use libmpv2::{
    render::{mpv_render_update, OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType},
    Mpv,
};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use tauri::{AppHandle, Manager, Runtime};

use wayland_client::{
    backend::{Backend, ObjectId},
    globals::{registry_queue_init, GlobalListContents},
    protocol::{
        wl_compositor::WlCompositor, wl_registry::WlRegistry,
        wl_subcompositor::WlSubcompositor, wl_subsurface::WlSubsurface, wl_surface::WlSurface,
    },
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
};
use wayland_egl::WlEglSurface;

// =========================================================================
// libEGL loader
// =========================================================================

/// Dynamically loaded `libEGL.so.1`. Cached for the lifetime of the process
/// so we pay the `dlopen` cost once and every subsequent lookup is a pointer
/// read. Returns `Err` (never panics) so `LinuxGlRenderer::new` can fall back.
fn egl() -> Result<&'static egl::DynamicInstance<egl::EGL1_4>, String> {
    static INSTANCE: OnceLock<Result<egl::DynamicInstance<egl::EGL1_4>, String>> = OnceLock::new();
    INSTANCE
        .get_or_init(|| {
            // SAFETY: `load_required` wraps `dlopen("libEGL.so.1")`; dlopen is
            // async-signal-safe and thread-safe, and the returned instance is
            // valid for the lifetime of the process.
            unsafe { egl::DynamicInstance::<egl::EGL1_4>::load_required() }
                .map_err(|e| format!("Failed to load libEGL: {e}"))
        })
        .as_ref()
        .map_err(|e| e.clone())
}

/// Resolve a GL function pointer via the EGL loader.
///
/// Returned as `*mut c_void` for both `gl::load_with` (the `gl` crate) and
/// libmpv's `get_proc_address` callback. `gl::load_with` is called exactly
/// once per attach, on the GLib main thread, while the EGL context is current.
fn gl_proc_address(name: &str) -> *mut c_void {
    match egl() {
        Ok(egl) => egl
            .get_proc_address(name)
            .map_or(std::ptr::null_mut(), |f| f as *mut c_void),
        Err(_) => std::ptr::null_mut(),
    }
}

// =========================================================================
// Wayland globals dispatcher (registry-only, silent otherwise)
// =========================================================================

struct WlGlobals;

impl Dispatch<WlRegistry, GlobalListContents> for WlGlobals {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: wayland_client::protocol::wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

wayland_client::delegate_noop!(WlGlobals: ignore WlCompositor);
wayland_client::delegate_noop!(WlGlobals: ignore WlSubcompositor);
wayland_client::delegate_noop!(WlGlobals: ignore WlSurface);
wayland_client::delegate_noop!(WlGlobals: ignore WlSubsurface);

// =========================================================================
// RAII-owned platform resources
// =========================================================================

/// Owns the EGL context + surface. The EGL *display* is not terminated on
/// drop because on Wayland it is shared with GTK/WebKit — terminating it
/// would tear down the compositor connection for the whole window.
struct OwnedEgl {
    display: egl::Display,
    surface: egl::Surface,
    context: egl::Context,
}

impl Drop for OwnedEgl {
    fn drop(&mut self) {
        // SAFETY: Drop runs on the GLib main thread (`Inner` is dropped from
        // there in `detach`/drop). `make_current(None)` is a no-op if the
        // context is not current; `destroy_surface` / `destroy_context` are
        // valid to call with no pending rendering on this thread.
        if let Ok(egl) = egl() {
            let _ = egl.make_current(self.display, None, None, None);
            let _ = egl.destroy_surface(self.display, self.surface);
            let _ = egl.destroy_context(self.display, self.context);
        }
    }
}

// SAFETY: The three handles are opaque pointers into libEGL's process-wide
// state. Our architecture restricts every EGL call that reads those handles
// (make_current, swap_buffers, destroy_*) to the GLib main thread. The Sync
// impl exists only so `Arc<Inner>` is Sync; OwnedEgl is never mutated via
// `&self` from multiple threads.
unsafe impl Send for OwnedEgl {}
unsafe impl Sync for OwnedEgl {}

/// Owns the Wayland subsurface tree that backs the EGL window.
///
/// Field order is the drop order: we release the EGL window buffer (which
/// decrements libEGL's reference on the child surface) before sending
/// `destroy` for the subsurface, and drop the Connection last so the fd
/// stays open while every protocol object on it is torn down.
struct WaylandSession {
    wl_egl_surface: WlEglSurface,
    subsurface: WlSubsurface,
    child_surface: WlSurface,
    queue: Mutex<EventQueue<WlGlobals>>,
    conn: Connection,
    /// Native `wl_display *` — borrowed from the Tauri window handle and owned
    /// by GTK. We keep it here as the value to pass as
    /// `MPV_RENDER_PARAM_WL_DISPLAY` and never free it ourselves.
    wl_display_ptr: *mut c_void,
}

impl Drop for WaylandSession {
    fn drop(&mut self) {
        self.subsurface.destroy();
        self.child_surface.commit();
        let _ = self.conn.flush();
        // Remaining fields drop in declaration order (wl_egl_surface already
        // dropped first because it is declared first — see field order above).
    }
}

// SAFETY: `WlEglSurface` wraps a `*mut wl_egl_window`. The resize operation is
// not thread-safe relative to EGL rendering, so we restrict it to the GLib
// main thread. Subsurface / surface protocol requests go through
// wayland-client's internally-locked Backend and are safe from any thread.
unsafe impl Send for WaylandSession {}
unsafe impl Sync for WaylandSession {}

// =========================================================================
// Mutable state shared across threads
// =========================================================================

#[derive(Default)]
struct SessionState {
    /// Pending `wl_egl_window_resize` queued by `set_frame`, applied by the
    /// GLib render thread while the EGL context is current.
    pending_resize: Option<(i32, i32)>,
    /// Last content rect (x, y, w, h) — used to restore position after unhide.
    last_frame: (i32, i32, i32, i32),
    /// CSD offset (shadow margin + header bar) added to subsurface position.
    csd_offset: (i32, i32),
}

// =========================================================================
// Inner (shared between renderer and libmpv update callback)
// =========================================================================

struct Inner {
    state: Mutex<SessionState>,
    first_frame_cb: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    /// Lockless hot-path flag toggled by `set_visible`.
    video_active: AtomicBool,
    /// Set to `false` by `detach` so any in-flight idle callback exits
    /// before touching resources that are about to be freed.
    valid: AtomicBool,
    // Platform resources — dropped last (after the Mutex/Atomic fields) in
    // field order: render_ctx → egl → wayland.
    render_ctx: RenderContext,
    egl: OwnedEgl,
    wayland: WaylandSession,
}

// SAFETY: `RenderContext`, `OwnedEgl`, and `WaylandSession` are only
// dereferenced from the GLib main thread (via `render_frame`). The libmpv
// update callback runs on an arbitrary thread but only schedules an idle
// task; it does not touch any of these fields directly. Mutex/Atomic fields
// are inherently thread-safe.
unsafe impl Send for Inner {}
unsafe impl Sync for Inner {}

// =========================================================================
// LinuxGlRenderer
// =========================================================================

pub struct LinuxGlRenderer {
    /// Pre-attach resources owned by the renderer until `attach` consumes them.
    pending: Option<Pending>,
    /// Active render state. `Some` between `attach` and `detach`.
    active: Option<Arc<Inner>>,
    /// Queued up by the caller before `attach`; moved into `Inner` there.
    /// `Mutex` only exists to satisfy `Sync`; we never contend it
    /// (`set_first_frame_callback` / `attach` are both called from the
    /// command thread before the renderer becomes shared).
    first_frame_cb: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

struct Pending {
    egl: OwnedEgl,
    wayland: WaylandSession,
    csd_offset: (i32, i32),
}

impl LinuxGlRenderer {
    /// Build the EGL context and Wayland subsurface tree. Runs on the GLib
    /// main thread (via `MainContext::invoke`) because every EGL call in the
    /// construction path must use the same thread as rendering.
    pub fn new<R: Runtime>(app: &AppHandle<R>) -> Result<Self, String> {
        // Fail fast if libEGL is not installed on the host.
        egl()?;

        let csd_offset = query_csd_offsets(app);

        let window = app
            .get_webview_window("main")
            .ok_or_else(|| "Window 'main' not found".to_string())?;

        let raw_window = window
            .window_handle()
            .map_err(|e| format!("window handle: {e:?}"))?
            .as_raw();
        let raw_display = window
            .display_handle()
            .map_err(|e| format!("display handle: {e:?}"))?
            .as_raw();

        let (wl_surface_ptr, wl_display_ptr) = match (raw_window, raw_display) {
            (RawWindowHandle::Wayland(wh), RawDisplayHandle::Wayland(dh)) => {
                (wh.surface.as_ptr(), dh.display.as_ptr())
            }
            _ => return Err("Embedded Linux renderer requires a Wayland session".into()),
        };

        let (egl_res, wayland) = build_wayland_on_main_thread(wl_surface_ptr, wl_display_ptr)?;

        Ok(Self {
            pending: Some(Pending {
                egl: egl_res,
                wayland,
                csd_offset,
            }),
            active: None,
            first_frame_cb: Mutex::new(None),
        })
    }

    /// Register a callback invoked once, on the first presented frame. Must
    /// be called before `attach`; it is moved into `Inner` at that point.
    pub fn set_first_frame_callback(&mut self, cb: Box<dyn FnOnce() + Send>) {
        if let Ok(mut slot) = self.first_frame_cb.lock() {
            *slot = Some(cb);
        }
    }
}

// =========================================================================
// PlatformRenderer impl
// =========================================================================

impl PlatformRenderer for LinuxGlRenderer {
    fn attach(&mut self, mpv: &mut Mpv) -> Result<(), String> {
        let Pending { egl: egl_res, wayland, csd_offset } = self
            .pending
            .take()
            .ok_or_else(|| "Renderer has already been attached".to_string())?;

        let first_frame_cb = self
            .first_frame_cb
            .lock()
            .ok()
            .and_then(|mut slot| slot.take());

        // `NonNull<mpv_handle>::as_ptr()` returns a `!Send` raw pointer.
        // Transport it through the dispatch closure as `usize` (which is
        // unconditionally `Send + Sync`) and cast back on the other side.
        // This avoids a wrapper type with its own `unsafe impl Send`.
        // The pointer is owned by the `MpvEngine` mutex held by the
        // caller for the duration of this call and is dereferenced only
        // on the GLib main thread.
        let mpv_ptr_usize = mpv.ctx.as_ptr() as usize;

        // Build the render context, `Arc<Inner>`, and wire the update
        // callback all on the GLib main thread. `Arc<Inner>` is `Send` via
        // the existing `unsafe impl Send for Inner`, so the dispatch
        // channel needs no extra send-assertion wrapper on the return.
        let inner = run_on_glib_main(move || -> Result<Arc<Inner>, String> {
            let mpv_ptr = mpv_ptr_usize as *mut c_void;
            let egl = egl()?;
            let display = egl_res.display;
            let surface = egl_res.surface;
            let context = egl_res.context;
            let wl_display_ptr = wayland.wl_display_ptr;

            egl.make_current(display, Some(surface), Some(surface), Some(context))
                .map_err(|e| format!("eglMakeCurrent (init): {e:?}"))?;

            // Load `gl` crate's dispatch table while the context is current.
            // This is the only `gl::load_with` call in the process; after
            // this, every `gl::*` call in `render_frame` is a static lookup.
            gl::load_with(|name| gl_proc_address(name) as *const _);

            fn get_proc_address(_ctx: &*mut c_void, name: &str) -> *mut c_void {
                gl_proc_address(name)
            }

            // SAFETY: `mpv_ptr` (originally `NonNull<mpv_handle>::as_ptr`)
            // remains valid for the duration of this call because the
            // caller is holding the MpvEngine mutex. The cast to
            // `*mut _` is inferred to `*mut mpv_handle` by the signature
            // of `RenderContext::new`; we never dereference it for any
            // other purpose.
            let mut render_ctx = RenderContext::new(
                unsafe { &mut *mpv_ptr.cast() },
                vec![
                    RenderParam::ApiType(RenderParamApiType::OpenGl),
                    RenderParam::InitParams(OpenGLInitParams {
                        get_proc_address,
                        ctx: std::ptr::null_mut::<c_void>(),
                    }),
                    // Tells libmpv which Wayland display the GL context is
                    // on so it can request dmabuf formats from the
                    // compositor and match its presentation timing.
                    RenderParam::WaylandDisplay(wl_display_ptr as *const c_void),
                ],
            )
            .map_err(|e| format!("mpv_render_context_create: {e}"))?;

            // SAFETY: gl crate dispatch is valid on this thread because we
            // just called `load_with` while the context was current.
            unsafe {
                gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                gl::Clear(gl::COLOR_BUFFER_BIT);
            }
            let _ = egl.swap_buffers(display, surface);
            let _ = egl.make_current(display, None, None, None);

            // Build the Arc with `new_cyclic` so the mpv update callback can
            // capture a `Weak<Inner>` that refers to the Arc currently being
            // constructed. This avoids needing `Arc::get_mut` after the fact
            // (which fails once a `Weak` exists, because `get_mut` requires
            // both strong_count == 1 AND weak_count == 0).
            //
            // If `detach` drops the only strong Arc before an idle callback
            // runs, `upgrade()` returns None and the callback is a no-op —
            // no dangling access possible.
            let inner = Arc::new_cyclic(move |weak: &std::sync::Weak<Inner>| {
                let weak = weak.clone();
                render_ctx.set_update_callback(move || {
                    if let Some(alive) = weak.upgrade() {
                        glib::idle_add_once(move || {
                            if alive.valid.load(Ordering::Acquire) {
                                render_frame(&alive);
                            }
                        });
                    }
                });
                Inner {
                    state: Mutex::new(SessionState {
                        pending_resize: None,
                        last_frame: (0, 0, 0, 0),
                        csd_offset,
                    }),
                    first_frame_cb: Mutex::new(first_frame_cb),
                    video_active: AtomicBool::new(true),
                    valid: AtomicBool::new(true),
                    render_ctx,
                    egl: egl_res,
                    wayland,
                }
            });
            Ok(inner)
        })?;

        self.active = Some(inner);
        Ok(())
    }

    fn resize(&mut self, _width: u32, _height: u32) {
        // Positioning is driven by `set_frame` from the frontend layout.
    }

    fn set_frame(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let Some(inner) = self.active.as_ref() else { return };
        let wi = (w as i32).max(1);
        let hi = (h as i32).max(1);

        let (adjusted_x, adjusted_y) = {
            let mut state = match inner.state.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            let ax = x as i32 + state.csd_offset.0;
            let ay = y as i32 + state.csd_offset.1;
            state.last_frame = (ax, ay, wi, hi);
            state.pending_resize = Some((wi, hi));
            (ax, ay)
        };

        // Subsurface position is double-buffered through wayland-client's
        // locked Backend — safe to call from the command thread. The
        // `wl_egl_window_resize` is applied later by `render_frame` on the
        // GLib thread where the EGL context is current.
        inner.wayland.subsurface.set_position(adjusted_x, adjusted_y);
        inner.wayland.child_surface.commit();
        let _ = inner.wayland.conn.flush();
        drain_wayland_queue(inner);

        // Kick a render so the resize is visible even when paused.
        let weak = Arc::downgrade(inner);
        glib::idle_add_once(move || {
            if let Some(alive) = weak.upgrade() {
                if alive.valid.load(Ordering::Acquire) {
                    render_frame(&alive);
                }
            }
        });
    }

    fn set_visible(&mut self, visible: bool) {
        let Some(inner) = self.active.as_ref() else { return };
        inner.video_active.store(visible, Ordering::Release);

        if visible {
            let (lx, ly, lw, lh) = {
                let state = match inner.state.lock() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                state.last_frame
            };
            inner.wayland.subsurface.set_position(lx, ly);
            if lw > 0 && lh > 0 {
                if let Ok(mut state) = inner.state.lock() {
                    state.pending_resize = Some((lw, lh));
                }
            }
        } else {
            // Move the subsurface off-screen rather than `wl_surface.attach`
            // a null buffer — attaching null on an EGL-managed surface would
            // trigger a protocol error with some compositors.
            inner.wayland.subsurface.set_position(-32000, -32000);
        }
        inner.wayland.child_surface.commit();
        let _ = inner.wayland.conn.flush();
        drain_wayland_queue(inner);
    }

    fn set_first_frame_callback(&mut self, cb: Box<dyn FnOnce() + Send>) {
        if let Some(inner) = self.active.as_ref() {
            if let Ok(mut slot) = inner.first_frame_cb.lock() {
                *slot = Some(cb);
            }
        } else if let Ok(mut slot) = self.first_frame_cb.lock() {
            *slot = Some(cb);
        }
    }

    fn detach(&mut self) {
        let Some(inner) = self.active.take() else { return };
        inner.valid.store(false, Ordering::Release);

        // All tear-down (RenderContext -> OwnedEgl -> WaylandSession) must
        // happen on the GLib main thread so it serializes behind any pending
        // idle render callbacks and uses the correct EGL context thread.
        run_on_glib_main(move || {
            // Dropping `inner` on this thread runs Drop for RenderContext,
            // OwnedEgl, and WaylandSession in field order.
            drop(inner);
        });
    }
}

impl Drop for LinuxGlRenderer {
    fn drop(&mut self) {
        self.detach();
        // `pending` (if attach never ran) drops here, also requires GLib
        // thread for the EGL handles. Construction happened there, and
        // dropping on a different thread is usually fine for destroy_*
        // calls because no render thread touches them in that case.
    }
}

// =========================================================================
// Per-frame render — GLib main thread only
// =========================================================================

fn render_frame(inner: &Inner) {
    if !inner.video_active.load(Ordering::Acquire) {
        return;
    }
    let Ok(egl) = egl() else { return };

    let display = inner.egl.display;
    let surface = inner.egl.surface;
    let context = inner.egl.context;

    if egl
        .make_current(display, Some(surface), Some(surface), Some(context))
        .is_err()
    {
        return;
    }

    let mut did_resize = false;
    if let Ok(mut state) = inner.state.lock() {
        if let Some((w, h)) = state.pending_resize.take() {
            inner.wayland.wl_egl_surface.resize(w, h, 0, 0);
            // SAFETY: gl::load_with was called at attach time with this EGL
            // context current; the context is current again on this thread.
            unsafe {
                gl::Viewport(0, 0, w, h);
                gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                gl::Clear(gl::COLOR_BUFFER_BIT);
            }
            did_resize = true;
        }
    }

    let w = egl.query_surface(display, surface, egl::WIDTH).unwrap_or(0);
    let h = egl.query_surface(display, surface, egl::HEIGHT).unwrap_or(0);
    if w < 1 || h < 1 {
        return;
    }

    let should_render = match inner.render_ctx.update() {
        Ok(flags) => (flags & mpv_render_update::Frame != 0) || did_resize,
        Err(_) => did_resize,
    };

    if !should_render {
        return;
    }

    if inner
        .render_ctx
        .render::<*mut c_void>(0, w, h, true)
        .is_err()
    {
        return;
    }
    if egl.swap_buffers(display, surface).is_err() {
        return;
    }
    inner.render_ctx.report_swap();

    if let Ok(mut slot) = inner.first_frame_cb.lock() {
        if let Some(cb) = slot.take() {
            cb();
        }
    }
}

// =========================================================================
// Construction helpers
// =========================================================================

/// Build EGL + Wayland subsurface resources on the GLib main thread.
fn build_wayland_on_main_thread(
    wl_surface_ptr: *mut c_void,
    wl_display_ptr: *mut c_void,
) -> Result<(OwnedEgl, WaylandSession), String> {
    // Raw pointers aren't Send, but `usize` is. Transport the two GTK-owned
    // Wayland handles across the main-thread dispatch boundary as integers
    // and cast back inside the closure. This avoids an `unsafe impl Send`
    // wrapper and sidesteps RFC 2229 disjoint-capture, which would otherwise
    // split any struct wrapper into its non-Send field captures.
    let ws_usize = wl_surface_ptr as usize;
    let wd_usize = wl_display_ptr as usize;
    run_on_glib_main(move || build_wayland(ws_usize as *mut c_void, wd_usize as *mut c_void))
}

fn build_wayland(
    wl_surface_ptr: *mut c_void,
    wl_display_ptr: *mut c_void,
) -> Result<(OwnedEgl, WaylandSession), String> {
    let egl = egl()?;

    // Secondary reference to the GTK Wayland connection. Shares the same fd
    // and object namespace, so ObjectId::from_ptr recognises GTK's objects.
    //
    // SAFETY: `wl_display_ptr` is a valid `wl_display *` owned by GTK for
    // the lifetime of the window. `from_foreign_display` only reads the fd
    // and bumps a refcount; it never frees the display.
    let backend = unsafe { Backend::from_foreign_display(wl_display_ptr as *mut _) };
    let conn = Connection::from_backend(backend);

    let (globals, mut queue) = registry_queue_init::<WlGlobals>(&conn)
        .map_err(|e| format!("Wayland registry_queue_init: {e}"))?;
    let qh = queue.handle();

    let compositor: WlCompositor = globals
        .bind(&qh, 4..=5, ())
        .map_err(|e| format!("bind wl_compositor: {e}"))?;
    let subcompositor: WlSubcompositor = globals
        .bind(&qh, 1..=1, ())
        .map_err(|e| format!("bind wl_subcompositor: {e}"))?;

    let mut dummy = WlGlobals;
    queue
        .roundtrip(&mut dummy)
        .map_err(|e| format!("wayland roundtrip: {e}"))?;

    // Wrap the parent surface pointer as a wayland-client proxy so we can
    // pass it to `get_subsurface`. We never receive events on it and its
    // Drop does not send `wl_surface.destroy` — GTK owns the surface.
    //
    // SAFETY: `wl_surface_ptr` is a valid `wl_surface *` on the same
    // connection we are using (shared fd via `from_foreign_display`).
    let parent_id = unsafe {
        ObjectId::from_ptr(WlSurface::interface(), wl_surface_ptr as *mut _)
    }
    .map_err(|_| "invalid parent wl_surface pointer")?;
    let parent_surface = WlSurface::from_id(&conn, parent_id)
        .map_err(|_| "cannot build proxy for parent wl_surface")?;

    let child_surface = compositor.create_surface(&qh, ());
    let subsurface = subcompositor.get_subsurface(&child_surface, &parent_surface, &qh, ());
    subsurface.place_below(&parent_surface);
    subsurface.set_desync();
    child_surface.commit();
    conn.flush().map_err(|e| format!("wayland flush: {e}"))?;

    // EGL: prefer `eglGetPlatformDisplayEXT(EGL_PLATFORM_WAYLAND_EXT)` so
    // libmpv's internal Wayland integration recognises the display as a
    // Wayland-backed one. Fall back to `eglGetDisplay` for old drivers.
    const EGL_PLATFORM_WAYLAND_EXT: u32 = 0x31D8;
    let egl_display = wayland_egl_display(egl, wl_display_ptr, EGL_PLATFORM_WAYLAND_EXT)?;

    // `eglInitialize` is idempotent per spec, but some Mesa releases
    // mishandle double-init on a display already initialised by GTK. Only
    // call it if `EGL_VERSION` is not yet queryable.
    if egl.query_string(Some(egl_display), egl::VERSION).is_err() {
        egl.initialize(egl_display)
            .map_err(|e| format!("eglInitialize: {e:?}"))?;
    }

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
    let config = egl
        .choose_first_config(egl_display, &config_attribs)
        .map_err(|e| format!("eglChooseConfig: {e:?}"))?
        .ok_or("no suitable EGL config")?;
    egl.bind_api(egl::OPENGL_API)
        .map_err(|e| format!("eglBindAPI(OPENGL): {e:?}"))?;

    let wl_egl_surface = WlEglSurface::new(child_surface.id(), 1, 1)
        .map_err(|e| format!("wl_egl_window_create: {e:?}"))?;

    // SAFETY: `wl_egl_surface.ptr()` is a valid `wl_egl_window *` and stays
    // alive as long as `WaylandSession` owns `wl_egl_surface`.
    let egl_surface = unsafe {
        egl.create_window_surface(
            egl_display,
            config,
            wl_egl_surface.ptr() as egl::NativeWindowType,
            None,
        )
    }
    .map_err(|e| format!("eglCreateWindowSurface: {e:?}"))?;

    let context_attribs = [
        egl::CONTEXT_MAJOR_VERSION, 3,
        egl::CONTEXT_MINOR_VERSION, 2,
        egl::CONTEXT_OPENGL_PROFILE_MASK, egl::CONTEXT_OPENGL_CORE_PROFILE_BIT,
        egl::NONE,
    ];
    let egl_context = egl
        .create_context(egl_display, config, None, &context_attribs)
        .map_err(|e| format!("eglCreateContext: {e:?}"))?;

    let egl_res = OwnedEgl {
        display: egl_display,
        surface: egl_surface,
        context: egl_context,
    };
    let wayland = WaylandSession {
        wl_egl_surface,
        subsurface,
        child_surface,
        queue: Mutex::new(queue),
        conn,
        wl_display_ptr,
    };
    Ok((egl_res, wayland))
}

fn wayland_egl_display(
    egl: &egl::DynamicInstance<egl::EGL1_4>,
    wl_display_ptr: *mut c_void,
    platform_enum: u32,
) -> Result<egl::Display, String> {
    type GetPlatformDisplayExt =
        unsafe extern "C" fn(u32, *mut c_void, *const i32) -> *mut c_void;

    if let Some(get_fn) = egl.get_proc_address("eglGetPlatformDisplayEXT") {
        // SAFETY: `eglGetPlatformDisplayEXT` has the documented signature
        // `EGLDisplay(EGLenum, void*, const EGLint*)`; transmuting the proc
        // address to that fn pointer matches the loader's contract.
        let get_platform_display: GetPlatformDisplayExt = unsafe { std::mem::transmute(get_fn) };
        // SAFETY: calls into libEGL with a valid wl_display pointer.
        let d_ptr = unsafe {
            get_platform_display(platform_enum, wl_display_ptr, std::ptr::null())
        };
        if !d_ptr.is_null() {
            // SAFETY: `egl::Display` is `repr(transparent)` over `*mut c_void`.
            return Ok(unsafe { std::mem::transmute::<*mut c_void, egl::Display>(d_ptr) });
        }
    }

    // SAFETY: Fallback path; `eglGetDisplay` accepts any native display
    // pointer and returns EGL_NO_DISPLAY on failure (mapped to None).
    unsafe { egl.get_display(wl_display_ptr) }
        .ok_or_else(|| "eglGetDisplay returned EGL_NO_DISPLAY".to_string())
}

/// Query GTK for the CSD (Client-Side Decoration) offset so subsurface
/// coordinates (which are relative to the parent `wl_surface`, inclusive of
/// shadow + titlebar) line up with frontend content-area coordinates.
fn query_csd_offsets<R: Runtime>(app: &AppHandle<R>) -> (i32, i32) {
    use gtk::prelude::*;

    if app.get_webview_window("main").is_none() {
        return (0, 0);
    }

    let (tx, rx) = std::sync::mpsc::channel();
    let app_handle = app.clone();

    glib::idle_add_once(move || {
        let Some(window) = app_handle.get_webview_window("main") else {
            let _ = tx.send((0, 0));
            return;
        };
        let offsets = (|| -> Option<(i32, i32)> {
            let gtk_win = window.gtk_window().ok()?;
            let vbox = window.default_vbox().ok()?;
            let (vbox_x, vbox_y) = vbox.translate_coordinates(&gtk_win, 0, 0)?;
            let gdk_win = gtk_win.window()?;
            let alloc = gtk_win.allocation();
            let shadow_x = (gdk_win.width() - alloc.width()).max(0) / 2;
            let shadow_y = (gdk_win.height() - alloc.height()).max(0) / 2;
            Some((shadow_x + vbox_x, shadow_y + vbox_y))
        })();
        let _ = tx.send(offsets.unwrap_or((0, 0)));
    });
    rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap_or((0, 0))
}

// =========================================================================
// GLib main-thread dispatch helper
// =========================================================================

/// Run `f` on the GLib main thread, blocking until it returns.
///
/// Uses `MainContext::invoke` (higher priority than idle) when dispatching,
/// and runs inline if already on the main thread to avoid deadlocking on
/// our own callback.
fn run_on_glib_main<F, T>(f: F) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let main_ctx = glib::MainContext::default();
    if main_ctx.is_owner() {
        return f();
    }
    let (tx, rx) = std::sync::mpsc::channel();
    main_ctx.invoke(move || {
        let _ = tx.send(f());
    });
    rx.recv()
        .expect("GLib main thread dispatch channel dropped before sending")
}

// =========================================================================
// Wayland event-queue drain
// =========================================================================

fn drain_wayland_queue(inner: &Inner) {
    let Ok(mut queue) = inner.wayland.queue.lock() else { return };
    let mut dummy = WlGlobals;
    let _ = queue.dispatch_pending(&mut dummy);
}

// =========================================================================
// MPV option sets
// =========================================================================

/// Options for embedded playback via the OpenGL render context (vo=libmpv).
pub fn embedded_options() -> Vec<(&'static str, &'static str)> {
    vec![
        ("vo", "libmpv"),
        // `auto-copy` decodes on the GPU but copies frames to CPU for GL
        // upload. Plain `auto` can map VAAPI surfaces directly as GL
        // textures, which produces colour corruption on drivers that
        // cannot handle the NV12→RGB interop.
        ("hwdec", "auto-copy"),
        ("ao", "pulse,pipewire,alsa,"),
        // Ignore config files so system-wide /etc/mpv/mpv.conf (or
        // user-level) cannot silently set audio=no / aid=no.
        ("config", "no"),
        ("video-sync", "audio"),
        ("cache", "yes"),
        ("demuxer-max-bytes", "150MiB"),
        ("demuxer-max-back-bytes", "75MiB"),
        ("keep-open", "yes"),
        ("terminal", "yes"),
        ("msg-level", "all=info,ao=debug"),
    ]
}

/// Options for the separate-window fallback (vo=gpu, native OSC).
pub fn fallback_options() -> Vec<(&'static str, &'static str)> {
    vec![
        ("hwdec", "auto"),
        ("ao", "pulse,pipewire,alsa,"),
        ("config", "no"),
        ("video-sync", "display-resample"),
        ("cache", "yes"),
        ("demuxer-max-bytes", "150MiB"),
        ("demuxer-max-back-bytes", "75MiB"),
        ("keep-open", "yes"),
        ("terminal", "yes"),
        ("msg-level", "all=info,ao=debug"),
    ]
}

// Silence the `Weak` import warning if refactors ever remove the callback
// path (kept so the weak-ref intent is obvious at the import site).
const _: fn() = || {
    let _: Option<Weak<Inner>> = None;
};
