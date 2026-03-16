import {
	createContext,
	useContext,
	useState,
	useCallback,
	useEffect,
	useMemo,
	useRef,
} from "react";
import type { Channel, Category, Provider } from "@/lib/types";
import {
	loadM3uPlaylist,
	loadM3uFile as loadM3uFileApi,
	loadXtreamProvider,
	getProviders,
	removeProvider as removeProviderApi,
	getAllChannels,
	refreshProvider as refreshProviderApi,
	refreshEpg as refreshEpgApi,
	updateProvider as updateProviderApi,
	toggleFavorite as toggleFavoriteApi,
} from "@/lib/tauri";

// --- Provider settings (stored in localStorage) ---

export interface ProviderSettings {
	autoRefresh: boolean;
	refreshIntervalHours: number;
	epgAutoRefresh: boolean;
	epgRefreshIntervalHours: number;
}

export const loadProviderSettings = (id: string): ProviderSettings => {
	try {
		const raw = localStorage.getItem(`provider-settings-${id}`);
		if (raw) {
			const parsed = JSON.parse(raw);
			// Migrate from old 3-way string format ("disabled"|"startup"|"interval")
			if (typeof parsed.autoRefresh === "string") {
				return {
					autoRefresh: parsed.autoRefresh !== "disabled",
					refreshIntervalHours: parsed.refreshIntervalHours ?? 24,
					epgAutoRefresh: parsed.epgAutoRefresh ?? true,
					epgRefreshIntervalHours: parsed.epgRefreshIntervalHours ?? 24,
				};
			}
			return {
				epgAutoRefresh: true,
				epgRefreshIntervalHours: 24,
				...parsed,
			} as ProviderSettings;
		}
	} catch {}
	return {
		autoRefresh: false,
		refreshIntervalHours: 24,
		epgAutoRefresh: true,
		epgRefreshIntervalHours: 24,
	};
};

export const saveProviderSettings = (id: string, settings: ProviderSettings) => {
	localStorage.setItem(`provider-settings-${id}`, JSON.stringify(settings));
};

// EPG last-refresh timestamp stored separately (providers have no epg_last_updated field)
const epgLastRefreshKey = (id: string) => `epg-last-refresh-${id}`;
const getEpgLastRefresh = (id: string): number =>
	parseInt(localStorage.getItem(epgLastRefreshKey(id)) ?? "0", 10);
const setEpgLastRefresh = (id: string) =>
	localStorage.setItem(epgLastRefreshKey(id), String(Date.now()));

// --- Context ---

interface ChannelsContextValue {
	channels: Channel[];
	categories: Category[];
	providers: Provider[];
	loading: boolean;
	error: string | null;
	loadM3u: (name: string, url: string) => Promise<void>;
	loadM3uFile: (name: string, path: string) => Promise<void>;
	loadXtream: (name: string, url: string, username: string, password: string) => Promise<void>;
	refreshProviders: () => Promise<void>;
	refreshChannels: () => Promise<void>;
	refreshProvider: (id: string) => Promise<void>;
	updateProvider: (
		id: string,
		name: string,
		url: string,
		username?: string,
		password?: string
	) => Promise<void>;
	removeProvider: (id: string) => Promise<void>;
	toggleFavorite: (channelId: string) => Promise<void>;
}

export const ChannelsContext = createContext<ChannelsContextValue | null>(null);

