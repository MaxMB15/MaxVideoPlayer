import { Play, Star } from "lucide-react";
import { cn } from "@/lib/utils";
import type { Channel } from "@/lib/types";

interface ChannelCardProps {
  channel: Channel;
  onPlay: (channel: Channel) => void;
  onToggleFavorite?: (channel: Channel) => void;
  variant?: "grid" | "list";
}

export function ChannelCard({
  channel,
  onPlay,
  onToggleFavorite,
  variant = "grid",
}: ChannelCardProps) {
  if (variant === "list") {
    return (
      <button
        onClick={() => onPlay(channel)}
        className="flex items-center gap-3 w-full p-3 rounded-lg hover:bg-accent/50 transition-colors text-left focus:outline-none focus:ring-2 focus:ring-primary"
        tabIndex={0}
      >
        <div className="h-10 w-10 rounded-md bg-secondary flex items-center justify-center overflow-hidden shrink-0">
          {channel.logoUrl ? (
            <img
              src={channel.logoUrl}
              alt=""
              className="h-full w-full object-contain"
              loading="lazy"
            />
          ) : (
            <Play className="h-4 w-4 text-muted-foreground" />
          )}
        </div>
        <div className="flex-1 min-w-0">
          <p className="text-sm font-medium truncate">{channel.name}</p>
          <p className="text-xs text-muted-foreground truncate">
            {channel.groupTitle}
          </p>
        </div>
        {onToggleFavorite && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onToggleFavorite(channel);
            }}
            className="shrink-0 p-1 rounded hover:bg-accent"
          >
            <Star
              className={cn(
                "h-4 w-4",
                channel.isFavorite
                  ? "fill-yellow-400 text-yellow-400"
                  : "text-muted-foreground"
              )}
            />
          </button>
        )}
      </button>
    );
  }

  return (
    <button
      onClick={() => onPlay(channel)}
      className="flex flex-col items-center p-4 rounded-xl bg-card border border-border hover:border-primary/50 hover:bg-accent/30 transition-all focus:outline-none focus:ring-2 focus:ring-primary group"
      tabIndex={0}
    >
      <div className="h-16 w-16 rounded-lg bg-secondary flex items-center justify-center overflow-hidden mb-3">
        {channel.logoUrl ? (
          <img
            src={channel.logoUrl}
            alt=""
            className="h-full w-full object-contain"
            loading="lazy"
          />
        ) : (
          <Play className="h-6 w-6 text-muted-foreground group-hover:text-primary transition-colors" />
        )}
      </div>
      <p className="text-sm font-medium text-center truncate w-full">
        {channel.name}
      </p>
      <p className="text-xs text-muted-foreground truncate w-full text-center">
        {channel.groupTitle}
      </p>
    </button>
  );
}
