import {
	createContext,
	useContext,
	useEffect,
	useState,
	useCallback,
	type ReactNode,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { listDownloads } from "@/lib/tauri";
import type { DownloadRecord, AggregateDownloadState } from "@/lib/types";

/** Pure: compute a series card's aggregate state from its episode downloads. */
export const aggregateForSeries = (
	episodeDownloads: DownloadRecord[],
	totalEpisodes: number,
): AggregateDownloadState => {
	if (episodeDownloads.length === 0) return "none";
	if (
		episodeDownloads.some(
			(d) => d.status === "downloading" || d.status === "queued",
		)
	)
		return "downloading";
	const completed = episodeDownloads.filter(
		(d) => d.status === "completed",
	).length;
	if (completed === 0) return "none";
	if (totalEpisodes > 0 && completed >= totalEpisodes) return "complete";
	return "partial";
};

interface DownloadsContextValue {
	downloads: DownloadRecord[];
	byChannel: Map<string, DownloadRecord>;
	bySeries: Map<string, DownloadRecord[]>;
	refresh: () => Promise<void>;
}

const DownloadsContext = createContext<DownloadsContextValue | null>(null);

export const DownloadsProvider = ({ children }: { children: ReactNode }) => {
	const [downloads, setDownloads] = useState<DownloadRecord[]>([]);

	const refresh = useCallback(async () => {
		setDownloads(await listDownloads());
	}, []);

	useEffect(() => {
		refresh();
		const unlisten = listen<DownloadRecord>("download://progress", (e) => {
			setDownloads((prev) => {
				const next = prev.filter((d) => d.id !== e.payload.id);
				next.push(e.payload);
				return next;
			});
		});
		return () => {
			unlisten.then((f) => f());
		};
	}, [refresh]);

	const byChannel = new Map<string, DownloadRecord>();
	const bySeries = new Map<string, DownloadRecord[]>();
	for (const d of downloads) {
		const existing = byChannel.get(d.channelId);
		if (!existing || d.status === "completed") byChannel.set(d.channelId, d);
		if (d.seriesChannelId) {
			const arr = bySeries.get(d.seriesChannelId) ?? [];
			arr.push(d);
			bySeries.set(d.seriesChannelId, arr);
		}
	}

	return (
		<DownloadsContext.Provider
			value={{ downloads, byChannel, bySeries, refresh }}
		>
			{children}
		</DownloadsContext.Provider>
	);
};

export const useDownloads = (): DownloadsContextValue => {
	const ctx = useContext(DownloadsContext);
	if (!ctx)
		throw new Error("useDownloads must be used within DownloadsProvider");
	return ctx;
};