export const useChannelsProvider = (): ChannelsContextValue => {
	const [channels, setChannels] = useState<Channel[]>([]);
	const [categories, setCategories] = useState<Category[]>([]);
	const [providers, setProviders] = useState<Provider[]>([]);
	const [loading, setLoading] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const startupDone = useRef(false);

	const channelIndex = useMemo(() => {
		const m = new Map<string, number>();
		channels.forEach((ch, i) => m.set(ch.id, i));
		return m;
	}, [channels]);

	const deriveCategories = (chs: Channel[]) => {
		const map = new Map<string, number>();
		for (const ch of chs) {
			map.set(ch.groupTitle, (map.get(ch.groupTitle) ?? 0) + 1);
		}
		setCategories(
			Array.from(map.entries()).map(([name, count], i) => ({
				id: `cat-${i}`,
				name,
				channelCount: count,
			}))
		);
	};

	const refreshChannels = useCallback(async () => {
		try {
			const chs = await getAllChannels();
			setChannels(chs);
			deriveCategories(chs);
		} catch (e) {
			setError(String(e));
		}
	}, []);

	const refreshProviders = useCallback(async () => {
		try {
			setProviders(await getProviders());
		} catch (e) {
			setError(String(e));
		}
	}, []);

	const refreshProvider = useCallback(
		async (id: string) => {
			try {
				await refreshProviderApi(id);
				await refreshProviders();
				await refreshChannels();
			} catch (e) {
				setError(String(e));
				throw e;
			}
		},
		[refreshProviders, refreshChannels]
	);

	const updateProvider = useCallback(
		async (id: string, name: string, url: string, username?: string, password?: string) => {
			try {
				await updateProviderApi(id, name, url, username, password);
				await refreshProviders();
			} catch (e) {
				setError(String(e));
				throw e;
			}
		},
		[refreshProviders]
	);

	const loadM3u = useCallback(
		async (name: string, url: string) => {
			setLoading(true);
			setError(null);
			try {
				await loadM3uPlaylist(name, url);
				await refreshProviders();
				await refreshChannels();
			} catch (e) {
				setError(String(e));
				throw e;
			} finally {
				setLoading(false);
			}
		},
		[refreshProviders, refreshChannels]
	);

	const loadM3uFile = useCallback(
		async (name: string, path: string) => {
			setLoading(true);
			setError(null);
			try {
				await loadM3uFileApi(name, path);
				await refreshProviders();
				await refreshChannels();
			} catch (e) {
				setError(String(e));
				throw e;
			} finally {
				setLoading(false);
			}
		},
		[refreshProviders, refreshChannels]
	);

	const loadXtream = useCallback(
		async (name: string, url: string, username: string, password: string) => {
			setLoading(true);
			setError(null);
			try {
				await loadXtreamProvider(name, url, username, password);
				await refreshProviders();
				await refreshChannels();
			} catch (e) {
				setError(String(e));
				throw e;
			} finally {
				setLoading(false);
			}
		},
		[refreshProviders, refreshChannels]
	);

	const removeProvider = useCallback(
		async (id: string) => {
			try {
				await removeProviderApi(id);
				await refreshProviders();
				await refreshChannels();
			} catch (e) {
				setError(String(e));
			}
		},
		[refreshProviders, refreshChannels]
	);

	const toggleFavorite = useCallback(
		async (channelId: string) => {
			try {
				const newFavorite = await toggleFavoriteApi(channelId);
				setChannels((prev) => {
					const idx = channelIndex.get(channelId);
					if (idx === undefined) return prev;
					const next = [...prev];
					next[idx] = { ...next[idx], isFavorite: newFavorite };
					return next;
				});
			} catch (e) {
				setError(String(e));
			}
		},
		[channelIndex]
	);

	// Initial load
	useEffect(() => {
		refreshProviders();
		refreshChannels();
	}, [refreshProviders, refreshChannels]);

	// Auto-refresh on startup: if provider/EPG is older than its interval, refresh immediately.
	// Runs once after the first non-empty providers load.
	useEffect(() => {
		if (providers.length === 0 || startupDone.current) return;
		startupDone.current = true;
		const now = Date.now();
		for (const p of providers) {
			const { autoRefresh, refreshIntervalHours, epgAutoRefresh, epgRefreshIntervalHours } =
				loadProviderSettings(p.id);

			if (autoRefresh) {
				const intervalMs = refreshIntervalHours * 60 * 60 * 1000;
				const lastMs = p.lastUpdated ? new Date(p.lastUpdated).getTime() : 0;
				if (now - lastMs >= intervalMs) {
					refreshProvider(p.id).catch(console.error);
				}
			}

			if (epgAutoRefresh && p.epgUrl) {
				const intervalMs = epgRefreshIntervalHours * 60 * 60 * 1000;
				const lastMs = getEpgLastRefresh(p.id);
				if (now - lastMs >= intervalMs) {
					refreshEpgApi(p.id)
						.then(() => setEpgLastRefresh(p.id))
						.catch(console.error);
				}
			}
		}
	}, [providers, refreshProvider]);

	// Auto-refresh on interval: keep ticking while the app is open.
	useEffect(() => {
		if (providers.length === 0) return;
		const timers: ReturnType<typeof setInterval>[] = [];
		for (const p of providers) {
			const { autoRefresh, refreshIntervalHours, epgAutoRefresh, epgRefreshIntervalHours } =
				loadProviderSettings(p.id);

			if (autoRefresh) {
				const ms = refreshIntervalHours * 60 * 60 * 1000;
				timers.push(setInterval(() => refreshProvider(p.id).catch(console.error), ms));
			}

			if (epgAutoRefresh && p.epgUrl) {
				const ms = epgRefreshIntervalHours * 60 * 60 * 1000;
				timers.push(
					setInterval(
						() =>
							refreshEpgApi(p.id)
								.then(() => setEpgLastRefresh(p.id))
								.catch(console.error),
						ms
					)
				);
			}
		}
		return () => timers.forEach(clearInterval);
	}, [providers, refreshProvider]);

	return {
		channels,
		categories,
		providers,
		loading,
		error,
		loadM3u,
		loadM3uFile,
		loadXtream,
		refreshProviders,
		refreshChannels,
		refreshProvider,
		updateProvider,
		removeProvider,
		toggleFavorite,
	};
};

export const useChannels = (): ChannelsContextValue => {
	const ctx = useContext(ChannelsContext);
	if (!ctx) throw new Error("useChannels must be used within a ChannelsProvider");
	return ctx;
};
