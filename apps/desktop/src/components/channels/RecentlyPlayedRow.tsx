import { useState, useEffect, useCallback, useMemo } from "react";
import { X } from "lucide-react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { getWatchHistory, deleteHistoryEntry } from "@/lib/tauri";
import type { WatchHistoryEntry, Channel } from "@/lib/types";

interface RecentlyPlayedRowProps {
	contentType: "live" | "movie" | "series";
	onPlay: (entry: WatchHistoryEntry) => void;
	channels?: Channel[];
}

/** Strip SxxExx patterns to get the series title from an episode name. */
const extractSeriesTitle = (name: string): string =>
	name.replace(/\s+S\d{1,3}E\d{1,3}.*/i, "").trim() || name;

interface DisplayEntry {
	key: string;
	displayName: string;
	entry: WatchHistoryEntry;
}

export const RecentlyPlayedRow = ({ contentType, onPlay, channels }: RecentlyPlayedRowProps) => {
	const [entries, setEntries] = useState<WatchHistoryEntry[]>([]);

	const load = useCallback(async () => {
		const history = await getWatchHistory(50);
		setEntries(history.filter((e) => e.contentType === contentType));
	}, [contentType]);

	useEffect(() => {
		load();
	}, [load]);

	// For series: deduplicate by series title, showing only the most recent entry per series
	const displayEntries = useMemo((): DisplayEntry[] => {
		if (contentType !== "series") {
			return entries.map((e) => ({ key: e.channelId, displayName: e.channelName, entry: e }));
		}

		// Build a lookup from channelId -> seriesTitle using the channels list
		const seriesTitleById = new Map<string, string>();
		if (channels) {
			for (const ch of channels) {
				if (ch.contentType === "series" && ch.seriesTitle) {
					seriesTitleById.set(ch.id, ch.seriesTitle);
				}
			}
		}

		const seen = new Set<string>();
		const deduped: DisplayEntry[] = [];

		for (const entry of entries) {
			// Try channel lookup first, then fall back to regex extraction
			const seriesTitle =
				seriesTitleById.get(entry.channelId) ?? extractSeriesTitle(entry.channelName);
			if (seen.has(seriesTitle)) continue;
			seen.add(seriesTitle);
			deduped.push({ key: seriesTitle, displayName: seriesTitle, entry });
		}
		return deduped;
	}, [entries, contentType, channels]);

	const handleDismiss = async (e: React.MouseEvent, channelId: string) => {
		e.stopPropagation();
		await deleteHistoryEntry(channelId);
		setEntries((prev) => prev.filter((entry) => entry.channelId !== channelId));
	};

	const handleClearAll = async () => {
		for (const entry of entries) await deleteHistoryEntry(entry.channelId);
		setEntries([]);
	};

	if (displayEntries.length === 0) return null;

	return (
		<div className="px-4 pt-3 pb-1">
			<div className="flex items-center justify-between mb-2">
				<span className="text-sm font-semibold text-muted-foreground">Recently Played</span>
				<button
					onClick={handleClearAll}
					className="text-[10px] text-primary hover:underline"
				>
					Clear all
				</button>
			</div>
			<ScrollArea className="w-full">
				<div className="flex gap-2.5 pb-2">
					{displayEntries.map(({ key, displayName, entry }) => (
						<button
							key={key}
							onClick={() => onPlay(entry)}
							className="relative min-w-[160px] max-w-[180px] bg-secondary rounded-lg p-2.5 text-left hover:bg-accent transition-colors group shrink-0"
						>
							<div
								className="absolute top-1 right-1.5 opacity-0 group-hover:opacity-100 transition-opacity"
								onClick={(e) => handleDismiss(e, entry.channelId)}
							>
								<X className="h-3 w-3 text-muted-foreground hover:text-foreground" />
							</div>
							<div className="text-xs font-medium truncate pr-4">{displayName}</div>
							{contentType === "live" && (
								<div className="text-[9px] text-primary mt-1">&#9679; LIVE</div>
							)}
						</button>
					))}
				</div>
			</ScrollArea>
		</div>
	);
};
