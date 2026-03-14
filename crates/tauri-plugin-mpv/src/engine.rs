//! Platform-agnostic libmpv instance management.
//! Options (vo, hwdec, ao) are passed in by the caller; this file has zero #[cfg] blocks.

use libmpv2::Mpv;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    pub is_playing: bool,
    pub is_paused: bool,
    pub current_url: Option<String>,
    pub volume: f64,
    pub position: f64,
    pub duration: f64,
}

pub struct MpvEngine {
    mpv: Option<Mpv>,
    current_url: Option<String>,
}

impl MpvEngine {
    pub fn new() -> Self {
        Self { mpv: None, current_url: None }
    }

    /// Create a new Mpv instance with the provided options.
    /// Drops any existing instance first (calls stop()).
    /// Returns a mutable reference so the caller can attach a render context
    /// before calling `loadfile`.
    pub fn create(&mut self, options: &[(&str, &str)]) -> Result<&mut Mpv, String> {
        self.stop();
        let opts: Vec<(String, String)> = options
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let mpv = Mpv::with_initializer(move |init| {
            for (k, v) in &opts {
                init.set_option(k.as_str(), v.as_str())?;
            }
            Ok(())
        })
        .map_err(|e| format!("mpv init: {}", e))?;
        self.mpv = Some(mpv);
        Ok(self.mpv.as_mut().unwrap())
    }

    /// Issue the loadfile command. Must be called AFTER render context is attached.
    pub fn loadfile(&self, url: &str) -> Result<(), String> {
        let mpv = self.mpv.as_ref().ok_or("no mpv instance")?;
        mpv.command("loadfile", &[url, "replace"])
            .map_err(|e| format!("loadfile: {}", e))
    }

    /// Record the current URL (called by MpvState after loadfile succeeds).
    pub fn set_current_url(&mut self, url: &str) {
        self.current_url = Some(url.to_string());
    }

    /// Stop playback and destroy the mpv instance.
    pub fn stop(&mut self) {
        if let Some(ref mpv) = self.mpv {
            let _ = mpv.command("stop", &[]);
        }
        self.mpv = None;
        self.current_url = None;
    }

    pub fn play(&self) -> Result<(), String> {
        self.mpv
            .as_ref()
            .ok_or_else(|| "no mpv instance".to_string())?
            .set_property("pause", false)
            .map_err(|e| e.to_string())
    }

    pub fn pause(&self) -> Result<(), String> {
        self.mpv
            .as_ref()
            .ok_or_else(|| "no mpv instance".to_string())?
            .set_property("pause", true)
            .map_err(|e| e.to_string())
    }

    pub fn seek(&self, position: f64) -> Result<(), String> {
        self.mpv
            .as_ref()
            .ok_or_else(|| "no mpv instance".to_string())?
            .command("seek", &[&position.to_string(), "absolute"])
            .map_err(|e| e.to_string())
    }

    pub fn set_volume(&self, volume: f64) -> Result<(), String> {
        let v = volume.clamp(0.0, 150.0);
        self.mpv
            .as_ref()
            .ok_or_else(|| "no mpv instance".to_string())?
            .set_property("volume", v)
            .map_err(|e| e.to_string())
    }

    /// Add a subtitle track from a local file path.
    pub fn sub_add(&self, path: &str) -> Result<(), String> {
        self.mpv
            .as_ref()
            .ok_or_else(|| "no mpv instance".to_string())?
            .command("sub-add", &[path, "select"])
            .map_err(|e| e.to_string())
    }

    /// Remove all external subtitle tracks (pass index -1 to remove all).
    pub fn sub_remove(&self, id: i64) -> Result<(), String> {
        self.mpv
            .as_ref()
            .ok_or_else(|| "no mpv instance".to_string())?
            .command("sub-remove", &[&id.to_string()])
            .map_err(|e| e.to_string())
    }

    pub fn get_state(&self) -> PlayerState {
        let mut state = PlayerState {
            current_url: self.current_url.clone(),
            volume: 100.0,
            ..Default::default()
        };
        if let Some(ref mpv) = self.mpv {
            state.position = mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
            state.duration = mpv.get_property::<f64>("duration").unwrap_or(0.0);
            state.is_paused = mpv.get_property::<bool>("pause").unwrap_or(false);
            state.volume = mpv.get_property::<f64>("volume").unwrap_or(100.0);
            state.is_playing = !state.is_paused && state.current_url.is_some();
        }
        state
    }
}
