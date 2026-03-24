import { useState, useEffect, useCallback, useRef } from "react";
import type { Update } from "@tauri-apps/plugin-updater";
import { refreshProvider, refreshEpg } from "@/lib/tauri";
import {
	loadProviderSettings,
	getEpgLastRefresh,
	setEpgLastRefresh,
	useChannels,
} from "@/hooks/useChannels";
import { parseDateMs } from "@/lib/date";
import type { UpdateState } from "@/hooks/useUpdateChecker";

export type StepStatus = "pending" | "active" | "done" | "error";

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
	updateState: UpdateState;
	onComplete?: () => void;
}

export const useSplashScreen = (options: UseSplashScreenOptions): SplashScreenState => {
	const { updateState, onComplete } = options;
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

			// Step 4: Wait for the shared update check to finish
			setStepStatus("updates", "active");

			// The update check was already triggered by useUpdateChecker on mount.
			// Wait for it to complete by polling the checking state.
			const waitForCheck = (): Promise<void> =>
				new Promise((resolve) => {
					const poll = () => {
						if (cancelled) return resolve();
						// updateState.checking is reactive; poll briefly
						if (!updateState.checking) return resolve();
						setTimeout(poll, 100);
					};
					poll();
				});
			await waitForCheck();

			if (cancelled) return;

			if (updateState.update) {
				setStepStatus("updates", "done", `Update available — v${updateState.update.version}`);
			} else {
				setStepStatus("updates", "done", "Up to date");
			}

			setAllDone(true);
			onCompleteRef.current?.();
		};

		run();
		return () => {
			cancelled = true;
		};
	}, [initialized]); // eslint-disable-line react-hooks/exhaustive-deps

	const progress =
		steps.length === 0
			? 0
			: steps.filter((s) => s.status === "done" || s.status === "error").length /
				steps.length;

	return { steps, allDone, progress, update: updateState.update, dismissed, hasProviders, dismiss };
};
