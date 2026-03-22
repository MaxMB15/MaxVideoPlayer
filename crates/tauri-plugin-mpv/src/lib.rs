#![allow(dead_code)]

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

mod commands;
mod engine;
pub mod mpv;
mod renderer;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "ios")]
mod ios;
#[cfg(target_os = "android")]
mod android;

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use mpv::MpvState;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("mpv")
        .invoke_handler(tauri::generate_handler![
            commands::mpv_load,
            commands::mpv_play,
            commands::mpv_pause,
            commands::mpv_stop,
            commands::mpv_seek,
            commands::mpv_set_volume,
            commands::mpv_set_bounds,
            commands::mpv_set_visible,
            commands::mpv_get_state,
            commands::mpv_sub_add,
            commands::mpv_sub_remove,
            commands::mpv_set_sub_pos,
            commands::mpv_set_sub_delay,
        ])
        .setup(|app, _api| {
            app.manage(MpvState::new());
            tracing::info!("MPV plugin initialized");
            Ok(())
        })
        .build()
}
