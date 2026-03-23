use crate::mpv::{MpvState, PlayerState};
#[cfg(target_os = "linux")]
use tauri::Manager;
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
    app: AppHandle<R>,
    state: State<'_, MpvState>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> Result<(), String> {
    // On Linux with CSD (client-side decorations), the Wayland/X11 subsurface is
    // positioned relative to the full window surface which includes the header bar.
    // CSS getBoundingClientRect() gives coordinates relative to the WebView viewport
    // which starts BELOW the header bar. Compute the decoration offset and adjust y.
    #[cfg(target_os = "linux")]
    let y = {
        if let Some(win) = app.get_webview_window("main") {
            let outer = win.outer_size().unwrap_or_default();
            let inner = win.inner_size().unwrap_or_default();
            let scale = win.scale_factor().unwrap_or(1.0);
            // Decoration height in physical pixels, convert to logical (CSS) pixels
            let deco_height = (outer.height as f64 - inner.height as f64) / scale;
            y + deco_height
        } else {
            y
        }
    };
    #[cfg(not(target_os = "linux"))]
    let _ = &app; // suppress unused warning on other platforms
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
pub async fn mpv_sub_add<R: Runtime>(
    _app: AppHandle<R>,
    state: State<'_, MpvState>,
    path: String,
) -> Result<(), String> {
    tracing::info!("[MPV cmd] sub_add path={}", path);
    state.sub_add(&path)?;
    Ok(())
}

#[command]
pub async fn mpv_sub_remove<R: Runtime>(
    _app: AppHandle<R>,
    state: State<'_, MpvState>,
    id: i64,
) -> Result<(), String> {
    tracing::info!("[MPV cmd] sub_remove id={}", id);
    state.sub_remove(id)?;
    Ok(())
}

#[command]
pub async fn mpv_set_sub_pos(
    pos: f64,
    state: tauri::State<'_, MpvState>,
) -> Result<(), String> {
    tracing::info!("[MPV] set_sub_pos: {}", pos);
    state.set_sub_pos(pos)
}

#[command]
pub async fn mpv_set_sub_delay(
    delay: f64,
    state: tauri::State<'_, MpvState>,
) -> Result<(), String> {
    tracing::info!("[MPV] set_sub_delay: {}", delay);
    state.set_sub_delay(delay)
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
