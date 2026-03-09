use mvp_core::cache::store::CacheStore;
use mvp_core::iptv::m3u::{fetch_and_parse_m3u_with_epg, parse_m3u_file};
use mvp_core::iptv::xtream::{fetch_xtream_channels, fetch_xtream_series_episodes};
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
    let playlist = fetch_and_parse_m3u_with_epg(&url)
        .await
        .map_err(|e| {
            tracing::error!("[IPTV] M3U fetch/parse failed: {e}");
            format!("Failed to load playlist: {e}")
        })?;
    let mut channels = playlist.channels;
    let epg_url = playlist.epg_url;
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
        epg_url,
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
        url: format!("file://{path}"),
        username: None,
        password: None,
        last_updated: Some(now_rfc3339()),
        channel_count: channels.len(),
        epg_url: None,
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
        epg_url: None,
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

#[command]
pub async fn refresh_provider(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    tracing::info!("[IPTV] refreshing provider id={}", id);

    let provider = {
        let cache = state.cache.lock().map_err(|e| e.to_string())?;
        cache.get_provider(&id).map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Provider not found: {id}"))?
    };

    let (mut channels, refreshed_epg_url) = match provider.provider_type {
        ProviderType::M3u => {
            if provider.url.starts_with("file://") {
                let path = provider.url.trim_start_matches("file://").to_string();
                let pb = std::path::PathBuf::from(path);
                let ch = tokio::task::spawn_blocking(move || parse_m3u_file(&pb))
                    .await
                    .map_err(|e| format!("task error: {e}"))?
                    .map_err(|e| format!("Failed to parse file: {e}"))?;
                (ch, None)
            } else {
                let playlist = fetch_and_parse_m3u_with_epg(&provider.url)
                    .await
                    .map_err(|e| format!("Failed to fetch: {e}"))?;
                (playlist.channels, playlist.epg_url)
            }
        }
        ProviderType::Xtream => {
            let username = provider.username.clone().unwrap_or_default();
            let password = provider.password.clone().unwrap_or_default();
            let ch = fetch_xtream_channels(&provider.url, &username, &password)
                .await
                .map_err(|e| format!("Failed to connect: {e}"))?;
            (ch, None)
        }
    };

    prefix_channel_ids(&id, &mut channels);

    let mut updated = provider;
    updated.last_updated = Some(now_rfc3339());
    updated.channel_count = channels.len();
    if refreshed_epg_url.is_some() {
        updated.epg_url = refreshed_epg_url.clone();
    }

    let cache = state.cache.lock().map_err(|e| e.to_string())?;
    cache.save_channels(&id, &channels).map_err(|e| e.to_string())?;
    cache.upsert_provider(&updated).map_err(|e| e.to_string())?;
    tracing::info!("[IPTV] refreshed provider={} with {} channels", id, channels.len());

    Ok(())
}

#[command]
pub async fn update_provider(
    state: State<'_, AppState>,
    id: String,
    name: String,
    url: String,
    username: Option<String>,
    password: Option<String>,
) -> Result<(), String> {
    tracing::info!("[IPTV] updating provider id={} name={:?}", id, name);
    let cache = state.cache.lock().map_err(|e| e.to_string())?;
    cache
        .update_provider_credentials(&id, &name, &url, username.as_deref(), password.as_deref())
        .map_err(|e| e.to_string())
}

/// Lazy-fetch episodes for an Xtream series when the user opens its drawer.
/// The channel URL encodes the series_id as "xtream://series/{series_id}".
#[command]
pub async fn get_xtream_series_episodes(
    state: State<'_, AppState>,
    channel_id: String,
) -> Result<Vec<Channel>, String> {
    let (channel, provider) = {
        let cache = state.cache.lock().map_err(|e| e.to_string())?;
        let ch = cache
            .get_channel_by_id(&channel_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Channel not found: {channel_id}"))?;
        let p = cache
            .get_provider_for_channel(&channel_id)
            .map_err(|e| e.to_string())?
            .ok_or("Provider not found for channel")?;
        (ch, p)
    };

    if !matches!(provider.provider_type, ProviderType::Xtream) {
        return Err("Not an Xtream provider".to_string());
    }

    let series_id: u64 = channel
        .url
        .strip_prefix("xtream://series/")
        .ok_or("Invalid Xtream series URL")?
        .parse()
        .map_err(|e| format!("Invalid series ID: {e}"))?;

    let username = provider.username.as_deref().unwrap_or("");
    let password = provider.password.as_deref().unwrap_or("");

    let mut episodes = fetch_xtream_series_episodes(
        &provider.url,
        username,
        password,
        series_id,
        &channel.name,
        channel.logo_url.as_deref(),
        &channel.group_title,
    )
    .await
    .map_err(|e| e.to_string())?;

    // Prefix episode IDs with the provider ID to match the app-wide convention.
    let provider_id = &provider.id;
    for ep in &mut episodes {
        ep.id = format!("{provider_id}-{}", ep.id);
    }

    tracing::info!(
        "[Xtream] fetched {} episodes for series_id={} (channel={})",
        episodes.len(),
        series_id,
        channel_id
    );

    Ok(episodes)
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
