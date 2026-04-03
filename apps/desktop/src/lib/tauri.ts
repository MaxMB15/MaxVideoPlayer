import { invoke } from "@tauri-apps/api/core";
import type {
	PlayerState,
	Channel,
	Provider,
	EpgProgram,
	EpgSearchResult,
	OmdbData,
	MdbListData,
	WhatsonData,
	WatchHistoryEntry,
	SubtitleSearchResult,
	GroupHierarchyEntry,
	PinnedGroup,
} from "./types";

// --- MPV Player Commands ---

export const mpvLoad = (url: string): Promise<void> => invoke("plugin:mpv|mpv_load", { url });

export const mpvPlay = (): Promise<void> => invoke("plugin:mpv|mpv_play");

export const mpvPause = (): Promise<void> => invoke("plugin:mpv|mpv_pause");

export const mpvStop = (): Promise<void> => invoke("plugin:mpv|mpv_stop");

export const mpvSeek = (position: number): Promise<void> =>
	invoke("plugin:mpv|mpv_seek", { position });

export const mpvSetVolume = (volume: number): Promise<void> =>
	invoke("plugin:mpv|mpv_set_volume", { volume });

export const mpvGetState = (): Promise<PlayerState> => invoke("plugin:mpv|mpv_get_state");

export const mpvSetVisible = (visible: boolean): Promise<void> =>
	invoke("plugin:mpv|mpv_set_visible", { visible });

export const mpvSetBounds = (x: number, y: number, w: number, h: number): Promise<void> =>
	invoke("plugin:mpv|mpv_set_bounds", { x, y, w, h });

// --- Core IPTV Commands ---

export const loadM3uPlaylist = (name: string, url: string): Promise<Channel[]> =>
	invoke("load_m3u_playlist", { name, url });

export const loadM3uFile = (name: string, path: string): Promise<Channel[]> =>
	invoke("load_m3u_file", { name, path });

export const loadXtreamProvider = (
	name: string,
	url: string,
	username: string,
	password: string
): Promise<Channel[]> => invoke("load_xtream_provider", { name, url, username, password });

export const getProviders = (): Promise<Provider[]> => invoke("get_providers");

export const removeProvider = (id: string): Promise<void> => invoke("remove_provider", { id });

export const getAllChannels = (): Promise<Channel[]> => invoke("get_all_channels");

export const toggleFavorite = (channelId: string): Promise<boolean> =>
	invoke("toggle_favorite", { channelId });

export const refreshProvider = (id: string): Promise<void> => invoke("refresh_provider", { id });

export const updateProvider = (
	id: string,
	name: string,
	url: string,
	username?: string,
	password?: string
): Promise<void> => invoke("update_provider", { id, name, url, username, password });

export const getXtreamSeriesEpisodes = (channelId: string): Promise<Channel[]> =>
	invoke("get_xtream_series_episodes", { channelId });

export const refreshEpg = (providerId: string): Promise<void> =>
	invoke("refresh_epg", { providerId });

export const getEpgProgrammes = (
	channelId: string,
	rangeStart: number,
	rangeEnd: number
): Promise<EpgProgram[]> => invoke("get_epg_programmes", { channelId, rangeStart, rangeEnd });

export const getEpgForLiveChannels = (
	rangeStart: number,
	rangeEnd: number
): Promise<EpgProgram[]> => invoke("get_epg_for_live_channels", { rangeStart, rangeEnd });

export const searchEpgProgrammes = (
	query: string,
	rangeStart: number
): Promise<EpgSearchResult[]> => invoke("search_epg_programmes", { query, rangeStart });

export const setEpgUrl = (providerId: string, epgUrl: string | null): Promise<void> =>
	invoke("set_epg_url", { providerId, epgUrl });

export const detectEpgUrl = (providerId: string): Promise<string | null> =>
	invoke<string | null>("detect_epg_url", { id: providerId });

// --- OMDB Commands ---

export const getOmdbApiKey = (): Promise<string | null> => invoke("get_omdb_api_key");

export const setOmdbApiKey = (key: string): Promise<void> => invoke("set_omdb_api_key", { key });

export const fetchOmdbData = (
	channelId: string,
	title: string,
	contentType: "movie" | "series"
): Promise<OmdbData | null> => invoke("fetch_omdb_data", { channelId, title, contentType });

// --- MDBList Commands ---

export const getMdbListApiKey = (): Promise<string | null> => invoke("get_mdblist_api_key");

export const setMdbListApiKey = (key: string): Promise<void> =>
	invoke("set_mdblist_api_key", { key });

export const testMdbListApiKey = (key: string): Promise<boolean> =>
	invoke("test_mdblist_api_key", { key });

export const fetchMdbListData = (imdbId: string, mediaType: string): Promise<MdbListData | null> =>
	invoke("fetch_mdblist_data", { imdbId, mediaType });

// --- Whatson Commands ---

export const fetchWhatsonData = (imdbId: string, mediaType: string): Promise<WhatsonData | null> =>
	invoke("fetch_whatson_data", { imdbId, mediaType });

// --- OpenSubtitles Commands ---

export const getOpenSubtitlesApiKey = (): Promise<string | null> =>
	invoke("get_opensubtitles_api_key");

