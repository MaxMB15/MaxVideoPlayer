import { useState, useEffect, useCallback } from "react";
import { X } from "lucide-react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { getWatchHistory, deleteHistoryEntry } from "@/lib/tauri";
import type { WatchHistoryEntry } from "@/lib/types";

interface RecentlyPlayedRowProps {
	contentType: "live" | "movie" | "series";
	onPlay: (entry: WatchHistoryEntry) => void;
}

export const RecentlyPlayedRow = ({ contentType, onPlay }: RecentlyPlayedRowProps) => {
	const [entries, setEntries] = useState<WatchHistoryEntry[]>([]);

	const load = useCallback(async () => {
		const history = await getWatchHistory(20);
		setEntries(history.filter((e) => e.contentType === contentType));
	}, [contentType]);

	useEffect(() => {
		load();
	}, [load]);

	const handleDismiss = async (e: React.MouseEvent, channelId: string) => {
		e.stopPropagation();
		await deleteHistoryEntry(channelId);
		setEntries((prev) => prev.filter((entry) => entry.channelId !== channelId));
	};

	const handleClearAll = async () => {
		for (const entry of entries) await deleteHistoryEntry(entry.channelId);
		setEntries([]);
	};

	if (entries.length === 0) return null;

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
					{entries.map((entry) => (
						<button
							key={entry.channelId}
							onClick={() => onPlay(entry)}
							className="relative min-w-[160px] max-w-[180px] bg-secondary rounded-lg p-2.5 text-left hover:bg-accent transition-colors group shrink-0"
						>
							<div
								className="absolute top-1 right-1.5 opacity-0 group-hover:opacity-100 transition-opacity"
								onClick={(e) => handleDismiss(e, entry.channelId)}
							>
								<X className="h-3 w-3 text-muted-foreground hover:text-foreground" />
							</div>
							<div className="text-xs font-medium truncate pr-4">
								{entry.channelName}
							</div>
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
