//! MPV plugin state - wraps MpvEngine and exposes a thread-safe API.

pub use super::engine::{MpvEngine, PlayerState};
use std::sync::Mutex;

#[cfg(target_os = "macos")]
use {crate::desktop::MacosSurface, dispatch::Queue};

/// Managed state for the MPV plugin.
/// Holds the embedded LibMPV engine and optional video surface (macOS).
pub struct MpvState {
    inner: Mutex<MpvEngine>,
    #[cfg(target_os = "macos")]
    surface: Mutex<Option<MacosSurface>>,
}

impl MpvState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(MpvEngine::new()),
            #[cfg(target_os = "macos")]
            surface: Mutex::new(None),
        }
    }

    pub fn load<R: tauri::Runtime>(
        &self,
        url: &str,
        app: Option<&tauri::AppHandle<R>>,
    ) -> Result<(), String> {
        self.load_with_path(url, None, app)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn load_with_path<R: tauri::Runtime>(
        &self,
        url: &str,
        mpv_path: Option<std::path::PathBuf>,
        _app: Option<&tauri::AppHandle<R>>,
    ) -> Result<(), String> {
        // Windows/Linux: vo=libmpv requires render context (macOS only for now).
        // Use vo=gpu which opens a separate window until embedded support is added.
        self.inner
            .lock()
            .map_err(|e| e.to_string())?
            .load_with_path(url, mpv_path)
    }

    #[cfg(target_os = "macos")]
    pub fn load_with_path<R: tauri::Runtime>(
        &self,
        url: &str,
        _mpv_path: Option<std::path::PathBuf>,
        app: Option<&tauri::AppHandle<R>>,
    ) -> Result<(), String> {
        if let Some(app) = app {
            let mut surf = self.surface.lock().map_err(|e| e.to_string())?;
            if surf.is_none() {
                // NSOpenGLContext/NSOpenGLView MUST be created on the main thread.
                // mpv_load runs on tokio worker, so we dispatch synchronously to main.
                let app = app.clone();
                let surface = Queue::main().exec_sync(|| MacosSurface::create(&app, "main"))?;
                *surf = Some(surface);
                tracing::info!("[MPV] created video surface");
            }
            drop(surf);
        }
        let guard = self.surface.lock().map_err(|e| e.to_string())?;
        let surf = guard
            .as_ref()
            .ok_or_else(|| "No video surface".to_string())?;
        self.inner
            .lock()
            .map_err(|e| e.to_string())?
            .load_with_surface(url, surf)?;
        Ok(())
    }

    pub fn play(&self) -> Result<(), String> {
        self.inner.lock().map_err(|e| e.to_string())?.play()
    }

    pub fn pause(&self) -> Result<(), String> {
        self.inner.lock().map_err(|e| e.to_string())?.pause()
    }

    pub fn stop(&self) {
        self.inner.lock().unwrap().stop();
    }

    pub fn seek(&self, position: f64) -> Result<(), String> {
        self.inner.lock().map_err(|e| e.to_string())?.seek(position)
    }

    pub fn set_volume(&self, volume: f64) -> Result<(), String> {
        self.inner.lock().map_err(|e| e.to_string())?.set_volume(volume)
    }

    pub fn get_state(&self) -> PlayerState {
        self.inner.lock().unwrap().get_state()
    }

    /// Access the engine for render setup (macOS). Use with care - holds the lock.
    pub fn engine(&self) -> std::sync::MutexGuard<'_, MpvEngine> {
        self.inner.lock().unwrap()
    }

    /// Update the macOS video surface frame (e.g. on window resize). No-op if no surface.
    #[cfg(target_os = "macos")]
    pub fn update_surface_frame(&self) {
        if let Ok(guard) = self.surface.lock() {
            if let Some(ref surf) = *guard {
                surf.update_frame();
            }
        }
    }
}
