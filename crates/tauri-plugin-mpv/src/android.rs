/// Android-specific MPV embedding (covers Fire Stick + Android phones/tablets).
///
/// On Android, libmpv is loaded as a shared library (libmpv.so) from the
/// app's native library directory. The rendering uses OpenGL ES via a
/// SurfaceView.
///
/// Integration flow:
/// 1. Tauri's Android plugin hook provides the Activity reference.
/// 2. We create a SurfaceView via JNI and add it to the Activity's
///    content view below the WebView.
/// 3. The SurfaceHolder's Surface is passed to mpv's render context
///    using `MPV_RENDER_API_TYPE_OPENGL`.
/// 4. The WebView renders React controls on top with a transparent background.
///
/// Fire Stick specifics:
///   - Fire Stick is Android TV (API 22+, ARM v7/v8).
///   - Input is D-pad based (remote control), no touch.
///   - The React frontend handles D-pad focus navigation via
///     @noriginmedia/norigin-spatial-navigation or similar.
///   - MPV configured with `hwdec=mediacodec` for Amazon's MediaCodec
///     hardware decoder.
///
/// MPV configuration for Android:
///   - `hwdec=mediacodec` for hardware decoding
///   - `vo=gpu` with `gpu-api=opengl`
///   - `ao=opensles` or `ao=aaudio` (API 26+)

use std::ffi::c_void;

pub struct AndroidSurface {
    _surface_ptr: *mut c_void,
}

unsafe impl Send for AndroidSurface {}

impl AndroidSurface {
    pub fn new() -> Self {
        tracing::info!("Android MPV surface: placeholder (requires libmpv.so)");
        Self {
            _surface_ptr: std::ptr::null_mut(),
        }
    }
}

/// MPV options optimized for Android / Fire Stick playback.
pub fn android_mpv_options() -> Vec<(&'static str, &'static str)> {
    vec![
        ("hwdec", "mediacodec"),
        ("vo", "gpu"),
        ("gpu-api", "opengl"),
        ("ao", "opensles"),
        ("video-sync", "audio"),
        ("cache", "yes"),
        ("demuxer-max-bytes", "50MiB"),
        ("demuxer-max-back-bytes", "25MiB"),
    ]
}

/// Fire Stick-specific options (lower resolution decode ceiling, optimized buffers).
pub fn firestick_mpv_options() -> Vec<(&'static str, &'static str)> {
    let mut opts = android_mpv_options();
    opts.extend_from_slice(&[
        ("demuxer-max-bytes", "32MiB"),
        ("video-sync", "audio"),
    ]);
    opts
}

pub fn setup_android_surface() {
    let _surface = AndroidSurface::new();
    tracing::debug!("Android MPV surface setup complete (placeholder)");
}
