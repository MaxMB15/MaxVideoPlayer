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
  startTime: number;   // Unix timestamp seconds
  endTime: number;
  category?: string;
}

export type Platform = "macos" | "ios" | "android" | "windows" | "linux";
export type LayoutMode = "desktop" | "mobile" | "tv";
