import { useState, useMemo, useEffect } from "react";
import { X, Play, ChevronLeft, ChevronRight, Star, MonitorPlay } from "lucide-react";
import type { Channel } from "@/lib/types";

interface SeriesDetailDrawerProps {
  showTitle: string;
  episodes: Channel[];
  onClose: () => void;
  onPlay: (channel: Channel) => void;
}

function episodeTitle(name: string): string {
  const stripped = name.replace(/^.*?\bS\d{1,3}E\d{1,3}\s*/i, "").trim();
  return stripped || name;
}

function dedupeEpisodes(episodes: Channel[]): Channel[] {
  const seen = new Map<string, Channel>();
  for (const ep of episodes) {
    const key = `${ep.season ?? 0}x${ep.episode ?? ep.name}`;
    if (!seen.has(key)) {
      seen.set(key, { ...ep, sources: [...ep.sources] });
    } else {
      const existing = seen.get(key)!;
      existing.sources.push(ep.url);
      existing.sources.push(...ep.sources);
      if (!existing.logoUrl && ep.logoUrl) existing.logoUrl = ep.logoUrl;
    }
  }
  return Array.from(seen.values());
}

type Step = "seasons" | "episodes" | "sources";

export function SeriesDetailModal({
  showTitle,
  episodes,
  onClose,
  onPlay,
}: SeriesDetailDrawerProps) {
  const [visible, setVisible] = useState(false);
  const [step, setStep] = useState<Step>("seasons");
  const [selectedSeason, setSelectedSeason] = useState<number | null>(null);
  const [sourceEp, setSourceEp] = useState<Channel | null>(null);

  const showLogoUrl = episodes[0]?.logoUrl;

  useEffect(() => {
    const id = requestAnimationFrame(() => setVisible(true));
    return () => cancelAnimationFrame(id);
  }, []);

  const handleClose = () => {
    setVisible(false);
    setTimeout(onClose, 300);
  };

  const deduped = useMemo(() => dedupeEpisodes(episodes), [episodes]);

  const seasons = useMemo(() => {
    const map = new Map<number, Channel[]>();
    for (const ep of deduped) {
      const s = ep.season ?? 0;
      if (!map.has(s)) map.set(s, []);
      map.get(s)!.push(ep);
    }
    for (const [, eps] of map) {
      eps.sort((a, b) => (a.episode ?? 0) - (b.episode ?? 0));
    }
    return Array.from(map.entries()).sort((a, b) => a[0] - b[0]);
  }, [deduped]);

  const currentEpisodes = useMemo(
    () =>
      selectedSeason !== null
        ? (seasons.find(([s]) => s === selectedSeason)?.[1] ?? [])
        : [],
    [seasons, selectedSeason]
  );

  const handleSeasonClick = (season: number) => {
    setSelectedSeason(season);
    setStep("episodes");
  };

  const handleEpisodeClick = (ep: Channel) => {
    if (ep.sources.length > 0) {
      setSourceEp(ep);
      setStep("sources");
    } else {
      onPlay(ep);
      handleClose();
    }
  };

  const handleBack = () => {
    if (step === "sources") setStep("episodes");
    else if (step === "episodes") setStep("seasons");
  };

  const handleSourcePick = (channel: Channel) => {
    onPlay(channel);
    handleClose();
  };

  const backLabel =
    step === "episodes"
      ? showTitle
      : step === "sources"
      ? selectedSeason === 0
        ? "Unknown Season"
        : `Season ${selectedSeason}`
      : "";

  return (
    <div className="fixed inset-0 z-50">
      {/* Backdrop */}
      <div
        className={`absolute inset-0 bg-black/60 backdrop-blur-sm transition-opacity duration-300 ${
          visible ? "opacity-100" : "opacity-0"
        }`}
        onClick={handleClose}
      />

      {/* Drawer */}
      <div
        className={`absolute bottom-0 left-0 right-0 bg-card rounded-t-2xl shadow-2xl flex flex-col transition-transform duration-300 ease-out max-h-[85vh] ${
          visible ? "translate-y-0" : "translate-y-full"
        }`}
      >
        {/* Handle */}
        <div className="flex justify-center pt-3 pb-1 shrink-0">
          <div className="w-9 h-1 rounded-full bg-border" />
        </div>

        {/* Header row */}
        <div className="flex items-center justify-between px-5 py-2 shrink-0">
          {step !== "seasons" ? (
            <button
              onClick={handleBack}
              className="flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground transition-colors"
            >
              <ChevronLeft className="h-4 w-4" />
              {backLabel}
            </button>
          ) : (
            <div className="flex-1" />
          )}
          <button
            onClick={handleClose}
            aria-label="Close"
            className="shrink-0 text-muted-foreground hover:text-foreground transition-colors ml-3"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Info card — seasons step only */}
        {step === "seasons" && (
          <>
            <div className="flex gap-4 px-5 pb-4 shrink-0">
              <div className="w-20 h-28 rounded-xl bg-secondary overflow-hidden shrink-0 flex items-center justify-center">
                {showLogoUrl ? (
                  <img src={showLogoUrl} alt="" className="h-full w-full object-cover" loading="lazy" />
                ) : (
                  <MonitorPlay className="h-8 w-8 text-muted-foreground/30" />
                )}
              </div>
              <div className="flex flex-col justify-center gap-1.5 flex-1 min-w-0">
                <p className="text-base font-semibold leading-tight line-clamp-2">{showTitle}</p>
                <p className="text-xs text-muted-foreground">Series · —</p>
                <div className="flex items-center gap-1.5 flex-wrap">
                  <span className="flex items-center gap-1 text-[11px] font-semibold bg-yellow-500/15 text-yellow-600 dark:text-yellow-400 px-2 py-0.5 rounded-full">
                    <Star className="h-2.5 w-2.5" /> N/A
                  </span>
                  <span className="text-[11px] font-semibold bg-red-500/15 text-red-500 px-2 py-0.5 rounded-full">
                    🍅 N/A Critics
                  </span>
                  <span className="text-[11px] font-semibold bg-orange-500/15 text-orange-500 px-2 py-0.5 rounded-full">
                    🍿 N/A Audience
                  </span>
                </div>
                <p className="text-xs text-muted-foreground leading-relaxed line-clamp-2">
                  No description available.
                </p>
              </div>
            </div>
            <div className="border-t border-border mx-5 shrink-0" />
          </>
        )}

        {/* Step: Seasons */}
        {step === "seasons" && (
          <div className="overflow-y-auto flex-1 px-3 py-2">
            {seasons.map(([s, eps]) => (
              <button
                key={s}
                onClick={() => handleSeasonClick(s)}
                className="w-full flex items-center gap-3 px-3 py-3 rounded-xl hover:bg-accent transition-colors text-left"
              >
                <div className="w-8 h-8 rounded-lg bg-secondary flex items-center justify-center shrink-0">
                  <span className="text-[10px] font-bold text-muted-foreground">
                    {s === 0 ? "?" : String(s)}
                  </span>
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium">
                    {s === 0 ? "Unknown Season" : `Season ${s}`}
                  </p>
                  <p className="text-xs text-muted-foreground mt-0.5">{eps.length} episodes</p>
                </div>
                <ChevronRight className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
              </button>
            ))}
            {seasons.length === 0 && (
              <p className="text-sm text-muted-foreground text-center py-8">No episodes found</p>
            )}
          </div>
        )}

        {/* Step: Episodes */}
        {step === "episodes" && (
          <div className="overflow-y-auto flex-1 px-3 py-2">
            {currentEpisodes.map((ep) => (
              <button
                key={ep.id}
                onClick={() => handleEpisodeClick(ep)}
                className="w-full flex items-center gap-3 px-3 py-2.5 rounded-xl hover:bg-accent transition-colors text-left"
              >
                <div className="w-8 h-8 rounded-lg bg-secondary flex items-center justify-center shrink-0">
                  <span className="text-[10px] font-bold text-muted-foreground">
                    {ep.episode != null ? String(ep.episode).padStart(2, "0") : "?"}
                  </span>
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm truncate">{episodeTitle(ep.name)}</p>
                </div>
                {ep.sources.length > 0 && (
                  <span className="text-[10px] bg-secondary/80 px-1.5 py-0.5 rounded-full shrink-0 text-muted-foreground font-medium">
                    {ep.sources.length + 1} src
                  </span>
                )}
                <ChevronRight className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
              </button>
            ))}
            {currentEpisodes.length === 0 && (
              <p className="text-sm text-muted-foreground text-center py-8">No episodes</p>
            )}
          </div>
        )}

        {/* Step: Sources */}
        {step === "sources" && sourceEp && (
          <div className="flex flex-col flex-1 overflow-hidden">
            <div className="px-5 pb-3 shrink-0">
              <p className="text-sm font-semibold truncate">{episodeTitle(sourceEp.name)}</p>
              <p className="text-xs text-muted-foreground mt-0.5">Choose a source to play</p>
            </div>
            <div className="border-t border-border mx-4 shrink-0" />
            <div className="overflow-y-auto flex-1 py-2 px-3">
              <button
                className="w-full flex items-center gap-3 px-3 py-3 rounded-xl hover:bg-accent transition-colors text-left"
                onClick={() => handleSourcePick(sourceEp)}
              >
                <div className="w-8 h-8 rounded-lg bg-primary/10 flex items-center justify-center shrink-0">
                  <Play className="h-3.5 w-3.5 text-primary ml-0.5" />
                </div>
                <div className="flex-1">
                  <p className="text-sm font-medium">Source 1</p>
                  <p className="text-xs text-muted-foreground">Default</p>
                </div>
                <ChevronRight className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
              </button>
              {sourceEp.sources.map((src, idx) => (
                <button
                  key={idx}
                  className="w-full flex items-center gap-3 px-3 py-3 rounded-xl hover:bg-accent transition-colors text-left"
                  onClick={() => handleSourcePick({ ...sourceEp, url: src })}
                >
                  <div className="w-8 h-8 rounded-lg bg-secondary flex items-center justify-center shrink-0">
                    <Play className="h-3.5 w-3.5 text-muted-foreground ml-0.5" />
                  </div>
                  <div className="flex-1">
                    <p className="text-sm font-medium">Source {idx + 2}</p>
                  </div>
                  <ChevronRight className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                </button>
              ))}
            </div>
          </div>
        )}

        <div className="shrink-0 pb-2" />
      </div>
    </div>
  );
}
