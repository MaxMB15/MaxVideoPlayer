export interface Channel {
	id: string;
	name: string;
	url: string;
	logoUrl?: string;
	groupTitle: string;
	tvgId?: string;
	tvgName?: string;
	isFavorite: boolean;
	contentType: "live" | "movie" | "series";
	sources: string[];
	seriesTitle?: string;
	season?: number;
	episode?: number;
}

export interface Category {
	id: string;
	name: string;
	channelCount: number;
}

export interface Provider {
	id: string;
	name: string;
	type: "m3u" | "xtream";
	url: string;
	username?: string;
	password?: string;
	lastUpdated?: string;
	channelCount: number;
	epgUrl?: string;
}

export interface Playlist {
	provider: Provider;
	channels: Channel[];
	categories: Category[];
}

export interface PlayerState {
	isPlaying: boolean;
	isPaused: boolean;
	currentUrl: string | null;
	volume: number;
	position: number;
	duration: number;
}

export interface EpgProgram {
	channelId: string;
	title: string;
	description?: string;
	startTime: number; // Unix timestamp seconds
	endTime: number;
	category?: string;
}

export interface EpgSearchResult {
	channelId: string;
	title: string;
	description?: string;
	startTime: number;
	endTime: number;
	channelName: string;
	channelLogoUrl?: string;
}

export interface OmdbData {
	title: string;
	year?: string;
	rated?: string;
	runtime?: string;
	genre?: string;
	director?: string;
	actors?: string;
	plot?: string;
	posterUrl?: string;
	imdbRating?: string;
	rottenTomatoes?: string;
	imdbId?: string;
	imdbVotes?: string;
}

export interface MdbListData {
	imdbId?: string;
	description?: string;
	language?: string;
	mediaType?: string;
	imdbRating?: number;
	imdbVotes?: number;
	tomatometer?: number;
	tomatometerState?: string;
	tomatometerCount?: number;
	tomatoAudienceScore?: number;
	tomatoAudienceCount?: number;
	tomatoAudienceState?: string;
	metacriticScore?: number;
	metacriticVotes?: number;
	tmdbRating?: number;
	tmdbVotes?: number;
	traktRating?: number;
	traktVotes?: number;
	letterboxdRating?: number;
	mdblistScore?: number;
}

export interface WatchHistoryEntry {
	channelId: string;
	channelName: string;
	channelLogo?: string;
	contentType: string;
	firstWatchedAt: number;
	lastWatchedAt: number;
	totalDurationSeconds: number;
	playCount: number;
}

export interface SubtitleEntry {
	fileId: number;
	languageCode: string;
	format: string;
	releaseName?: string;
	downloadCount?: number;
}

export interface SubtitleSearchResult {
	entries: SubtitleEntry[];
	languages: string[];
}

export interface SubtitleCue {
	start: number; // seconds
	end: number;   // seconds
	text: string;
}

export type Platform = "macos" | "ios" | "android" | "windows" | "linux";
export type LayoutMode = "desktop" | "mobile" | "tv";
