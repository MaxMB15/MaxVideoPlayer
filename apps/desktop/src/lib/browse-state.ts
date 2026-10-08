/**
 * Channel-list state that survives the list unmounting while the player is
 * open: the selected tab, the search text, and the current watch session
 * (used to decide whether to keep the search when coming back).
 */

const KEY = "mvp_browseState";

/** Watching for at least this long clears the search when returning to the list. */
export const SEARCH_RESET_AFTER_MS = 60_000;

interface StoredBrowseState {
	tab?: string;
	search?: string;
	/** When the current player session started playing something. */
	watchStartedAt?: number;
	/** When the player was left (unset while still in the player). */
	watchEndedAt?: number;
}

const read = (): StoredBrowseState => {
	try {
		return JSON.parse(sessionStorage.getItem(KEY) ?? "{}") as StoredBrowseState;
	} catch {
		return {};
	}
};

const write = (patch: Partial<StoredBrowseState>): void => {
	try {
		sessionStorage.setItem(KEY, JSON.stringify({ ...read(), ...patch }));
	} catch {}
};

export const saveBrowseTab = (tab: string): void => write({ tab });

export const saveBrowseSearch = (search: string): void => write({ search });

/** Called by the player when playback starts; continues an open session. */
export const markWatchStarted = (now = Date.now()): void => {
	const s = read();
	write({ watchStartedAt: s.watchStartedAt ?? now, watchEndedAt: undefined });
};

/** Called by the player when it's left. */
export const markWatchEnded = (now = Date.now()): void => {
	if (read().watchStartedAt !== undefined) write({ watchEndedAt: now });
};

/**
 * Read the state to restore when the channel list opens. Ends any watch
 * session: if it lasted at least SEARCH_RESET_AFTER_MS the search is cleared,
 * otherwise it's kept so the user can pick another result.
 */
export const consumeBrowseState = (now = Date.now()): { tab?: string; search: string } => {
	const s = read();
	let search = s.search ?? "";
	if (s.watchStartedAt !== undefined) {
		const watched = (s.watchEndedAt ?? now) - s.watchStartedAt;
		if (watched >= SEARCH_RESET_AFTER_MS) search = "";
		write({ search, watchStartedAt: undefined, watchEndedAt: undefined });
	}
	return { tab: s.tab, search };
};
