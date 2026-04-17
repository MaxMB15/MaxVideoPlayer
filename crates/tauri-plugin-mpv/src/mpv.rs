//! Thread-safe MPV plugin state.
//! Owns MpvEngine + the platform renderer, coordinates load/fallback.

pub use crate::engine::PlayerState;
use crate::engine::MpvEngine;
use crate::idle_inhibit::IdleInhibitor;
use crate::renderer::PlatformRenderer;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

#[cfg(target_os = "macos")]
use crate::macos::{embedded_options, fallback_options, MacosGlRenderer};

#[cfg(target_os = "linux")]
use crate::linux::{embedded_options as linux_embedded_options, fallback_options as linux_fallback_options, LinuxGlRenderer};

pub struct MpvState {
    inner: Mutex<MpvEngine>,
    renderer: Mutex<Option<Box<dyn PlatformRenderer>>>,
    fallback_active: AtomicBool,
    idle_inhibitor: IdleInhibitor,
}

impl MpvState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(MpvEngine::new()),
            renderer: Mutex::new(None),
            fallback_active: AtomicBool::new(false),
            idle_inhibitor: IdleInhibitor::new(),
        }
    }

    pub fn load<R: tauri::Runtime>(
        &self,
        url: &str,
        app: &tauri::AppHandle<R>,
    ) -> Result<(), String> {
        // Take the old renderer OUT of the mutex before dropping it.
        // detach() calls Queue::main().exec_sync(), which blocks the background thread
        // until the main thread processes the closure. The main thread's on_window_event
        // resize handler also needs the renderer mutex — holding the mutex while calling
        // exec_sync causes a deadlock. Dropping outside the lock avoids this.
        let old_renderer = self.renderer.lock().map_err(|e| e.to_string())?.take();
        drop(old_renderer); // calls detach() with renderer mutex RELEASED
        self.inner.lock().map_err(|e| e.to_string())?.stop();
        self.idle_inhibitor.uninhibit();
        self.fallback_active.store(false, Ordering::Release);

        let result = self.load_impl(url, app);
        if result.is_ok() {
            self.idle_inhibitor.inhibit();
        }
        result
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

        // Emit mpv://first-frame when the first video frame is rendered so the
        // frontend knows to make the WKWebView transparent.
        {
            use tauri::Emitter;
            let app_clone = app.clone();
            gl_renderer.set_first_frame_callback(Box::new(move || {
                let _ = app_clone.emit("mpv://first-frame", ());
            }));
        }

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

    #[cfg(target_os = "linux")]
    fn load_impl<R: tauri::Runtime>(
        &self,
        url: &str,
        app: &tauri::AppHandle<R>,
    ) -> Result<(), String> {
        let mut gl_renderer = match LinuxGlRenderer::new(app) {
            Ok(r) => r,
            Err(e) => return self.launch_fallback(url, app, &e),
        };

        {
            use tauri::Emitter;
            let app_clone = app.clone();
            gl_renderer.set_first_frame_callback(Box::new(move || {
                let _ = app_clone.emit("mpv://first-frame", ());
            }));
        }

        let attach_result = {
            let mut engine = self.inner.lock().map_err(|e| e.to_string())?;
            match engine.create(&linux_embedded_options()) {
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
        engine.configure_audio()?;
        engine.loadfile(url)?;
        engine.set_current_url(url);
        Ok(())
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
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

        self.launch_fallback_impl(url, reason)
    }

    #[cfg(target_os = "macos")]
    fn launch_fallback_impl(&self, url: &str, _reason: &str) -> Result<(), String> {
        let mut engine = self.inner.lock().map_err(|e| e.to_string())?;
        engine.create(&fallback_options())?;
        engine.loadfile(url)?;
        engine.set_current_url(url);
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn launch_fallback_impl(&self, url: &str, _reason: &str) -> Result<(), String> {
        let mut engine = self.inner.lock().map_err(|e| e.to_string())?;
        engine.create(&linux_fallback_options())?;
        engine.configure_audio()?;
        engine.loadfile(url)?;
        engine.set_current_url(url);
        Ok(())
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    fn launch_fallback_impl(&self, url: &str, _reason: &str) -> Result<(), String> {
        let mut engine = self.inner.lock().map_err(|e| e.to_string())?;
        engine.create(&[])?;
        engine.loadfile(url)?;
        engine.set_current_url(url);
        Ok(())
    }

    /// Reposition the video surface to a CSS-pixel rect reported by the frontend.
    pub fn set_visible(&self, visible: bool) {
        if let Ok(mut renderer) = self.renderer.lock() {
            if let Some(ref mut r) = *renderer {
                r.set_visible(visible);
            }
        }
    }

    pub fn set_bounds(&self, x: f64, y: f64, w: f64, h: f64) {
        if let Ok(mut renderer) = self.renderer.lock() {
            if let Some(ref mut r) = *renderer {
                r.set_frame(x, y, w, h);
            }
        }
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
        let result = self.inner.lock().map_err(|e| e.to_string())?.play();
        if result.is_ok() {
            self.idle_inhibitor.inhibit();
        }
        result
    }

    pub fn pause(&self) -> Result<(), String> {
        let result = self.inner.lock().map_err(|e| e.to_string())?.pause();
        if result.is_ok() {
            self.idle_inhibitor.uninhibit();
        }
        result
    }

    pub fn stop(&self) {
        let old_renderer = match self.renderer.lock() {
            Ok(mut r) => r.take(),
            Err(p) => p.into_inner().take(),
        };
        drop(old_renderer); // detach() runs synchronously on GLib main thread
        match self.inner.lock() {
            Ok(mut e) => e.stop(),
            Err(p) => p.into_inner().stop(),
        }
        self.idle_inhibitor.uninhibit();
    }

    pub fn seek(&self, position: f64) -> Result<(), String> {
        self.inner.lock().map_err(|e| e.to_string())?.seek(position)
    }

    pub fn set_volume(&self, volume: f64) -> Result<(), String> {
        self.inner.lock().map_err(|e| e.to_string())?.set_volume(volume)
    }

    pub fn sub_add(&self, path: &str) -> Result<(), String> {
        self.inner.lock().map_err(|e| e.to_string())?.sub_add(path)
    }

    pub fn sub_remove(&self, id: i64) -> Result<(), String> {
        self.inner.lock().map_err(|e| e.to_string())?.sub_remove(id)
    }

    pub fn set_sub_pos(&self, pos: f64) -> Result<(), String> {
        self.inner.lock().map_err(|e| e.to_string())?.set_sub_pos(pos)
    }

    pub fn set_sub_delay(&self, delay: f64) -> Result<(), String> {
        self.inner.lock().map_err(|e| e.to_string())?.set_sub_delay(delay)
    }

    pub fn get_state(&self) -> PlayerState {
        self.inner.lock().unwrap().get_state()
    }
}
