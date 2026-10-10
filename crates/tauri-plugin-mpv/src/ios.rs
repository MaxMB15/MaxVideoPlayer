// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Max Boksem. See NOTICE for additional terms under GPLv3 section 7.

//! iOS video output.
//!
//! mpv draws straight into a `CAMetalLayer` through MoltenVK
//! (`vo=gpu-next`, `gpu-context=moltenvk`), with the layer pointer passed as
//! `wid`. mpv owns the draw loop, so there's no render context to drive like
//! on macOS and Linux. The layer belongs to a UIView that the plugin's Swift
//! package (`ios/Sources/MpvPlugin.swift`) keeps under the web view.
//!
//! The Swift side also runs the audio session, Now Playing and the lock
//! screen controls. Rust calls it through the `mvp_ios_*` functions, and it
//! calls back through the two C callbacks registered in [`install`].

use crate::mpv::MpvState;
use crate::renderer::PlatformRenderer;
use libmpv2::{
    events::{Event, PropertyData},
    Format, Mpv,
};
use std::ffi::{c_char, CString};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, OnceLock,
};
use tauri::{AppHandle, Manager, Runtime};

extern "C" {
    fn mvp_ios_register_callbacks(
        lifecycle: Option<extern "C" fn(bool)>,
        remote: Option<extern "C" fn(i32, f64)>,
    );
    fn mvp_ios_surface_create() -> usize;
    fn mvp_ios_surface_set_frame(x: f64, y: f64, width: f64, height: f64);
    fn mvp_ios_surface_set_visible(visible: bool);
    fn mvp_ios_surface_detach();
    fn mvp_ios_set_idle_timer_disabled(disabled: bool);
    fn mvp_ios_set_audio_active(active: bool);
    fn mvp_ios_now_playing(title: *const c_char, duration: f64, position: f64, playing: bool);
    fn mvp_ios_now_playing_clear();
}

/// Remote command codes from `MvpRemoteCommand` in `MpvPlugin.swift`.
const REMOTE_PLAY: i32 = 0;
const REMOTE_PAUSE: i32 = 1;
const REMOTE_TOGGLE: i32 = 2;
const REMOTE_SKIP: i32 = 3;
const REMOTE_SEEK_TO: i32 = 4;

/// mpv options for the Metal layer. The caller adds `wid`, which changes
/// with every layer.
pub fn embedded_options() -> Vec<(&'static str, &'static str)> {
    vec![
        ("vo", "gpu-next"),
        ("gpu-api", "vulkan"),
        ("gpu-context", "moltenvk"),
        ("hwdec", "videotoolbox"),
        ("hwdec-codecs", "all"),
        ("video-rotate", "no"),
        ("profile", "fast"),
        ("cache", "yes"),
        ("cache-secs", "10"),
        ("demuxer-max-bytes", "50MiB"),
        ("demuxer-max-back-bytes", "25MiB"),
        ("stream-lavf-o", "reconnect=1,reconnect_streamed=1,reconnect_delay_max=5"),
        ("network-timeout", "30"),
        // Keep the last frame at EOF so the frontend can show its controls,
        // same as desktop.
        ("keep-open", "yes"),
        ("terminal", "no"),
        ("input-default-bindings", "no"),
        // No `ytdl` or `osc`: MPVKit builds mpv for iOS without Lua, and
        // those options only exist with it. Setting one fails init with
        // MPV_ERROR_OPTION_NOT_FOUND.
    ]
}

/// Called once from plugin setup. Hands Swift the callbacks for the app
/// lifecycle and the lock screen controls, which need the app's `MpvState`.
pub fn install<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    let with_state: StateFn = Box::new(move |f: &dyn Fn(&MpvState)| f(&app.state::<MpvState>()));
    if HANDLERS.set(with_state).is_err() {
        return;
    }
    unsafe { mvp_ios_register_callbacks(Some(on_lifecycle), Some(on_remote)) };
}

type StateFn = Box<dyn Fn(&dyn Fn(&MpvState)) + Send + Sync>;

static HANDLERS: OnceLock<StateFn> = OnceLock::new();

