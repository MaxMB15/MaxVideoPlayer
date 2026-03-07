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

    /// Tear down the surface. Must be idempotent.
    fn detach(&mut self);
}
