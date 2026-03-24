import { useEffect, useState, useCallback, useRef } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export interface UpdateState {
	update: Update | null;
	checking: boolean;
	installing: boolean;
	progress: number | null;
	error: string | null;
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

	// Guard against concurrent check() calls — only one at a time
	const checkingRef = useRef(false);

	const checkForUpdates = useCallback(async (): Promise<Update | null> => {
		if (checkingRef.current) return null;
		checkingRef.current = true;
		setChecking(true);
		setError(null);
		try {
			const result = (await check()) ?? null;
			setUpdate(result);
			return result;
		} catch (err) {
			const msg = err instanceof Error ? err.message : String(err);
			console.warn("[updater] check error:", msg);
			return null;
		} finally {
			setChecking(false);
			checkingRef.current = false;
		}
	}, []);

	// Check on mount
	useEffect(() => {
		checkForUpdates();
	}, [checkForUpdates]);

	// Re-check every 2 hours
	useEffect(() => {
		const id = setInterval(() => {
			checkForUpdates();
		}, 2 * 60 * 60 * 1000);
		return () => clearInterval(id);
	}, [checkForUpdates]);

	const dismiss = useCallback(() => setUpdate(null), []);

	const install = useCallback(async () => {
		if (!update) return;
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
	}, [update]);

	return { update, checking, installing, progress, error, dismiss, install, checkForUpdates };
};
