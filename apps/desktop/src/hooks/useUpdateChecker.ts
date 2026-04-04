import { useEffect, useState, useCallback, useRef } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getInstallInfo } from "@/lib/tauri";

export interface UpdateState {
	update: Update | null;
	checking: boolean;
	installing: boolean;
	progress: number | null;
	error: string | null;
	/** true when update exists but auto-install is unsupported (deb/rpm) */
	manualUpdateRequired: boolean;
	/** URL to the releases page for manual download */
	releaseUrl: string | null;
	dismiss: () => void;
	install: () => void;
	checkForUpdates: () => Promise<Update | null>;
}

export const useUpdateChecker = (): UpdateState => {
	const [update, setUpdate] = useState<Update | null>(null);
	const [checking, setChecking] = useState(false);
	const [installing, setInstalling] = useState(false);
	const [progress, setProgress] = useState<number | null>(null);
	const [error, setError] = useState<string | null>(null);
	const [manualUpdateRequired, setManualUpdateRequired] = useState(false);
	const [releaseUrl, setReleaseUrl] = useState<string | null>(null);

	// Cache install info so we only call it once
	const installInfoRef = useRef<{ installType: string; releaseUrl: string } | null>(null);

	// Share a single in-flight promise so concurrent callers (mount + splash)
	// all wait for the same check() call instead of one getting null.
	const inflightRef = useRef<Promise<Update | null> | null>(null);

	const checkForUpdates = useCallback((): Promise<Update | null> => {
		if (inflightRef.current) return inflightRef.current;

		const promise = (async () => {
			setChecking(true);
			setError(null);
			try {
				// Fetch install info once
				if (!installInfoRef.current) {
					installInfoRef.current = await getInstallInfo();
				}

				const result = (await check({ timeout: 5_000 })) ?? null;
				setUpdate(result);

				if (result && installInfoRef.current.installType === "package") {
					setManualUpdateRequired(true);
					setReleaseUrl(installInfoRef.current.releaseUrl);
				} else {
					setManualUpdateRequired(false);
				}

				return result;
			} catch (err) {
				const msg = err instanceof Error ? err.message : String(err);
				console.warn("[updater] check error:", msg);
				return null;
			} finally {
				setChecking(false);
				inflightRef.current = null;
			}
		})();

		inflightRef.current = promise;
		return promise;
	}, []);

	// Check on mount
	useEffect(() => {
		checkForUpdates();
	}, [checkForUpdates]);

	// Re-check every 2 hours
	useEffect(() => {
		const id = setInterval(
			() => {
				checkForUpdates();
			},
			2 * 60 * 60 * 1000
		);
		return () => clearInterval(id);
	}, [checkForUpdates]);

	const dismiss = useCallback(() => {
		setUpdate(null);
		setManualUpdateRequired(false);
	}, []);

	const install = useCallback(async () => {
		if (!update) return;

		// For deb/rpm installs, open the releases page instead of attempting auto-update
		if (manualUpdateRequired && releaseUrl) {
			window.open(releaseUrl, "_blank");
			return;
		}

		setInstalling(true);
		setProgress(0);
		setError(null);
		try {
			let downloaded = 0;
			let total: number | undefined;
			await update.downloadAndInstall((event) => {
				if (event.event === "Started") {
					total = event.data.contentLength ?? undefined;
				} else if (event.event === "Progress") {
					downloaded += event.data.chunkLength;
					if (total) setProgress(Math.round((downloaded / total) * 100));
				}
			});
			await relaunch();
		} catch (err) {
			const msg = err instanceof Error ? err.message : String(err);
			console.error("[updater] install failed:", msg);
			setError(`Update failed: ${msg}`);
			setInstalling(false);
			setProgress(null);
		}
	}, [update, manualUpdateRequired, releaseUrl]);

	return {
		update,
		checking,
		installing,
		progress,
		error,
		manualUpdateRequired,
		releaseUrl,
		dismiss,
		install,
		checkForUpdates,
	};
};
