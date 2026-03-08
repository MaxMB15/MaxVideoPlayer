use crate::mpv::{MpvState, PlayerState};
use tauri::{command, AppHandle, Runtime, State};

#[command]
pub async fn mpv_load<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, MpvState>,
    url: String,
) -> Result<(), String> {
    tracing::info!("[MPV cmd] load url={}", url);
    state.load(&url, &app)?;
    tracing::debug!("[MPV cmd] load complete, state={:?}", state.get_state());
    Ok(())
}

#[command]
pub async fn mpv_play<R: Runtime>(
    _app: AppHandle<R>,
    state: State<'_, MpvState>,
) -> Result<(), String> {
    tracing::info!("[MPV cmd] play");
    state.play()?;
    Ok(())
}

#[command]
pub async fn mpv_pause<R: Runtime>(
    _app: AppHandle<R>,
    state: State<'_, MpvState>,
) -> Result<(), String> {
    tracing::info!("[MPV cmd] pause");
    state.pause()?;
    Ok(())
}

#[command]
pub async fn mpv_stop<R: Runtime>(
    _app: AppHandle<R>,
    state: State<'_, MpvState>,
) -> Result<(), String> {
    tracing::info!("[MPV cmd] stop");
    state.stop();
    Ok(())
}

#[command]
pub async fn mpv_seek<R: Runtime>(
    _app: AppHandle<R>,
    state: State<'_, MpvState>,
    position: f64,
) -> Result<(), String> {
    tracing::debug!("[MPV cmd] seek position={}", position);
    state.seek(position)?;
    Ok(())
}

#[command]
pub async fn mpv_set_volume<R: Runtime>(
    _app: AppHandle<R>,
    state: State<'_, MpvState>,
    volume: f64,
) -> Result<(), String> {
    tracing::debug!("[MPV cmd] set_volume volume={}", volume);
    state.set_volume(volume)?;
    Ok(())
}

#[command]
pub async fn mpv_set_bounds<R: Runtime>(
    _app: AppHandle<R>,
    state: State<'_, MpvState>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> Result<(), String> {
    state.set_bounds(x, y, w, h);
    Ok(())
}

#[command]
pub async fn mpv_set_visible<R: Runtime>(
    _app: AppHandle<R>,
    state: State<'_, MpvState>,
    visible: bool,
) -> Result<(), String> {
    state.set_visible(visible);
    Ok(())
}

#[command]
pub async fn mpv_get_state<R: Runtime>(
    _app: AppHandle<R>,
    state: State<'_, MpvState>,
) -> Result<PlayerState, String> {
    let s = state.get_state();
    tracing::trace!("[MPV cmd] get_state -> playing={} url={:?}", s.is_playing, s.current_url);
    Ok(s)
}
