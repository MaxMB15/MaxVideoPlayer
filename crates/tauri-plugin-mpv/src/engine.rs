//! LibMPV engine using libmpv2 - embedded playback via render API.
//! Configured with vo=libmpv for custom rendering (no default video output).

use libmpv2::{mpv_error, Mpv};
use serde::{Deserialize, Serialize};

pub(crate) fn format_mpv_error(e: &libmpv2::Error) -> String {
    use libmpv2::Error;
    match e {
        Error::Raw(code) => {
            let msg = match *code {
                mpv_error::Unsupported => "Unsupported (vo=libmpv or format)".to_string(),
                mpv_error::VoInitFailed => "Video output init failed".to_string(),
                mpv_error::LoadingFailed => "Loading stream/file failed".to_string(),
                mpv_error::AoInitFailed => "Audio output init failed".to_string(),
                mpv_error::NothingToPlay => "No video/audio in stream".to_string(),
                mpv_error::OptionError => "Invalid option".to_string(),
                mpv_error::OptionNotFound => "Option not found".to_string(),
                _ => format!("MPV error (code {})", code),
            };
            msg
        }
        other => other.to_string(),
    }
}
use std::sync::Mutex;

#[cfg(target_os = "macos")]
use {crate::desktop::{macos_mpv_options, MacosRenderJob}, dispatch::Queue};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    pub is_playing: bool,
    pub is_paused: bool,
    pub current_url: Option<String>,
    pub volume: f64,
    pub position: f64,
    pub duration: f64,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            is_playing: false,
            is_paused: false,
            current_url: None,
            volume: 100.0,
            position: 0.0,
            duration: 0.0,
        }
    }
}

/// LibMPV engine - uses libmpv2 property/command API.
/// Configured for embedded rendering (vo=libmpv).
pub struct MpvEngine {
    mpv: Mutex<Option<Mpv>>,
    current_url: Mutex<Option<String>>,
    #[cfg(target_os = "macos")]
    render_job: Mutex<Option<MacosRenderJob>>,
}

impl MpvEngine {
    pub fn new() -> Self {
        Self {
            mpv: Mutex::new(None),
            current_url: Mutex::new(None),
            #[cfg(target_os = "macos")]
            render_job: Mutex::new(None),
        }
    }

    fn with_mpv<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(&Mpv) -> Result<T, String>,
    {
        let guard = self.mpv.lock().map_err(|e| e.to_string())?;
        let mpv = guard.as_ref().ok_or_else(|| "No MPV instance".to_string())?;
        f(mpv)
    }

    fn with_mpv_mut<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(&mut Option<Mpv>) -> Result<T, String>,
    {
        let mut guard = self.mpv.lock().map_err(|e| e.to_string())?;
        f(&mut *guard)
    }

    /// Create MPV with vo=gpu and wid for embedded playback (macOS native embedding).
    #[cfg(target_os = "macos")]
    fn create_mpv_with_wid(wid: i64) -> Result<Mpv, String> {
        let mpv = Mpv::with_initializer(move |init| {
            for (k, v) in crate::desktop::macos_mpv_options_for_gpu() {
                init.set_option(k, v)?;
            }
            init.set_option("vo", "gpu")?;
            init.set_option("wid", wid)?;
            Ok(())
        })
        .map_err(|e| format_mpv_error(&e))?;
        Ok(mpv)
    }

    /// Create and initialize MPV. `vo` controls video output: "libmpv" for embedded, "gpu" for separate window.
    fn create_mpv_with_vo(vo: &str) -> Result<Mpv, String> {
        let vo = vo.to_string();
        let mpv = Mpv::with_initializer(move |init| {
            #[cfg(target_os = "macos")]
            {
                for (k, v) in macos_mpv_options() {
                    init.set_option(k, v)?;
                }
                init.set_option("vo", vo.as_str())?;
            }
            #[cfg(not(target_os = "macos"))]
            {
                init.set_option("vo", vo.as_str())?;
                init.set_option("hwdec", "auto")?;
                #[cfg(target_os = "windows")]
                init.set_option("ao", "wasapi")?;
            }
            Ok(())
        })
        .map_err(|e| format_mpv_error(&e))?;
        Ok(mpv)
    }

    #[cfg(not(target_os = "macos"))]
    fn create_mpv() -> Result<Mpv, String> {
        Self::create_mpv_with_vo("gpu")
    }

    /// Load a URL for playback.
    #[cfg(not(target_os = "macos"))]
    pub fn load(&self, url: &str) -> Result<(), String> {
        self.stop();

        let mpv = Self::create_mpv()?;
        mpv.command("loadfile", &[url, "replace"])
            .map_err(|e| format_mpv_error(&e))?;

        *self.mpv.lock().map_err(|e| e.to_string())? = Some(mpv);
        *self.current_url.lock().map_err(|e| e.to_string())? = Some(url.to_string());

        tracing::info!("[MPV engine] loaded url={}", url);
        Ok(())
    }

