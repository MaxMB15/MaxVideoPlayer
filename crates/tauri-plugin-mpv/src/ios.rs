/// iOS-specific MPV embedding.
///
/// On iOS, libmpv is statically linked (libmpv.a compiled for arm64).
/// The rendering pipeline uses Metal via `mpv_render_context_create` with
/// `MPV_RENDER_API_TYPE_SW` (software) or a Metal CAMetalLayer.
///
/// Integration flow:
/// 1. Tauri's iOS plugin hook provides the root UIViewController.
/// 2. We create a UIView, insert it below the WKWebView in the view hierarchy.
/// 3. A CAMetalLayer is attached to the UIView for GPU-accelerated rendering.
/// 4. The mpv render context draws into the Metal layer each frame.
/// 5. The WKWebView sits on top with a transparent background, rendering
///    the React control overlay.
///
/// MPV configuration for iOS:
///   - `hwdec=videotoolbox` for hardware decoding via Apple's VideoToolbox
///   - `vo=gpu` with `gpu-api=auto` (Metal on iOS 13+)
///   - Audio output via `ao=coreaudio`

use std::ffi::c_void;

/// Opaque handle representing the iOS video surface.
/// Wraps a pointer to the UIView used for MPV rendering.
pub struct IosSurface {
    _view_ptr: *mut c_void,
}

unsafe impl Send for IosSurface {}

impl IosSurface {
    /// Create a new iOS surface.
    ///
    /// In the full implementation this receives the UIViewController from
    /// Tauri's iOS plugin API, creates a UIView + CAMetalLayer, and inserts
    /// it into the view hierarchy beneath the WKWebView.
    pub fn new() -> Self {
        tracing::info!("iOS MPV surface: placeholder (requires libmpv.a)");
        Self {
            _view_ptr: std::ptr::null_mut(),
        }
    }
}

/// MPV options optimized for iOS playback.
pub fn ios_mpv_options() -> Vec<(&'static str, &'static str)> {
    vec![
        ("hwdec", "videotoolbox"),
        ("vo", "gpu"),
        ("gpu-api", "auto"),
        ("ao", "coreaudio"),
        ("video-sync", "display-resample"),
        ("interpolation", "yes"),
        ("cache", "yes"),
        ("demuxer-max-bytes", "50MiB"),
        ("demuxer-max-back-bytes", "25MiB"),
    ]
}

pub fn setup_ios_surface() {
    let _surface = IosSurface::new();
    tracing::debug!("iOS MPV surface setup complete (placeholder)");
}
