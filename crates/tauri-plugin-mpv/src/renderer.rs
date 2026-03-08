//! PlatformRenderer trait — the contract each OS must implement.
//! All platform code lives in its own file (macos.rs / windows.rs / etc.)
//! and is selected at compile time via #[cfg]. This file has no #[cfg] blocks.

use libmpv2::Mpv;

/// Handles embedding the libmpv video surface into the native window.
///
/// Implementations:
/// - macOS   → `macos::MacosGlRenderer`  (NSOpenGLView + OpenGL Core 3.2 render context)
/// - Windows → `windows::WindowsRenderer` (stub; HWND wid works on Win32)
/// - iOS     → `ios::IosRenderer`         (stub)
/// - Android → `android::AndroidRenderer` (stub)
pub trait PlatformRenderer: Send + Sync {
    /// Set up the video surface and attach libmpv's render context.
    ///
    /// Called once per stream load. `mpv` has been created with platform
    /// options (vo=libmpv, hwdec, audio) but `loadfile` has NOT been called yet.
    /// On success the renderer pumps frames until `detach` is called.
    fn attach(&mut self, mpv: &mut Mpv) -> Result<(), String>;

    /// Update render surface dimensions on window resize.
    fn resize(&mut self, width: u32, height: u32);

    /// Reposition and resize the video surface to a specific rect (in CSS/logical pixels).
    /// Called by the frontend to align the NSOpenGLView with the player content area.
    fn set_frame(&mut self, x: f64, y: f64, w: f64, h: f64);

    /// Show or hide the video surface without stopping playback.
    fn set_visible(&mut self, visible: bool);

    /// Tear down the surface. Must be idempotent.
    fn detach(&mut self);
}
