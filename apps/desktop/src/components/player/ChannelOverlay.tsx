import { useDeferredValue, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useChannels } from "@/hooks/useChannels";
import { Search, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { Channel } from "@/lib/types";

const ROW_HEIGHT = 48;

interface ChannelOverlayProps {
	onClose: () => void;
	onSelectChannel: (channel: Channel) => void;
}

export const ChannelOverlay = ({ onClose, onSelectChannel }: ChannelOverlayProps) => {
	const { channels } = useChannels();
	const [query, setQuery] = useState("");
	const deferredQuery = useDeferredValue(query);
	const scrollRef = useRef<HTMLDivElement>(null);

	// Live channels only — the full catalogue (movies + series) can run to
	// hundreds of thousands of entries, which isn't a useful quick-switch list.
	const liveChannels = useMemo(
		() => channels.filter((c) => c.contentType === "live"),
		[channels]
	);

	const visible = useMemo(() => {
		const q = deferredQuery.trim().toLowerCase();
		if (!q) return liveChannels;
		return liveChannels.filter(
			(c) => c.name.toLowerCase().includes(q) || c.groupTitle.toLowerCase().includes(q)
		);
	}, [liveChannels, deferredQuery]);

	// Virtualized: rendering every row at once blocks the UI thread on large playlists.
	const virtualizer = useVirtualizer({
		count: visible.length,
		getScrollElement: () => scrollRef.current,
		estimateSize: () => ROW_HEIGHT,
		overscan: 8,
	});

	const handleSelect = (ch: Channel) => {
		onSelectChannel(ch);
		onClose();
	};

	return (
		<div className="absolute right-0 top-0 bottom-0 w-80 bg-black/90 backdrop-blur-sm border-l border-white/10 flex flex-col">
			<div className="flex items-center justify-between p-3 border-b border-white/10">
				<h3 className="text-sm font-semibold text-white">Channels</h3>
				<Button
					variant="ghost"
					size="icon"
					onClick={onClose}
					className="h-7 w-7 text-white hover:bg-white/20"
				>
					<X className="h-4 w-4" />
				</Button>
			</div>
			<div className="p-2 border-b border-white/10">
				<div className="relative">
					<Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-white/40" />
					<input
						value={query}
						onChange={(e) => setQuery(e.target.value)}
						onKeyDown={(e) => {
							// Player hotkeys are suppressed while typing, so handle Escape here.
							if (e.key === "Escape") {
								e.preventDefault();
								if (query) setQuery("");
								else onClose();
							}
						}}
						placeholder="Filter channels"
						aria-label="Filter channels"
						className="w-full h-8 pl-7 pr-2 rounded-md bg-white/10 text-xs text-white placeholder:text-white/40 focus:outline-none focus:ring-1 focus:ring-primary"
					/>
				</div>
			</div>
			<div ref={scrollRef} className="flex-1 overflow-y-auto">
				{visible.length === 0 ? (
					<p className="text-xs text-white/50 text-center py-8">
						{liveChannels.length === 0 ? "No live channels" : "No matching channels"}
					</p>
				) : (
					<div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
						{virtualizer.getVirtualItems().map((row) => {
							const ch = visible[row.index];
							return (
								<div
									key={ch.id}
									className="absolute left-0 right-0 px-2"
									style={{ top: row.start, height: ROW_HEIGHT }}
								>
									<button
										onClick={() => handleSelect(ch)}
										className="flex items-center gap-2 w-full h-full px-2 rounded-md text-left text-white hover:bg-white/10 transition-colors focus:outline-none focus:ring-1 focus:ring-primary"
									>
										<div className="h-8 w-8 rounded bg-white/10 flex items-center justify-center overflow-hidden shrink-0">
											{ch.logoUrl ? (
												<img
													src={ch.logoUrl}
													alt=""
													className="h-full w-full object-contain"
													loading="lazy"
												/>
											) : (
												<span className="text-[10px] text-white/50">
													TV
												</span>
											)}
										</div>
										<div className="min-w-0 flex-1">
											<p className="text-xs font-medium truncate">
												{ch.name}
											</p>
											<p className="text-[10px] text-white/50 truncate">
												{ch.groupTitle}
											</p>
										</div>
									</button>
								</div>
							);
						})}
					</div>
				)}
			</div>
		</div>
	);
};
