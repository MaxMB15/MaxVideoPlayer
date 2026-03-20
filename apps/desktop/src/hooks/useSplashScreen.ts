import { useState, useEffect, useCallback, useRef } from "react";
import { check } from "@tauri-apps/plugin-updater";
import type { Update } from "@tauri-apps/plugin-updater";
import { refreshProvider, refreshEpg } from "@/lib/tauri";
import {
	loadProviderSettings,
	getEpgLastRefresh,
	setEpgLastRefresh,
	useChannels,
} from "@/hooks/useChannels";
import { parseDateMs } from "@/lib/date";

export type StepStatus = "pending" | "active" | "done";

export interface SplashStep {
	id: string;
	label: string;
	status: StepStatus;
}

export interface SplashScreenState {
	steps: SplashStep[];
	allDone: boolean;
	progress: number;
	update: Update | null;
	dismissed: boolean;
	hasProviders: boolean;
	dismiss: () => void;
}

const SESSION_KEY = "splash-shown";

interface UseSplashScreenOptions {
	onComplete?: () => void;
}

export const useSplashScreen = (options: UseSplashScreenOptions = {}): SplashScreenState => {
	const { onComplete } = options;
	const onCompleteRef = useRef(onComplete);
	useEffect(() => {
		onCompleteRef.current = onComplete;
	}, [onComplete]);

	const alreadyShownRef = useRef(sessionStorage.getItem(SESSION_KEY) === "true");

	// Consume providers from ChannelsContext (already loaded on mount — no extra Tauri calls)
	const { providers, initialized } = useChannels();

	// Initialize all 4 steps immediately so they're visible from the first render
	const [steps, setSteps] = useState<SplashStep[]>(() => {
		if (alreadyShownRef.current) return [];
		return [
			{ id: "providers", label: "Loading providers & channels", status: "active" },
			{ id: "playlists", label: "Checking playlists…", status: "pending" },
			{ id: "epg", label: "Checking EPG…", status: "pending" },
			{ id: "updates", label: "Checking for updates", status: "pending" },
		];
	});
	const [allDone, setAllDone] = useState(alreadyShownRef.current);
	const [update, setUpdate] = useState<Update | null>(null);
	const [dismissed, setDismissed] = useState(alreadyShownRef.current);
	const [hasProviders, setHasProviders] = useState(false);

	const dismiss = useCallback(() => {
		sessionStorage.setItem(SESSION_KEY, "true");
		setDismissed(true);
	}, []);

	// Run once ChannelsContext has finished its initial provider fetch
	useEffect(() => {
		if (alreadyShownRef.current || !initialized) return;

		let cancelled = false;

		const setStepStatus = (id: string, status: StepStatus, label?: string) => {
			setSteps((prev) =>
				prev.map((s) => (s.id === id ? { ...s, status, ...(label ? { label } : {}) } : s))
			);
		};

		const run = async () => {
			const hasAnyProviders = providers.length > 0;
			setHasProviders(hasAnyProviders);

			// Step 1 complete — providers came from ChannelsContext, no extra fetch needed
			setStepStatus("providers", "done", "Providers & channels loaded");

			// Determine which providers need refresh
			const now = Date.now();
			const providerRefreshIds: string[] = [];
			const epgRefreshIds: string[] = [];

			for (const p of providers) {
				const {
					autoRefresh,
					refreshIntervalHours,
					epgAutoRefresh,
					epgRefreshIntervalHours,
				} = loadProviderSettings(p.id);

				if (autoRefresh) {
					const lastMs = parseDateMs(p.lastUpdated);
					if (now - lastMs >= refreshIntervalHours * 60 * 60 * 1000) {
						providerRefreshIds.push(p.id);
					}
				}

				if (epgAutoRefresh && p.epgUrl) {
					const lastMs = getEpgLastRefresh(p.id);
					if (now - lastMs >= epgRefreshIntervalHours * 60 * 60 * 1000) {
						epgRefreshIds.push(p.id);
					}
				}
			}

			// Step 2: Refresh playlists (always shown; mark done immediately if nothing to do)
			setStepStatus("playlists", "active", "Refreshing playlists…");
			if (providerRefreshIds.length > 0) {
				await Promise.allSettled(providerRefreshIds.map((id) => refreshProvider(id)));
				if (cancelled) return;
				setStepStatus("playlists", "done", "Playlists refreshed");
			} else {
				setStepStatus(
					"playlists",
					"done",
					hasAnyProviders ? "Playlists up to date" : "No playlists configured"
				);
			}

			// Step 3: Refresh EPG (always shown; mark done immediately if nothing to do)
			setStepStatus("epg", "active", "Checking EPG…");
			if (epgRefreshIds.length > 0) {
				await Promise.allSettled(
					epgRefreshIds.map((id) =>
						refreshEpg(id)
							.then(() => setEpgLastRefresh(id))
							.catch(() => {})
					)
				);
				if (cancelled) return;
				setStepStatus("epg", "done", "EPG refreshed");
			} else {
				setStepStatus(
					"epg",
					"done",
					hasAnyProviders ? "EPG up to date" : "No EPG configured"
				);
			}

			// Step 4: Check for updates
			setStepStatus("updates", "active");
			let foundUpdate: Update | null = null;
			try {
				foundUpdate = (await check()) ?? null;
			} catch {
				// Silently ignore — offline, etc.
			}
			if (cancelled) return;
			if (foundUpdate) {
				setUpdate(foundUpdate);
				setStepStatus("updates", "done", `Update available — v${foundUpdate.version}`);
			} else {
				setStepStatus("updates", "done", "Up to date");
			}

			setAllDone(true);
			onCompleteRef.current?.();
		}

		run();
		return () => {
			cancelled = true;
		};
	}, [initialized]); // eslint-disable-line react-hooks/exhaustive-deps

	const progress =
		steps.length === 0 ? 0 : steps.filter((s) => s.status === "done").length / steps.length;

	return { steps, allDone, progress, update, dismissed, hasProviders, dismiss };
}