export const setOpenSubtitlesApiKey = (key: string): Promise<void> =>
	invoke("set_opensubtitles_api_key", { key });

export const testOpenSubtitlesApiKey = (key: string): Promise<boolean> =>
	invoke("test_opensubtitles_api_key", { key });

export const searchSubtitles = (
	imdbId: string,
	season?: number,
	episode?: number
): Promise<SubtitleSearchResult | null> => invoke("search_subtitles", { imdbId, season, episode });

export const downloadSubtitle = (fileId: number): Promise<string> =>
	invoke("download_subtitle", { fileId });

export const readSubtitleFile = (path: string): Promise<string> =>
	invoke("read_subtitle_file", { path });

export const mpvSubAdd = (path: string): Promise<void> =>
	invoke("plugin:mpv|mpv_sub_add", { path });

export const mpvSubRemove = (id: number): Promise<void> =>
	invoke("plugin:mpv|mpv_sub_remove", { id });

export const mpvSetSubPos = (pos: number): Promise<void> =>
	invoke("plugin:mpv|mpv_set_sub_pos", { pos });

export const mpvSetSubDelay = (delay: number): Promise<void> =>
	invoke("plugin:mpv|mpv_set_sub_delay", { delay });

// --- Watch History Commands ---

export const recordPlayStart = (
	channelId: string,
	channelName: string,
	channelLogo: string | null,
	contentType: string
): Promise<void> =>
	invoke<void>("record_play_start", {
		channelId,
		channelName,
		channelLogo,
		contentType,
	});

export const recordPlayEnd = (channelId: string, durationSeconds: number): Promise<void> =>
	invoke<void>("record_play_end", { channelId, durationSeconds });

export const getWatchHistory = (limit: number): Promise<WatchHistoryEntry[]> =>
	invoke<WatchHistoryEntry[]>("get_watch_history", { limit });

export const deleteHistoryEntry = (channelId: string): Promise<void> =>
	invoke<void>("delete_history_entry", { channelId });

export const clearWatchHistory = (): Promise<void> => invoke<void>("clear_watch_history");

export const clearAllCaches = (): Promise<void> => invoke<void>("clear_all_caches");

// --- Group Hierarchy Commands ---

export const getGroupHierarchy = (
	providerId: string,
	contentType: string
): Promise<GroupHierarchyEntry[]> =>
	invoke<GroupHierarchyEntry[]>("get_group_hierarchy", { providerId, contentType });

export const reorderGroupHierarchyEntry = (
	providerId: string,
	contentType: string,
	groupName: string,
	newSortOrder: number
): Promise<void> =>
	invoke("reorder_group_hierarchy_entry", {
		providerId,
		contentType,
		groupName,
		newSortOrder,
	});

export const updateGroupHierarchyEntry = (
	providerId: string,
	contentType: string,
	groupName: string,
	newSuperCategory: string | null,
	newSortOrder: number
): Promise<void> =>
	invoke("update_group_hierarchy_entry", {
		providerId,
		contentType,
		groupName,
		newSuperCategory,
		newSortOrder,
	});

export const deleteGroupHierarchy = (providerId: string, contentType: string): Promise<void> =>
	invoke("delete_group_hierarchy", { providerId, contentType });

export const pinGroup = (
	providerId: string,
	contentType: string,
	groupName: string,
	sortOrder: number
): Promise<void> => invoke("pin_group", { providerId, contentType, groupName, sortOrder });

export const unpinGroup = (
	providerId: string,
	contentType: string,
	groupName: string
): Promise<void> => invoke("unpin_group", { providerId, contentType, groupName });

export const getPinnedGroups = (providerId: string, contentType: string): Promise<PinnedGroup[]> =>
	invoke<PinnedGroup[]>("get_pinned_groups", { providerId, contentType });

// --- Gemini API Commands ---

export const getGeminiApiKey = (): Promise<string | null> =>
	invoke<string | null>("get_gemini_api_key");

export const setGeminiApiKey = (key: string): Promise<void> =>
	invoke("set_gemini_api_key", { key });

export const testGeminiApiKey = (key: string): Promise<boolean> =>
	invoke<boolean>("test_gemini_api_key", { key });

export const categorizeProvider = (
	providerId: string,
	contentType: string,
	apiKey: string,
	groupsWithSamples: [string, string[]][]
): Promise<GroupHierarchyEntry[]> =>
	invoke<GroupHierarchyEntry[]>("categorize_provider", {
		providerId,
		contentType,
		apiKey,
		groupsWithSamples,
	});

export const fixUncategorizedGroups = (
	providerId: string,
	contentType: string,
	apiKey: string,
	uncategorizedGroups: [string, string[]][],
	existingCategories: string[]
): Promise<GroupHierarchyEntry[]> =>
	invoke<GroupHierarchyEntry[]>("fix_uncategorized_groups", {
		providerId,
		contentType,
		apiKey,
		uncategorizedGroups,
		existingCategories,
	});

export const renameSuperCategory = (
	providerId: string,
	contentType: string,
	oldName: string,
	newName: string
): Promise<void> => invoke("rename_super_category", { providerId, contentType, oldName, newName });

export const deleteSuperCategory = (
	providerId: string,
	contentType: string,
	categoryName: string
): Promise<void> => invoke("delete_super_category", { providerId, contentType, categoryName });