    /// Load with macOS surface. Tries vo=libmpv (embedded) first; falls back to vo=gpu (separate window) on Unsupported.
    #[cfg(target_os = "macos")]
    pub fn load_with_surface(
        &self,
        url: &str,
        surface: &crate::desktop::MacosSurface,
    ) -> Result<(), String> {
        self.stop();

        let result = (|| -> Result<(), String> {
            let mut mpv =
                Self::create_mpv_with_vo("libmpv")
                    .map_err(|e| format!("create_mpv(vo=libmpv): {}", e))?;
            tracing::debug!("[MPV engine] mpv handle created, creating render context...");

            let job = Queue::main()
                .exec_sync(|| MacosRenderJob::new(&mut mpv, surface))
                .map_err(|e| format!("create_render_context: {}", e))?;
            tracing::debug!("[MPV engine] render context ready, loading file...");

            mpv.command("loadfile", &[url, "replace"])
                .map_err(|e| format!("loadfile: {}", format_mpv_error(&e)))?;

            *self.mpv.lock().map_err(|e| e.to_string())? = Some(mpv);
            *self.render_job.lock().map_err(|e| e.to_string())? = Some(job);
            *self.current_url.lock().map_err(|e| e.to_string())? = Some(url.to_string());
            Ok(())
        })();

        match result {
            Ok(()) => {
                tracing::info!("[MPV engine] loaded url={} (embedded)", url);
                Ok(())
            }
            Err(e)
                if e.contains("Unsupported")
                    || e.contains("vo=libmpv")
                    || e.contains("code -18")
                    || e.contains("create_render_context")
                    || e.contains("Video output init failed") =>
            {
                tracing::warn!(
                    "[MPV engine] render API failed ({}), trying wid embedding",
                    e
                );
                match self.load_with_wid(url, surface) {
                    Ok(()) => Ok(()),
                    Err(wid_err) => {
                        tracing::warn!(
                            "[MPV engine] wid embedding failed ({}), falling back to separate window",
                            wid_err
                        );
                        self.load_with_vo_gpu(url)
                    }
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Load with vo=gpu + wid (embedded in our view). Fallback when vo=libmpv render API fails.
    #[cfg(target_os = "macos")]
    fn load_with_wid(
        &self,
        url: &str,
        surface: &crate::desktop::MacosSurface,
    ) -> Result<(), String> {
        self.stop();

        let wid = surface.wid() as i64;
        let mpv = Self::create_mpv_with_wid(wid)?;
        mpv.command("loadfile", &[url, "replace"])
            .map_err(|e| format_mpv_error(&e))?;

        *self.mpv.lock().map_err(|e| e.to_string())? = Some(mpv);
        *self.current_url.lock().map_err(|e| e.to_string())? = Some(url.to_string());

        tracing::info!("[MPV engine] loaded url={} (embedded via wid)", url);
        Ok(())
    }

    /// Load with vo=gpu (separate window). Last resort when wid embedding fails.
    #[cfg(target_os = "macos")]
    fn load_with_vo_gpu(&self, url: &str) -> Result<(), String> {
        self.stop();

        let mpv = Self::create_mpv_with_vo("gpu")?;
        mpv.command("loadfile", &[url, "replace"])
            .map_err(|e| format_mpv_error(&e))?;

        *self.mpv.lock().map_err(|e| e.to_string())? = Some(mpv);
        *self.current_url.lock().map_err(|e| e.to_string())? = Some(url.to_string());

        tracing::info!("[MPV engine] loaded url={} (separate window)", url);
        Ok(())
    }

    /// Load with optional path (for API compatibility; path ignored when using libmpv).
    #[cfg(not(target_os = "macos"))]
    pub fn load_with_path(&self, url: &str, _mpv_path: Option<std::path::PathBuf>) -> Result<(), String> {
        self.load(url)
    }

    pub fn play(&self) -> Result<(), String> {
        self.with_mpv(|mpv| {
            mpv.set_property("pause", false).map_err(|e| e.to_string())
        })
    }

    pub fn pause(&self) -> Result<(), String> {
        self.with_mpv(|mpv| mpv.set_property("pause", true).map_err(|e| e.to_string()))
    }

    pub fn stop(&self) {
        #[cfg(target_os = "macos")]
        {
            *self.render_job.lock().unwrap() = None;
        }
        let _ = self.with_mpv_mut(|opt| {
            *opt = None;
            *self.current_url.lock().map_err(|e| e.to_string())? = None;
            Ok(())
        });
    }

    pub fn seek(&self, position: f64) -> Result<(), String> {
        self.with_mpv(|mpv| {
            mpv.command("seek", &[&position.to_string(), "absolute"])
                .map_err(|e| e.to_string())
        })
    }

    pub fn set_volume(&self, volume: f64) -> Result<(), String> {
        let v = volume.clamp(0.0, 150.0);
        self.with_mpv(|mpv| mpv.set_property("volume", v).map_err(|e| e.to_string()))
    }

    pub fn get_state(&self) -> PlayerState {
        let mut state = PlayerState {
            current_url: self
                .current_url
                .lock()
                .map(|g| g.clone())
                .unwrap_or(None),
            ..Default::default()
        };

        if let Ok(guard) = self.mpv.lock() {
            if let Some(ref mpv) = *guard {
                state.position = mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
                state.duration = mpv.get_property::<f64>("duration").unwrap_or(0.0);
                state.is_paused = mpv.get_property::<bool>("pause").unwrap_or(false);
                state.volume = mpv.get_property::<f64>("volume").unwrap_or(100.0);
                state.is_playing = !state.is_paused && state.current_url.is_some();
            }
        }

        state
    }

}
