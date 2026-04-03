import { memo, useState } from "react";
import { Play, Tv2, Heart, Film } from "lucide-react";
import type { Channel, EpgProgram } from "@/lib/types";
import { EpgTimelineBar } from "./EpgTimelineBar";

/** Module-level cache of URLs that failed to load — persists across remounts from virtual list scrolling. */
const brokenImageUrls = new Set<string>();

/** Width (px) of the left channel-info column in RowCard — must match spacer in ChannelList header. */
export const ROW_CARD_LEFT_WIDTH = 180;

interface ChannelCardProps {
	channel: Channel;
	onPlay: (channel: Channel) => void;
	variant?: "row" | "poster";
	onToggleFavorite?: (channel: Channel) => void;
	/** Programs in the EPG display window for this channel. */
	epgPrograms?: EpgProgram[];
	/** Dynamic window start (Unix seconds). Falls back to now − 1h. */
	windowStart?: number;
	/** Dynamic window end (Unix seconds). Falls back to now + 2h. */
	windowEnd?: number;
}

const RowCard = memo(function RowCard({
	channel,
	onPlay,
	onToggleFavorite,
	epgPrograms,
	windowStart,
	windowEnd,
}: {
	channel: Channel;
	onPlay: (ch: Channel) => void;
	onToggleFavorite?: (ch: Channel) => void;
	epgPrograms?: EpgProgram[];
	windowStart?: number;
	windowEnd?: number;
}) {
	const now = Math.floor(Date.now() / 1000);
	const [imgError, setImgError] = useState(() =>
		channel.logoUrl ? brokenImageUrls.has(channel.logoUrl) : false
	);
	const showFallback = !channel.logoUrl || imgError;

	return (
		/* Outer wrapper is a div (not button) so nested buttons and div[role=button] inside
		   EpgTimelineBar are valid HTML and no nesting warnings occur. */
		<div
			role="button"
			tabIndex={0}
			onClick={() => onPlay(channel)}
			onKeyDown={(e) => {
				if (e.key === "Enter" || e.key === " ") {
					e.preventDefault();
					onPlay(channel);
				}
			}}
			className="group flex items-center w-full rounded-lg hover:bg-accent/60 transition-colors cursor-pointer focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
		>
			{/* Left: channel info — fixed width (must match ROW_CARD_LEFT_WIDTH) */}
			<div
				className="flex items-center gap-2 px-2 py-1.5 shrink-0 min-w-0"
				style={{ width: `${ROW_CARD_LEFT_WIDTH}px` }}
			>
				<div className="relative h-6 w-6 rounded bg-secondary flex items-center justify-center overflow-hidden shrink-0">
					{!showFallback ? (
						<img
							src={channel.logoUrl}
							alt=""
							className="h-full w-full object-contain"
							loading="lazy"
							onError={() => {
								if (channel.logoUrl) brokenImageUrls.add(channel.logoUrl);
								setImgError(true);
							}}
						/>
					) : (
						<Tv2 className="h-3 w-3 text-muted-foreground" />
					)}
				</div>

				<div className="min-w-0 flex-1">
					<p className="text-xs leading-tight truncate">{channel.name}</p>
					{channel.groupTitle && (
						<p className="text-[10px] text-muted-foreground/70 truncate mt-0.5">
							{channel.groupTitle}
						</p>
					)}
				</div>
			</div>

			{/* Middle: EPG timeline — flex-1, takes the bulk of row width */}
			<div className="flex-1 min-w-0 py-1.5">
				{epgPrograms && epgPrograms.length > 0 ? (
					<EpgTimelineBar
						programmes={epgPrograms}
						now={now}
						windowStart={windowStart}
						windowEnd={windowEnd}
						height="h-9"
					/>
				) : (
					<div className="h-9 rounded-md border border-border/15 flex items-center justify-center bg-secondary/10">
						<span className="text-[8px] text-muted-foreground/25 select-none">
							no epg
						</span>
					</div>
				)}
			</div>

			{/* Right: LIVE badge + favourite */}
			<div className="flex items-center gap-1 px-2 shrink-0">
				<span className="flex items-center gap-0.5 text-[9px] font-semibold text-red-400 whitespace-nowrap">
					<span className="h-1.5 w-1.5 rounded-full bg-red-400 animate-pulse" />
					LIVE
				</span>

				{onToggleFavorite && (
					<button
						tabIndex={0}
						onClick={(e) => {
							e.stopPropagation();
							onToggleFavorite(channel);
						}}
						className="h-6 w-6 flex items-center justify-center rounded hover:bg-accent shrink-0 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
						aria-label={
							channel.isFavorite ? "Remove from favorites" : "Add to favorites"
						}
					>
						<Heart
							className={`h-3 w-3 transition-colors ${
								channel.isFavorite
									? "fill-current text-red-500"
									: "text-muted-foreground"
							}`}
						/>
					</button>
				)}
			</div>
		</div>
	);
});

