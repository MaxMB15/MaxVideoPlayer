// Player preferences kept in web storage.
//
// The volume picked in the player lasts until the app quits (sessionStorage),
// so leaving and reopening the player doesn't reset it. The default volume and
// hardware decoding are Settings choices and persist across launches
// (localStorage). Storage can throw (private mode, quota), so every access is
// guarded and falls back to the built-in defaults.

const VOLUME_KEY = "mvp_volume";
const DEFAULT_VOLUME_KEY = "mvp_default_volume";
const HWDEC_KEY = "mvp_hwdec";

export const MAX_VOLUME = 150;
const BUILTIN_VOLUME = 100;

export interface VolumePreference {
	/** The volume the user last chose. */
	volume: number;
	/** Volume to restore when unmuting. */
	preMute: number;
}

const clampVolume = (v: number): number => Math.min(MAX_VOLUME, Math.max(0, Math.round(v)));

/** The volume playback starts at in a new session (Settings → Default volume). */
export const readDefaultVolume = (): number => {
	try {
		const raw = localStorage.getItem(DEFAULT_VOLUME_KEY);
		const n = raw === null ? NaN : Number.parseFloat(raw);
		return Number.isFinite(n) ? clampVolume(n) : BUILTIN_VOLUME;
	} catch {
		return BUILTIN_VOLUME;
	}
};

/**
 * Save a new default volume. It also drops the volume remembered for this
 * session, so the next stream starts at the new default instead of waiting
 * for a restart.
 */
export const writeDefaultVolume = (volume: number): void => {
	try {
		localStorage.setItem(DEFAULT_VOLUME_KEY, String(clampVolume(volume)));
		sessionStorage.removeItem(VOLUME_KEY);
	} catch {}
};

/** The volume to use for the next stream: this session's choice, else the default. */
export const readVolumePreference = (): VolumePreference => {
	try {
		const raw = sessionStorage.getItem(VOLUME_KEY);
		if (raw) return JSON.parse(raw) as VolumePreference;
	} catch {}
	const volume = readDefaultVolume();
	return { volume, preMute: volume > 0 ? volume : BUILTIN_VOLUME };
};

export const writeVolumePreference = (pref: VolumePreference): void => {
	try {
		sessionStorage.setItem(VOLUME_KEY, JSON.stringify(pref));
	} catch {}
};

/** Whether mpv may use hardware decoding. On unless turned off in Settings. */
export const readHwdecEnabled = (): boolean => {
	try {
		return localStorage.getItem(HWDEC_KEY) !== "off";
	} catch {
		return true;
	}
};

export const writeHwdecEnabled = (enabled: boolean): void => {
	try {
		localStorage.setItem(HWDEC_KEY, enabled ? "on" : "off");
	} catch {}
};
