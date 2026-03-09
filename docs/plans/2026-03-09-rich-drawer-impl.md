# Rich Movie & Series Drawer Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Redesign the movie source drawer with a poster+info card layout (placeholder IMDB, RT Critics, RT Audience ratings) and redesign the series drawer with a strict 3-step flow: Seasons → Episodes → Sources, with the same info card on the seasons screen.

**Architecture:** Pure frontend — two component rewrites, no new dependencies, no API calls. `MovieSourceDrawer` lives in `ChannelList.tsx`. `SeriesDetailModal.tsx` is rewritten in-place. All rating values are hardcoded "N/A" placeholders. Series step type changes from `"episodes" | "sources"` to `"seasons" | "episodes" | "sources"`.

**Tech Stack:** React + TypeScript, Tailwind CSS v3, lucide-react icons.

---

### Task 1: Redesign MovieSourceDrawer

**Files:**
- Modify: `apps/desktop/src/components/channels/ChannelList.tsx`

No new logic — pure UI rewrite. No tests required (no testable logic introduced).

**Step 1: Update the lucide-react import to add `Star`**

Line 4 of `ChannelList.tsx`. Replace:
```tsx
import { Loader2, Tv2, Clapperboard, MonitorPlay, X, Play } from "lucide-react";
```
With:
```tsx
import { Loader2, Tv2, Clapperboard, MonitorPlay, X, Play, Star } from "lucide-react";
```

**Step 2: Replace the entire `MovieSourceDrawer` function (lines 16–102) with:**

```tsx
function MovieSourceDrawer({
  movie,
  onClose,
  onPlay,
}: {
  movie: Channel;
  onClose: () => void;
  onPlay: (ch: Channel) => void;
}) {
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

  const allSources = [movie.url, ...movie.sources];
  const hasSources = allSources.length > 1;

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
        className={`absolute bottom-0 left-0 right-0 bg-card rounded-t-2xl shadow-2xl flex flex-col transition-transform duration-300 ease-out ${
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
            className="text-muted-foreground hover:text-foreground transition-colors"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Info card: poster left, details right */}
        <div className="flex gap-4 px-5 pb-4 shrink-0">
          <div className="w-20 h-28 rounded-xl bg-secondary overflow-hidden shrink-0 flex items-center justify-center">
            {movie.logoUrl ? (
              <img src={movie.logoUrl} alt="" className="h-full w-full object-cover" loading="lazy" />
            ) : (
              <Clapperboard className="h-8 w-8 text-muted-foreground/30" />
            )}
          </div>
          <div className="flex flex-col justify-center gap-1.5 flex-1 min-w-0">
            <p className="text-base font-semibold leading-tight line-clamp-2">{movie.name}</p>
            <p className="text-xs text-muted-foreground">— · —</p>
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

        {/* Source selector + Play button */}
        <div className="px-5 py-4 flex flex-col gap-3 shrink-0">
          {hasSources && (
            <div className="relative">
              <select
                value={selectedSourceIdx}
                onChange={(e) => setSelectedSourceIdx(Number(e.target.value))}
                className="w-full bg-secondary text-foreground text-sm rounded-xl px-3 py-2.5 border border-border focus:outline-none focus:ring-1 focus:ring-ring appearance-none cursor-pointer pr-8"
              >
                {allSources.map((_, idx) => (
                  <option key={idx} value={idx}>
                    {idx === 0 ? "Source 1 (default)" : `Source ${idx + 1}`}
                  </option>
                ))}
              </select>
              <div className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground">
                <svg className="h-3.5 w-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                </svg>
              </div>
            </div>
          )}
          <button
            onClick={handlePlay}
            className="w-full flex items-center justify-center gap-2 bg-primary text-primary-foreground rounded-xl py-2.5 text-sm font-semibold hover:bg-primary/90 active:bg-primary/80 transition-colors"
          >
            <Play className="h-4 w-4 ml-0.5" />
            Play
          </button>
        </div>

        <div className="shrink-0 pb-2" />
      </div>
    </div>
  );
}
```

**Step 3: Run tests**

```bash
cd apps/desktop && npm test -- --run
```

Expected: all 34 tests PASS.

**Step 4: TypeScript check**

```bash
cd apps/desktop && npx tsc --noEmit
```

Expected: no errors.

**Step 5: Commit**

```bash
git add apps/desktop/src/components/channels/ChannelList.tsx
git commit -m "feat: redesign movie drawer with info card and source dropdown"
```

---

### Task 2: Redesign SeriesDetailModal — 3-step flow with info card

**Files:**
- Modify: `apps/desktop/src/components/channels/SeriesDetailModal.tsx`

Complete rewrite of the file. Same props interface — `ChannelList.tsx` does not change.

**Step 1: Replace the entire file content with:**

```tsx
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
```

**Step 2: Run tests**

```bash
cd apps/desktop && npm test -- --run
```

Expected: all 34 tests PASS.

**Step 3: TypeScript check**

```bash
cd apps/desktop && npx tsc --noEmit
```

Expected: no errors.

**Step 4: Commit**

```bash
git add apps/desktop/src/components/channels/SeriesDetailModal.tsx
git commit -m "feat: redesign series drawer with 3-step seasons/episodes/sources flow and info card"
```
