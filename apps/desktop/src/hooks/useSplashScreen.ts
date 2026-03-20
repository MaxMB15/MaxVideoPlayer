import { useState, useEffect, useCallback, useRef } from "react";
import { check } from "@tauri-apps/plugin-updater";
import type { Update } from "@tauri-apps/plugin-updater";
import { getProviders, getAllChannels, refreshProvider, refreshEpg } from "@/lib/tauri";
import { loadProviderSettings, getEpgLastRefresh, setEpgLastRefresh } from "@/hooks/useChannels";
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

export function useSplashScreen(options: UseSplashScreenOptions = {}): SplashScreenState {
	const { onComplete } = options;
	const onCompleteRef = useRef(onComplete);
	useEffect(() => {
		onCompleteRef.current = onComplete;
	}, [onComplete]);

	const alreadyShownRef = useRef(sessionStorage.getItem(SESSION_KEY) === "true");

	const [steps, setSteps] = useState<SplashStep[]>([]);
	const [allDone, setAllDone] = useState(alreadyShownRef.current);
	const [update, setUpdate] = useState<Update | null>(null);
	const [dismissed, setDismissed] = useState(alreadyShownRef.current);
	const [hasProviders, setHasProviders] = useState(false);

	const dismiss = useCallback(() => {
		sessionStorage.setItem(SESSION_KEY, "true");
		setDismissed(true);
	}, []);

	useEffect(() => {
		if (alreadyShownRef.current) return;

		let cancelled = false;

		async function run() {
			// Step 1: Load providers and channels
			setSteps([{ id: "providers", label: "Loading providers & channels", status: "active" }]);

			let providers: Awaited<ReturnType<typeof getProviders>> = [];
			try {
				[providers] = await Promise.all([getProviders(), getAllChannels()]);
			} catch {
				// Treat as no providers on error
			}
			if (cancelled) return;

			const hasAnyProviders = providers.length > 0;
			setHasProviders(hasAnyProviders);

			// Determine which optional steps are needed
			const now = Date.now();
			const providerRefreshIds: string[] = [];
			const epgRefreshIds: string[] = [];

			for (const p of providers) {
				const { autoRefresh, refreshIntervalHours, epgAutoRefresh, epgRefreshIntervalHours } =
					loadProviderSettings(p.id);

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

			// Build full step list now that we know which steps are needed
			const builtSteps: SplashStep[] = [
				{ id: "providers", label: "Providers & channels loaded", status: "done" },
				...(providerRefreshIds.length > 0
					? [{ id: "playlists", label: "Refreshing playlists", status: "pending" as StepStatus }]
					: []),
				...(epgRefreshIds.length > 0
					? [{ id: "epg", label: "Refreshing EPG", status: "pending" as StepStatus }]
					: []),
				{ id: "updates", label: "Checking for updates", status: "pending" },
			];
			setSteps(builtSteps);

			const setProgress = (id: string, status: StepStatus, label?: string) => {
				setSteps((prev) =>
					prev.map((s) =>
						s.id === id ? { ...s, status, ...(label ? { label } : {}) } : s
					)
				);
			};

			// Step 2: Refresh playlists
			if (providerRefreshIds.length > 0) {
				setProgress("playlists", "active");
				await Promise.allSettled(providerRefreshIds.map((id) => refreshProvider(id)));
				if (cancelled) return;
				setProgress("playlists", "done", "Playlists refreshed");
			}

			// Step 3: Refresh EPG
			if (epgRefreshIds.length > 0) {
				setProgress("epg", "active");
				await Promise.allSettled(
					epgRefreshIds.map((id) =>
						refreshEpg(id)
							.then(() => setEpgLastRefresh(id))
							.catch(() => {})
					)
				);
				if (cancelled) return;
				setProgress("epg", "done", "EPG refreshed");
			}

			// Step 4: Check for updates
			setProgress("updates", "active");
			let foundUpdate: Update | null = null;
			try {
				foundUpdate = (await check()) ?? null;
			} catch {
				// Silently ignore — offline, etc.
			}
			if (cancelled) return;
			if (foundUpdate) {
				setUpdate(foundUpdate);
				setProgress("updates", "done", `Update available — v${foundUpdate.version}`);
			} else {
				setProgress("updates", "done", "Up to date");
			}

			setAllDone(true);
			onCompleteRef.current?.();
		}

		run();
		return () => {
			cancelled = true;
		};
	}, []);

	const progress =
		steps.length === 0 ? 0 : steps.filter((s) => s.status === "done").length / steps.length;

	return { steps, allDone, progress, update, dismissed, hasProviders, dismiss };
}
