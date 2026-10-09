import { createContext, useContext, useEffect, useState, useCallback, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { listDownloads } from "@/lib/tauri";
import type { DownloadRecord, AggregateDownloadState } from "@/lib/types";

/** Pure: compute a series card's aggregate state from its episode downloads. */
export const aggregateForSeries = (
	episodeDownloads: DownloadRecord[],
	totalEpisodes: number
): AggregateDownloadState => {
	if (episodeDownloads.length === 0) return "none";
	if (episodeDownloads.some((d) => d.status === "downloading" || d.status === "queued"))
		return "downloading";
	const completed = episodeDownloads.filter((d) => d.status === "completed").length;
	if (completed === 0) return "none";
	if (totalEpisodes > 0 && completed >= totalEpisodes) return "complete";
	return "partial";
};

/**
 * Pure: should `candidate` replace `current` as the representative record for a
 * channel? A completed record always wins; otherwise the most recent attempt
 * does, so a retry supersedes an earlier failed/cancelled record.
 */
export const supersedes = (candidate: DownloadRecord, current: DownloadRecord): boolean =>
	candidate.status === "completed" ||
	(current.status !== "completed" && candidate.createdAt >= current.createdAt);

/**
 * Pure: collapse the flat download list into per-channel and per-series views,
 * keeping a single representative record per channel (see {@link supersedes}).
 * `byChannel` therefore counts movies + individual episodes (never the series
 * container), and each `bySeries` array holds one record per episode.
 */
export const groupDownloads = (
	downloads: DownloadRecord[]
): {
	byChannel: Map<string, DownloadRecord>;
	bySeries: Map<string, DownloadRecord[]>;
} => {
	const byChannel = new Map<string, DownloadRecord>();
	const bySeriesByChannel = new Map<string, Map<string, DownloadRecord>>();
	for (const d of downloads) {
		const existing = byChannel.get(d.channelId);
		if (!existing || supersedes(d, existing)) byChannel.set(d.channelId, d);
		if (d.seriesChannelId) {
			let inner = bySeriesByChannel.get(d.seriesChannelId);
			if (!inner) {
				inner = new Map<string, DownloadRecord>();
				bySeriesByChannel.set(d.seriesChannelId, inner);
			}
			const prev = inner.get(d.channelId);
			if (!prev || supersedes(d, prev)) inner.set(d.channelId, d);
		}
	}
	const bySeries = new Map<string, DownloadRecord[]>();
	for (const [seriesId, inner] of bySeriesByChannel) {
		bySeries.set(seriesId, Array.from(inner.values()));
	}
	return { byChannel, bySeries };
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

	const { byChannel, bySeries } = groupDownloads(downloads);

	return (
		<DownloadsContext.Provider value={{ downloads, byChannel, bySeries, refresh }}>
			{children}
		</DownloadsContext.Provider>
	);
};

export const useDownloads = (): DownloadsContextValue => {
	const ctx = useContext(DownloadsContext);
	if (!ctx) throw new Error("useDownloads must be used within DownloadsProvider");
	return ctx;
};
