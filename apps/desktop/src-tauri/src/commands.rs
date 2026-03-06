use mvp_core::cache::store::CacheStore;
use mvp_core::iptv::m3u::{fetch_and_parse_m3u, parse_m3u_file};
use mvp_core::iptv::xtream::fetch_xtream_channels;
use mvp_core::models::channel::Channel;
use mvp_core::models::playlist::{Provider, ProviderType};
use std::sync::Mutex;
use tauri::{command, State};

pub struct AppState {
    pub cache: Mutex<CacheStore>,
}

fn prefix_channel_ids(provider_id: &str, channels: &mut [Channel]) {
    for ch in channels.iter_mut() {
        ch.id = format!("{provider_id}-{}", ch.id);
    }
}

#[command]
pub async fn load_m3u_playlist(
    state: State<'_, AppState>,
    name: String,
    url: String,
) -> Result<Vec<Channel>, String> {
    tracing::info!("[IPTV] loading M3U playlist name={:?} url={}", name, url);
    let mut channels = fetch_and_parse_m3u(&url)
        .await
        .map_err(|e| {
            tracing::error!("[IPTV] M3U fetch/parse failed: {e}");
            format!("Failed to load playlist: {e}")
        })?;
    tracing::info!("[IPTV] parsed {} channels from M3U URL", channels.len());

    let provider_id = format!("m3u-{}", uuid_simple());
    prefix_channel_ids(&provider_id, &mut channels);

    let provider = Provider {
        id: provider_id.clone(),
        name,
        provider_type: ProviderType::M3u,
        url,
        username: None,
        password: None,
        last_updated: Some(now_rfc3339()),
        channel_count: channels.len(),
    };

    let cache = state.cache.lock().map_err(|e| e.to_string())?;
    cache.upsert_provider(&provider).map_err(|e| {
        tracing::error!("[IPTV] failed to save provider: {e}");
        e.to_string()
    })?;
    cache.save_channels(&provider_id, &channels).map_err(|e| {
        tracing::error!("[IPTV] failed to save channels: {e}");
        e.to_string()
    })?;
    tracing::info!("[IPTV] saved provider={} with {} channels", provider_id, channels.len());

    Ok(channels)
}

/// Load an M3U file from a local filesystem path (streaming, low memory).
#[command]
pub async fn load_m3u_file(
    state: State<'_, AppState>,
    name: String,
    path: String,
) -> Result<Vec<Channel>, String> {
    tracing::info!("[IPTV] loading M3U from file path={:?} name={:?}", path, name);

    let file_path = std::path::PathBuf::from(&path);
    let mut channels = tokio::task::spawn_blocking(move || parse_m3u_file(&file_path))
        .await
        .map_err(|e| format!("task error: {e}"))?
        .map_err(|e| {
            tracing::error!("[IPTV] M3U file parse failed: {e}");
            format!("Failed to parse playlist: {e}")
        })?;
    tracing::info!("[IPTV] parsed {} channels from file", channels.len());

    let provider_id = format!("m3u-{}", uuid_simple());
    prefix_channel_ids(&provider_id, &mut channels);

    let source_label = std::path::Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());

    let provider = Provider {
        id: provider_id.clone(),
        name,
        provider_type: ProviderType::M3u,
        url: format!("file://{source_label}"),
        username: None,
        password: None,
        last_updated: Some(now_rfc3339()),
        channel_count: channels.len(),
    };

    let cache = state.cache.lock().map_err(|e| e.to_string())?;
    cache.upsert_provider(&provider).map_err(|e| {
        tracing::error!("[IPTV] failed to save provider: {e}");
        e.to_string()
    })?;
    cache.save_channels(&provider_id, &channels).map_err(|e| {
        tracing::error!("[IPTV] failed to save channels: {e}");
        e.to_string()
    })?;
    tracing::info!("[IPTV] saved file provider={} with {} channels", provider_id, channels.len());

    Ok(channels)
}

#[command]
pub async fn load_xtream_provider(
    state: State<'_, AppState>,
    name: String,
    url: String,
    username: String,
    password: String,
) -> Result<Vec<Channel>, String> {
    tracing::info!("[IPTV] loading Xtream provider name={:?} url={}", name, url);
    let mut channels = fetch_xtream_channels(&url, &username, &password)
        .await
        .map_err(|e| {
            tracing::error!("[IPTV] Xtream connect failed: {e}");
            format!("Failed to connect: {e}")
        })?;
    tracing::info!("[IPTV] fetched {} channels from Xtream", channels.len());

    let provider_id = format!("xt-{}", uuid_simple());
    prefix_channel_ids(&provider_id, &mut channels);

    let provider = Provider {
        id: provider_id.clone(),
        name,
        provider_type: ProviderType::Xtream,
        url,
        username: Some(username),
        password: Some(password),
        last_updated: Some(now_rfc3339()),
        channel_count: channels.len(),
    };

    let cache = state.cache.lock().map_err(|e| e.to_string())?;
    cache.upsert_provider(&provider).map_err(|e| e.to_string())?;
    cache.save_channels(&provider_id, &channels).map_err(|e| e.to_string())?;
    tracing::info!("[IPTV] saved Xtream provider={} with {} channels", provider_id, channels.len());

    Ok(channels)
}

#[command]
pub async fn get_providers(state: State<'_, AppState>) -> Result<Vec<Provider>, String> {
    let cache = state.cache.lock().map_err(|e| e.to_string())?;
    let providers = cache.get_providers().map_err(|e| e.to_string())?;
    tracing::debug!("[IPTV] get_providers -> {} providers", providers.len());
    Ok(providers)
}

#[command]
pub async fn remove_provider(state: State<'_, AppState>, id: String) -> Result<(), String> {
    tracing::info!("[IPTV] removing provider id={}", id);
    let cache = state.cache.lock().map_err(|e| e.to_string())?;
    cache.remove_provider(&id).map_err(|e| e.to_string())
}

#[command]
pub async fn get_all_channels(state: State<'_, AppState>) -> Result<Vec<Channel>, String> {
    let cache = state.cache.lock().map_err(|e| e.to_string())?;
    let channels = cache.get_all_channels().map_err(|e| e.to_string())?;
    tracing::debug!("[IPTV] get_all_channels -> {} channels", channels.len());
    Ok(channels)
}

#[command]
pub async fn toggle_favorite(
    state: State<'_, AppState>,
    channel_id: String,
) -> Result<bool, String> {
    tracing::info!("[IPTV] toggle_favorite channel_id={}", channel_id);
    let cache = state.cache.lock().map_err(|e| e.to_string())?;
    cache
        .toggle_favorite(&channel_id)
        .map_err(|e| e.to_string())
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{nanos:x}")
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}
