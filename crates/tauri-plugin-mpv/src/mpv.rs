//! Thread-safe MPV plugin state.
//! Owns MpvEngine + the platform renderer, coordinates load/fallback.

pub use crate::engine::PlayerState;
use crate::engine::MpvEngine;
use crate::renderer::PlatformRenderer;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

#[cfg(target_os = "macos")]
use crate::macos::{embedded_options, fallback_options, MacosGlRenderer};

pub struct MpvState {
    inner: Mutex<MpvEngine>,
    renderer: Mutex<Option<Box<dyn PlatformRenderer>>>,
    fallback_active: AtomicBool,
}

impl MpvState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(MpvEngine::new()),
            renderer: Mutex::new(None),
            fallback_active: AtomicBool::new(false),
        }
    }

    pub fn load<R: tauri::Runtime>(
        &self,
        url: &str,
        app: &tauri::AppHandle<R>,
    ) -> Result<(), String> {
        // Teardown any existing renderer + engine before starting fresh.
        {
            let mut renderer = self.renderer.lock().map_err(|e| e.to_string())?;
            *renderer = None;
        }
        self.inner.lock().map_err(|e| e.to_string())?.stop();
        self.fallback_active.store(false, Ordering::Release);

        self.load_impl(url, app)
    }

    #[cfg(target_os = "macos")]
    fn load_impl<R: tauri::Runtime>(
        &self,
        url: &str,
        app: &tauri::AppHandle<R>,
    ) -> Result<(), String> {
        // Create the NSOpenGLView renderer (main-thread work happens inside new()).
        let mut gl_renderer = match MacosGlRenderer::new(app) {
            Ok(r) => r,
            Err(e) => return self.launch_fallback(url, app, &e),
        };

        // Create mpv with embedded options and attach the renderer.
        let attach_result = {
            let mut engine = self.inner.lock().map_err(|e| e.to_string())?;
            match engine.create(&embedded_options()) {
                Ok(mpv) => gl_renderer.attach(mpv),
                Err(e) => Err(e),
            }
        };

        if let Err(e) = attach_result {
            self.inner.lock().map_err(|e| e.to_string())?.stop();
            return self.launch_fallback(url, app, &e);
        }

        {
            let mut renderer = self.renderer.lock().map_err(|e| e.to_string())?;
            *renderer = Some(Box::new(gl_renderer));
        }

        let mut engine = self.inner.lock().map_err(|e| e.to_string())?;
        engine.loadfile(url)?;
        engine.set_current_url(url);
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    fn load_impl<R: tauri::Runtime>(
        &self,
        url: &str,
        _app: &tauri::AppHandle<R>,
    ) -> Result<(), String> {
        let mut engine = self.inner.lock().map_err(|e| e.to_string())?;
        engine.create(&[])?;
        engine.loadfile(url)?;
        engine.set_current_url(url);
        Ok(())
    }

    fn launch_fallback<R: tauri::Runtime>(
        &self,
        url: &str,
        app: &tauri::AppHandle<R>,
        reason: &str,
    ) -> Result<(), String> {
        use tauri::Emitter;
        tracing::warn!(
            "[MPV] embedded renderer failed ({}), launching fallback window",
            reason
        );
        self.fallback_active.store(true, Ordering::Release);
        let _ = app.emit("mpv://render-fallback", serde_json::json!({ "reason": reason }));

        self.launch_fallback_impl(url)
    }

    #[cfg(target_os = "macos")]
    fn launch_fallback_impl(&self, url: &str) -> Result<(), String> {
        let mut engine = self.inner.lock().map_err(|e| e.to_string())?;
        engine.create(&fallback_options())?;
        engine.loadfile(url)?;
        engine.set_current_url(url);
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    fn launch_fallback_impl(&self, url: &str) -> Result<(), String> {
        let mut engine = self.inner.lock().map_err(|e| e.to_string())?;
        engine.create(&[])?;
        engine.loadfile(url)?;
        engine.set_current_url(url);
        Ok(())
    }

    /// Forward a window resize to the active renderer (e.g. from Tauri WindowEvent::Resized).
    pub fn resize(&self, width: u32, height: u32) {
        if let Ok(mut renderer) = self.renderer.lock() {
            if let Some(ref mut r) = *renderer {
                r.resize(width, height);
            }
        }
    }

    pub fn play(&self) -> Result<(), String> {
        self.inner.lock().map_err(|e| e.to_string())?.play()
    }

    pub fn pause(&self) -> Result<(), String> {
        self.inner.lock().map_err(|e| e.to_string())?.pause()
    }

    pub fn stop(&self) {
        {
            let mut renderer = self.renderer.lock().unwrap();
            *renderer = None;
        }
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
}
