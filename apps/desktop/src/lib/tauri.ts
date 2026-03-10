import { invoke } from "@tauri-apps/api/core";
import type { PlayerState, Channel, Provider, EpgProgram, OmdbData, WatchHistoryEntry } from "./types";

// --- MPV Player Commands ---

export async function mpvLoad(url: string): Promise<void> {
  return invoke("plugin:mpv|mpv_load", { url });
}

export async function mpvPlay(): Promise<void> {
  return invoke("plugin:mpv|mpv_play");
}

export async function mpvPause(): Promise<void> {
  return invoke("plugin:mpv|mpv_pause");
}

export async function mpvStop(): Promise<void> {
  return invoke("plugin:mpv|mpv_stop");
}

export async function mpvSeek(position: number): Promise<void> {
  return invoke("plugin:mpv|mpv_seek", { position });
}

export async function mpvSetVolume(volume: number): Promise<void> {
  return invoke("plugin:mpv|mpv_set_volume", { volume });
}

export async function mpvGetState(): Promise<PlayerState> {
  return invoke("plugin:mpv|mpv_get_state");
}

export async function mpvSetVisible(visible: boolean): Promise<void> {
  return invoke("plugin:mpv|mpv_set_visible", { visible });
}

export async function mpvSetBounds(
  x: number,
  y: number,
  w: number,
  h: number
): Promise<void> {
  return invoke("plugin:mpv|mpv_set_bounds", { x, y, w, h });
}

// --- Core IPTV Commands ---

export async function loadM3uPlaylist(
  name: string,
  url: string
): Promise<Channel[]> {
  return invoke("load_m3u_playlist", { name, url });
}

export async function loadM3uFile(
  name: string,
  path: string
): Promise<Channel[]> {
  return invoke("load_m3u_file", { name, path });
}

export async function loadXtreamProvider(
  name: string,
  url: string,
  username: string,
  password: string
): Promise<Channel[]> {
  return invoke("load_xtream_provider", { name, url, username, password });
}

export async function getProviders(): Promise<Provider[]> {
  return invoke("get_providers");
}

export async function removeProvider(id: string): Promise<void> {
  return invoke("remove_provider", { id });
}

export async function getAllChannels(): Promise<Channel[]> {
  return invoke("get_all_channels");
}

export async function toggleFavorite(channelId: string): Promise<boolean> {
  return invoke("toggle_favorite", { channelId });
}

export async function refreshProvider(id: string): Promise<void> {
  return invoke("refresh_provider", { id });
}

export async function updateProvider(
  id: string,
  name: string,
  url: string,
  username?: string,
  password?: string
): Promise<void> {
  return invoke("update_provider", { id, name, url, username, password });
}

export async function getXtreamSeriesEpisodes(channelId: string): Promise<Channel[]> {
  return invoke("get_xtream_series_episodes", { channelId });
}

export async function refreshEpg(providerId: string): Promise<void> {
  return invoke("refresh_epg", { providerId });
}

export async function getEpgProgrammes(
  channelId: string,
  rangeStart: number,
  rangeEnd: number
): Promise<EpgProgram[]> {
  return invoke("get_epg_programmes", { channelId, rangeStart, rangeEnd });
}

export async function setEpgUrl(
  providerId: string,
  epgUrl: string | null
): Promise<void> {
  return invoke("set_epg_url", { providerId, epgUrl });
}

export const detectEpgUrl = (providerId: string): Promise<string | null> =>
  invoke<string | null>("detect_epg_url", { id: providerId });

// --- OMDB Commands ---

export async function getOmdbApiKey(): Promise<string | null> {
  return invoke("get_omdb_api_key");
}

export async function setOmdbApiKey(key: string): Promise<void> {
  return invoke("set_omdb_api_key", { key });
}

export async function fetchOmdbData(
  channelId: string,
  title: string,
  contentType: "movie" | "series"
): Promise<OmdbData | null> {
  return invoke("fetch_omdb_data", { channelId, title, contentType });
}

// --- Watch History Commands ---

export const recordPlayStart = (
  channelId: string,
  channelName: string,
  channelLogo: string | null,
  contentType: string
): Promise<void> =>
  invoke<void>("record_play_start", { channelId, channelName, channelLogo, contentType });

export const recordPlayEnd = (
  channelId: string,
  durationSeconds: number
): Promise<void> =>
  invoke<void>("record_play_end", { channelId, durationSeconds });

export const getWatchHistory = (limit: number): Promise<WatchHistoryEntry[]> =>
  invoke<WatchHistoryEntry[]>("get_watch_history", { limit });

export const deleteHistoryEntry = (channelId: string): Promise<void> =>
  invoke<void>("delete_history_entry", { channelId });

export const clearWatchHistory = (): Promise<void> =>
  invoke<void>("clear_watch_history");
