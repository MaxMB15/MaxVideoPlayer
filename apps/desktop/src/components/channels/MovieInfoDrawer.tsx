import { useState, useEffect } from "react";
import { X, Play, Star, Clapperboard } from "lucide-react";
import { Select } from "@/components/ui/select";
import type { Channel } from "@/lib/types";

interface MovieInfoDrawerProps {
  movie: Channel;
  onClose: () => void;
  onPlay: (ch: Channel) => void;
}

export function MovieInfoDrawer({ movie, onClose, onPlay }: MovieInfoDrawerProps) {
  const [visible, setVisible] = useState(false);
  const [selectedSourceIdx, setSelectedSourceIdx] = useState(0);

  useEffect(() => {
    const id = requestAnimationFrame(() => setVisible(true));
    return () => cancelAnimationFrame(id);
  }, []);

  const handleClose = () => {
    setVisible(false);
    setTimeout(onClose, 300);
  };

  const allSources = [...new Set([movie.url, ...movie.sources])];
  const hasSources = allSources.length > 1;

  const sourceOptions = allSources.map((_, idx) => ({
    value: idx,
    label: idx === 0 ? "Source 1 (default)" : `Source ${idx + 1}`,
  }));

  const handlePlay = () => {
    const url = allSources[selectedSourceIdx];
    onPlay(selectedSourceIdx === 0 ? movie : { ...movie, url });
    handleClose();
  };

  return (
    <div className="fixed inset-0 z-50">
      <div
        className={`absolute inset-0 bg-black/60 backdrop-blur-sm transition-opacity duration-300 ${
          visible ? "opacity-100" : "opacity-0"
        }`}
        onClick={handleClose}
      />
      <div
        className={`absolute bottom-0 left-0 right-0 bg-card rounded-t-2xl shadow-2xl flex flex-col transition-transform duration-300 ease-out max-h-[85vh] overflow-hidden ${
          visible ? "translate-y-0" : "translate-y-full"
        }`}
      >
        {/* Handle */}
        <div className="flex justify-center pt-3 pb-1 shrink-0">
          <div className="w-9 h-1 rounded-full bg-border" />
        </div>

        {/* Close button */}
        <div className="flex justify-end px-5 pt-1 shrink-0">
          <button
            onClick={handleClose}
            aria-label="Close"
            className="text-muted-foreground hover:text-foreground transition-colors"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Side-by-side: movie info (left ~65%) + controls (right ~35%) */}
        <div className="flex gap-4 px-5 pb-5 shrink-0">
          {/* Poster */}
          <div className="w-20 h-28 rounded-xl bg-secondary overflow-hidden shrink-0 flex items-center justify-center">
            {movie.logoUrl ? (
              <img src={movie.logoUrl} alt="" className="h-full w-full object-cover" loading="lazy" />
            ) : (
              <Clapperboard className="h-8 w-8 text-muted-foreground/30" />
            )}
          </div>

          {/* Movie info */}
          <div className="flex flex-col justify-center gap-1.5 flex-[2] min-w-0">
            <p className="text-base font-semibold leading-tight line-clamp-2">{movie.name}</p>
            <p className="text-xs text-muted-foreground">— · —</p>
            <div className="flex items-center gap-1.5 flex-wrap">
              <span className="flex items-center gap-1 text-[11px] font-semibold bg-yellow-500/15 text-yellow-600 dark:text-yellow-400 px-2 py-0.5 rounded-full">
                <Star className="h-2.5 w-2.5" /> N/A
              </span>
              <span className="text-[11px] font-semibold bg-red-500/15 text-red-500 px-2 py-0.5 rounded-full">
                🍅 N/A
              </span>
              <span className="text-[11px] font-semibold bg-orange-500/15 text-orange-500 px-2 py-0.5 rounded-full">
                🍿 N/A
              </span>
            </div>
            <p className="text-xs text-muted-foreground leading-relaxed line-clamp-2">
              No description available.
            </p>
          </div>

          {/* Controls */}
          <div className="flex flex-col justify-center gap-2.5 flex-[1] min-w-0 shrink-0">
            {hasSources && (
              <Select
                value={selectedSourceIdx}
                onChange={setSelectedSourceIdx}
                options={sourceOptions}
                aria-label="Select source"
              />
            )}
            <button
              onClick={handlePlay}
              className="w-full flex items-center justify-center gap-2 bg-primary text-primary-foreground rounded-xl py-2.5 text-sm font-semibold hover:bg-primary/90 active:bg-primary/80 transition-colors"
            >
              <Play className="h-4 w-4 ml-0.5" />
              Play
            </button>
          </div>
        </div>

        <div className="shrink-0 pb-2" />
      </div>
    </div>
  );
}
