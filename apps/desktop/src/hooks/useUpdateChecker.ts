import { useEffect, useState, useCallback } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export interface UpdateState {
	update: Update | null;
	checking: boolean;
	installing: boolean;
	progress: number | null;
	dismiss: () => void;
	install: () => void;
}

export const useUpdateChecker = (): UpdateState => {
	const [update, setUpdate] = useState<Update | null>(null);
	const [checking, setChecking] = useState(false);
	const [installing, setInstalling] = useState(false);
	const [progress, setProgress] = useState<number | null>(null);

	useEffect(() => {
		// Check for updates on mount, silently ignore errors (offline, etc.)
		setChecking(true);
		check()
			.then((result) => {
				console.log("[updater] result:", result);
				setUpdate(result ?? null);
			})
			.catch((err) => console.warn("[updater] error:", err))
			.finally(() => setChecking(false));
	}, []);

	// Re-check every 2 hours while the app is open
	useEffect(() => {
		const id = setInterval(
			() => {
				check()
					.then((result) => setUpdate(result ?? null))
					.catch(() => {});
			},
			2 * 60 * 60 * 1000
		);
		return () => clearInterval(id);
	}, []);

	const dismiss = useCallback(() => setUpdate(null), []);

	const install = useCallback(async () => {
		if (!update) return;
		setInstalling(true);
		setProgress(0);
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
			console.error("[updater] install failed:", err);
			setInstalling(false);
			setProgress(null);
		}
	}, [update]);

	return { update, checking, installing, progress, dismiss, install };
}