const PosterCard = ({
	channel,
	onPlay,
	onToggleFavorite,
}: {
	channel: Channel;
	onPlay: (ch: Channel) => void;
	onToggleFavorite?: (ch: Channel) => void;
}) => {
	const hasSources = channel.sources.length > 0;
	const [imgError, setImgError] = useState(() =>
		channel.logoUrl ? brokenImageUrls.has(channel.logoUrl) : false
	);
	const showFallback = !channel.logoUrl || imgError;

	return (
		<div className="group flex flex-col text-left relative">
			<button
				onClick={() => onPlay(channel)}
				className="focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring rounded-lg w-full"
			>
				<div className="relative w-full aspect-[2/1] rounded-lg overflow-hidden mb-1.5 border border-border/40">
					{!showFallback ? (
						<img
							src={channel.logoUrl}
							alt=""
							className="h-full w-full object-cover transition-transform duration-200 group-hover:scale-105"
							loading="lazy"
							onError={() => {
								if (channel.logoUrl) brokenImageUrls.add(channel.logoUrl);
								setImgError(true);
							}}
						/>
					) : (
						<div className="h-full w-full bg-gradient-to-br from-secondary via-secondary/80 to-secondary/50 flex flex-col items-center justify-center gap-1">
							<Film className="h-5 w-5 text-muted-foreground/25" />
							<span className="text-[9px] font-medium text-muted-foreground/30 uppercase tracking-wider">
								No poster
							</span>
						</div>
					)}
					<div className="absolute inset-0 bg-black/0 group-hover:bg-black/50 transition-colors flex items-center justify-center">
						<div className="h-9 w-9 rounded-full bg-primary flex items-center justify-center opacity-0 group-hover:opacity-100 scale-90 group-hover:scale-100 transition-all duration-150">
							<Play className="h-4 w-4 text-white ml-0.5" />
						</div>
					</div>
					{hasSources && (
						<div className="absolute top-1.5 right-1.5 bg-black/70 text-white text-[9px] font-semibold px-1.5 py-0.5 rounded-full">
							{channel.sources.length + 1}
						</div>
					)}
					{onToggleFavorite && (
						<div
							role="button"
							tabIndex={0}
							onClick={(e) => {
								e.stopPropagation();
								onToggleFavorite(channel);
							}}
							onKeyDown={(e) => {
								if (e.key === "Enter" || e.key === " ") {
									e.preventDefault();
									e.stopPropagation();
									onToggleFavorite(channel);
								}
							}}
							className="absolute top-1 left-1 z-10 h-7 w-7 flex items-center justify-center rounded-full bg-black/50 hover:bg-black/70 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring transition-colors cursor-pointer"
							aria-label={
								channel.isFavorite ? "Remove from favorites" : "Add to favorites"
							}
						>
							<Heart
								className={`h-3.5 w-3.5 transition-colors ${
									channel.isFavorite ? "fill-current text-red-500" : "text-white"
								}`}
							/>
						</div>
					)}
				</div>
				<p className="text-xs leading-snug line-clamp-2 text-foreground/85 group-hover:text-foreground transition-colors px-0.5">
					{channel.name}
				</p>
			</button>
		</div>
	);
};

export const ChannelCard = memo(function ChannelCard({
	channel,
	onPlay,
	variant = "row",
	onToggleFavorite,
	epgPrograms,
	windowStart,
	windowEnd,
}: ChannelCardProps) {
	return variant === "poster" ? (
		<PosterCard channel={channel} onPlay={onPlay} onToggleFavorite={onToggleFavorite} />
	) : (
		<RowCard
			channel={channel}
			onPlay={onPlay}
			onToggleFavorite={onToggleFavorite}
			epgPrograms={epgPrograms}
			windowStart={windowStart}
			windowEnd={windowEnd}
		/>
	);
});
