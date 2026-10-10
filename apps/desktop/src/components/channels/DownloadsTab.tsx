// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Max Boksem. See NOTICE for additional terms under GPLv3 section 7.

import { useChannels } from "@/hooks/useChannels";
import { useDownloads } from "@/hooks/useDownloads";
import { ChannelCard } from "./ChannelCard";
import type { Channel } from "@/lib/types";

interface DownloadsTabProps {
	onPlay: (channel: Channel) => void;
	onToggleFavorite?: (channel: Channel) => void;
}

export const DownloadsTab = ({ onPlay, onToggleFavorite }: DownloadsTabProps) => {
	const { channels } = useChannels();
	const { byChannel, bySeries } = useDownloads();

	const ids = new Set<string>();
	// Standalone cards only for movie downloads — individual episodes are
	// represented by their parent series card (added from bySeries below).
	for (const [channelId, rec] of byChannel) {
		if (rec.kind === "movie") ids.add(channelId);
	}
	for (const [seriesId] of bySeries) ids.add(seriesId);

	const items = channels.filter((c) => ids.has(c.id));

	if (items.length === 0) {
		return (
			<div className="flex items-center justify-center h-40 text-sm text-muted-foreground">
				No downloads yet.
			</div>
		);
	}

	return (
		<div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6 gap-3 p-4">
			{items.map((c) => (
				<ChannelCard
					key={c.id}
					channel={c}
					variant="poster"
					onPlay={onPlay}
					onToggleFavorite={onToggleFavorite}
				/>
			))}
		</div>
	);
};
