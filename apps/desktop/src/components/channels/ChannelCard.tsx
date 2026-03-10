import { Play, Tv2, Heart } from "lucide-react";
import type { Channel } from "@/lib/types";

interface ChannelCardProps {
  channel: Channel;
  onPlay: (channel: Channel) => void;
  variant?: "row" | "poster";
  onToggleFavorite?: (channel: Channel) => void;
}

function RowCard({
  channel,
  onPlay,
  onToggleFavorite,
}: {
  channel: Channel;
  onPlay: (ch: Channel) => void;
  onToggleFavorite?: (ch: Channel) => void;
}) {
  return (
    <button
      onClick={() => onPlay(channel)}
      className="group flex items-center gap-3 w-full px-2 py-2 rounded-lg hover:bg-accent transition-colors text-left focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
    >
      <div className="h-8 w-8 rounded bg-secondary flex items-center justify-center overflow-hidden shrink-0">
        {channel.logoUrl ? (
          <img src={channel.logoUrl} alt="" className="h-full w-full object-contain" loading="lazy" />
        ) : (
          <Tv2 className="h-3.5 w-3.5 text-muted-foreground" />
        )}
      </div>
      <div className="flex-1 min-w-0">
        <p className="text-sm leading-tight truncate">{channel.name}</p>
        {channel.groupTitle && (
          <p className="text-[11px] text-muted-foreground truncate mt-0.5">{channel.groupTitle}</p>
        )}
      </div>
      <span className="flex items-center gap-1 text-[10px] font-medium text-red-400 shrink-0">
        <span className="h-1.5 w-1.5 rounded-full bg-red-400 animate-pulse" />
        LIVE
      </span>
      {onToggleFavorite && (
        <button
          onClick={(e) => {
            e.stopPropagation();
            onToggleFavorite(channel);
          }}
          className="h-7 w-7 flex items-center justify-center rounded hover:bg-accent shrink-0 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
          aria-label={channel.isFavorite ? "Remove from favorites" : "Add to favorites"}
        >
          <Heart
            className={`h-4 w-4 transition-colors ${
              channel.isFavorite ? "fill-current text-red-500" : "text-muted-foreground"
            }`}
          />
        </button>
      )}
    </button>
  );
}

function PosterCard({
  channel,
  onPlay,
  onToggleFavorite,
}: {
  channel: Channel;
  onPlay: (ch: Channel) => void;
  onToggleFavorite?: (ch: Channel) => void;
}) {
  const hasSources = channel.sources.length > 0;

  return (
    <div className="group flex flex-col text-left relative">
      <button
        onClick={() => onPlay(channel)}
        className="focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring rounded-lg w-full"
      >
        <div className="relative w-full h-24 rounded-lg bg-secondary overflow-hidden mb-1.5">
          {channel.logoUrl ? (
            <img
              src={channel.logoUrl}
              alt=""
              className="h-full w-full object-cover transition-transform duration-200 group-hover:scale-105"
              loading="lazy"
            />
          ) : (
            <div className="h-full w-full flex items-center justify-center">
              <Play className="h-6 w-6 text-muted-foreground/30" />
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
            <button
              onClick={(e) => {
                e.stopPropagation();
                onToggleFavorite(channel);
              }}
              className="absolute top-1 right-1 z-10 h-7 w-7 flex items-center justify-center rounded-full bg-black/50 hover:bg-black/70 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring transition-colors"
              aria-label={channel.isFavorite ? "Remove from favorites" : "Add to favorites"}
            >
              <Heart
                className={`h-3.5 w-3.5 transition-colors ${
                  channel.isFavorite ? "fill-current text-red-500" : "text-white"
                }`}
              />
            </button>
          )}
        </div>
        <p className="text-xs leading-snug line-clamp-2 text-foreground/85 group-hover:text-foreground transition-colors px-0.5">
          {channel.name}
        </p>
      </button>
    </div>
  );
}

export function ChannelCard({ channel, onPlay, variant = "row", onToggleFavorite }: ChannelCardProps) {
  return variant === "poster"
    ? <PosterCard channel={channel} onPlay={onPlay} onToggleFavorite={onToggleFavorite} />
    : <RowCard channel={channel} onPlay={onPlay} onToggleFavorite={onToggleFavorite} />;
}