/// Runs `f` with the app's `MpvState` on a new thread. The callbacks arrive
/// on the main thread, and an mpv call can wait on a lock held by a load.
fn with_state_async(f: impl Fn(&MpvState) + Send + 'static) {
    let spawned = std::thread::Builder::new()
        .name("mpv-ios-callback".into())
        .spawn(move || {
            if let Some(with_state) = HANDLERS.get() {
                with_state(&f);
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("[MPV ios] failed to spawn callback thread: {e}");
    }
}

/// iOS stops apps that use the GPU in the background, so video output is
/// off while the app is in the background. Audio keeps playing.
extern "C" fn on_lifecycle(background: bool) {
    with_state_async(move |state| state.set_video_enabled(!background));
}

extern "C" fn on_remote(command: i32, value: f64) {
    with_state_async(move |state| {
        let result = match command {
            REMOTE_PLAY => state.play(),
            REMOTE_PAUSE => state.pause(),
            REMOTE_TOGGLE if state.get_state().is_paused => state.play(),
            REMOTE_TOGGLE => state.pause(),
            REMOTE_SKIP => state.seek_relative(value),
            REMOTE_SEEK_TO => state.seek(value),
            _ => Ok(()),
        };
        if let Err(e) = result {
            tracing::warn!("[MPV ios] remote command {command} failed: {e}");
        }
    });
}

pub fn set_idle_timer_disabled(disabled: bool) {
    unsafe { mvp_ios_set_idle_timer_disabled(disabled) };
}

/// Turns the audio session on for playback, or off along with the lock
/// screen's Now Playing info when playback stops.
pub fn set_audio_active(active: bool) {
    unsafe { mvp_ios_set_audio_active(active) };
    if !active {
        unsafe { mvp_ios_now_playing_clear() };
    }
}

/// The Metal view under the web view. The view outlives this value: Swift
/// reuses it for the next stream and only hides it on detach.
pub struct IosMetalRenderer {
    layer: usize,
    /// In a mutex only so the renderer is `Sync`.
    first_frame_cb: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    /// Stops the watcher thread, whose client handle keeps the mpv core
    /// alive. Tripped in `detach`, before the engine stops.
    watcher_kill: Option<Arc<AtomicBool>>,
    detached: bool,
}

impl IosMetalRenderer {
    pub fn new() -> Result<Self, String> {
        let layer = unsafe { mvp_ios_surface_create() };
        if layer == 0 {
            return Err("the web view isn't ready for video yet".into());
        }
        Ok(Self {
            layer,
            first_frame_cb: Mutex::new(None),
            watcher_kill: None,
            detached: false,
        })
    }

    /// The value for mpv's `wid` option, the `CAMetalLayer` pointer.
    pub fn wid(&self) -> String {
        self.layer.to_string()
    }
}

impl PlatformRenderer for IosMetalRenderer {
    /// mpv already has the layer through `wid`. This only starts the thread
    /// that reports the first frame and keeps Now Playing up to date.
    fn attach(&mut self, mpv: &mut Mpv) -> Result<(), String> {
        let client = mpv
            .create_client(Some("ios-watcher"))
            .map_err(|e| format!("mpv_create_client: {e}"))?;
        let kill = Arc::new(AtomicBool::new(false));
        self.watcher_kill = Some(kill.clone());
        let first_frame = self
            .first_frame_cb
            .get_mut()
            .unwrap_or_else(|p| p.into_inner())
            .take();
        std::thread::Builder::new()
            .name("mpv-ios-watcher".into())
            .spawn(move || run_watcher(client, kill, first_frame))
            .map_err(|e| format!("spawn watcher: {e}"))?;
        Ok(())
    }

    /// The view follows `set_frame` and its own layout, so a window resize
    /// needs nothing here.
    fn resize(&mut self, _width: u32, _height: u32) {}

    fn set_frame(&mut self, x: f64, y: f64, w: f64, h: f64) {
        unsafe { mvp_ios_surface_set_frame(x, y, w, h) };
    }

    fn set_visible(&mut self, visible: bool) {
        unsafe { mvp_ios_surface_set_visible(visible) };
    }

    fn set_first_frame_callback(&mut self, cb: Box<dyn FnOnce() + Send>) {
        *self.first_frame_cb.get_mut().unwrap_or_else(|p| p.into_inner()) = Some(cb);
    }

    fn detach(&mut self) {
        if self.detached {
            return;
        }
        self.detached = true;
        if let Some(kill) = self.watcher_kill.take() {
            kill.store(true, Ordering::Release);
        }
        unsafe { mvp_ios_surface_detach() };
    }
}

impl Drop for IosMetalRenderer {
    fn drop(&mut self) {
        self.detach();
    }
}

const OBSERVE_VO_CONFIGURED: u64 = 1;
const OBSERVE_PAUSE: u64 = 2;
const OBSERVE_DURATION: u64 = 3;
const OBSERVE_TITLE: u64 = 4;

/// Waits for the first video frame, then keeps the lock screen's Now Playing
/// info in step with pause, seeks, duration and title.
fn run_watcher(
    mut client: Mpv,
    kill: Arc<AtomicBool>,
    mut first_frame: Option<Box<dyn FnOnce() + Send>>,
) {
    let _ = client.disable_deprecated_events();
    let observed = [
        ("vo-configured", Format::Flag, OBSERVE_VO_CONFIGURED),
        ("pause", Format::Flag, OBSERVE_PAUSE),
        ("duration", Format::Double, OBSERVE_DURATION),
        ("media-title", Format::String, OBSERVE_TITLE),
    ];
    for (name, format, id) in observed {
        if let Err(e) = client.observe_property(name, format, id) {
            tracing::warn!("[MPV ios] observe {name} failed: {e}");
        }
    }

    let mut paused = false;
    let mut duration = 0.0;
    let mut title: Option<CString> = None;

    while !kill.load(Ordering::Acquire) {
        let changed = match client.wait_event(0.5) {
            Some(Ok(Event::Shutdown)) => break,
            Some(Ok(Event::PropertyChange { name, change, .. })) => match (name, change) {
                ("vo-configured", PropertyData::Flag(true)) => {
                    if let Some(cb) = first_frame.take() {
                        cb();
                    }
                    false
                }
                ("pause", PropertyData::Flag(p)) => {
                    paused = p;
                    true
                }
                ("duration", PropertyData::Double(d)) => {
                    duration = d;
                    true
                }
                ("media-title", PropertyData::Str(t)) => {
                    title = CString::new(t).ok();
                    true
                }
                _ => false,
            },
            Some(Ok(Event::PlaybackRestart)) => true,
            _ => false,
        };
        if changed {
            let position = client.get_property::<f64>("time-pos").unwrap_or(0.0);
            let title_ptr = title.as_ref().map_or(std::ptr::null(), |t| t.as_ptr());
            unsafe { mvp_ios_now_playing(title_ptr, duration, position, !paused) };
        }
    }
}
