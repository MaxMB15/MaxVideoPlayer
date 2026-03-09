# Content Deduplication & Bottom Drawer Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Deduplicate movies and series episodes by title/season+episode, fix tab counts, and replace the series centered modal with a unified bottom drawer for both movies and series source selection.

**Architecture:** Pure frontend — all deduplication happens in `useMemo` hooks in `ChannelList`. `ChannelCard` becomes dumb (always calls `onPlay`). Two drawers live in `ChannelList` JSX: a simple movie source drawer and the redesigned series drawer (with season → episode → source steps). `SeriesDetailModal` is rewritten in-place as a bottom drawer.

**Tech Stack:** React + TypeScript, Tailwind CSS v3, lucide-react icons, no new dependencies.

---

### Task 1: Add deduplication pure functions and tests

**Files:**
- Modify: `apps/desktop/src/lib/channels.test.ts`

The test file already contains pure-function tests for channel logic. Add deduplication functions tested in isolation — same pattern as `deriveCategories`.

**Step 1: Add the two pure dedup functions to the test file (above the `describe` blocks)**

Add this block after the `makeChannel` helper (line 30) in `channels.test.ts`:

```typescript
// ── Deduplication helpers (mirrored in ChannelList) ──────────────────────

function dedupeByTitle(channels: Channel[]): Channel[] {
  const seen = new Map<string, Channel>();
  for (const ch of channels) {
    if (!seen.has(ch.name)) {
      seen.set(ch.name, { ...ch, sources: [...ch.sources] });
    } else {
      const existing = seen.get(ch.name)!;
      existing.sources.push(ch.url);
      existing.sources.push(...ch.sources);
      if (!existing.logoUrl && ch.logoUrl) existing.logoUrl = ch.logoUrl;
    }
  }
  return Array.from(seen.values());
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
```

**Step 2: Add tests for `dedupeByTitle`**

Add after the `Channel type` describe block:

```typescript
describe("dedupeByTitle", () => {
  it("keeps single entry unchanged", () => {
    const ch = makeChannel({ contentType: "movie", name: "The Matrix" });
    const result = dedupeByTitle([ch]);
    expect(result).toHaveLength(1);
    expect(result[0].sources).toEqual([]);
  });

  it("merges two entries with same title into one with alternate source", () => {
    const ch1 = makeChannel({ id: "m-1", name: "The Matrix", url: "http://src1/matrix", contentType: "movie" });
    const ch2 = makeChannel({ id: "m-2", name: "The Matrix", url: "http://src2/matrix", contentType: "movie" });
    const result = dedupeByTitle([ch1, ch2]);
    expect(result).toHaveLength(1);
    expect(result[0].url).toBe("http://src1/matrix");
    expect(result[0].sources).toContain("http://src2/matrix");
  });

  it("keeps distinct titles as separate entries", () => {
    const ch1 = makeChannel({ id: "m-1", name: "The Matrix", contentType: "movie" });
    const ch2 = makeChannel({ id: "m-2", name: "Inception", contentType: "movie" });
    expect(dedupeByTitle([ch1, ch2])).toHaveLength(2);
  });

  it("picks up logo from second entry when first has none", () => {
    const ch1 = makeChannel({ id: "m-1", name: "The Matrix", logoUrl: undefined, contentType: "movie" });
    const ch2 = makeChannel({ id: "m-2", name: "The Matrix", logoUrl: "http://logo.png", contentType: "movie" });
    const result = dedupeByTitle([ch1, ch2]);
    expect(result[0].logoUrl).toBe("http://logo.png");
  });

  it("merges sources from all entries", () => {
    const ch1 = makeChannel({ id: "m-1", name: "Film", url: "http://url1", sources: ["http://url1b"], contentType: "movie" });
    const ch2 = makeChannel({ id: "m-2", name: "Film", url: "http://url2", sources: ["http://url2b"], contentType: "movie" });
    const result = dedupeByTitle([ch1, ch2]);
    expect(result[0].sources).toEqual(["http://url1b", "http://url2", "http://url2b"]);
  });

  it("handles empty input", () => {
    expect(dedupeByTitle([])).toEqual([]);
  });

  it("is O(n) — does not regress on large input", () => {
    const channels = Array.from({ length: 10_000 }, (_, i) =>
      makeChannel({ id: `m-${i}`, name: `Movie ${i % 500}`, url: `http://src/${i}`, contentType: "movie" })
    );
    const start = performance.now();
    const result = dedupeByTitle(channels);
    const elapsed = performance.now() - start;
    expect(result).toHaveLength(500);
    expect(elapsed).toBeLessThan(100); // well under 100ms for 10k items
  });
});

describe("dedupeEpisodes", () => {
  it("keeps single episode unchanged", () => {
    const ep = makeChannel({ contentType: "series", season: 1, episode: 1 });
    const result = dedupeEpisodes([ep]);
    expect(result).toHaveLength(1);
    expect(result[0].sources).toEqual([]);
  });

  it("merges duplicate S01E01 from two providers", () => {
    const ep1 = makeChannel({ id: "e-1", contentType: "series", season: 1, episode: 1, url: "http://p1/s1e1" });
    const ep2 = makeChannel({ id: "e-2", contentType: "series", season: 1, episode: 1, url: "http://p2/s1e1" });
    const result = dedupeEpisodes([ep1, ep2]);
    expect(result).toHaveLength(1);
    expect(result[0].sources).toContain("http://p2/s1e1");
  });

  it("keeps different episodes separate", () => {
    const ep1 = makeChannel({ id: "e-1", contentType: "series", season: 1, episode: 1 });
    const ep2 = makeChannel({ id: "e-2", contentType: "series", season: 1, episode: 2 });
    expect(dedupeEpisodes([ep1, ep2])).toHaveLength(2);
  });

  it("treats same episode number in different seasons as different", () => {
    const ep1 = makeChannel({ id: "e-1", contentType: "series", season: 1, episode: 1 });
    const ep2 = makeChannel({ id: "e-2", contentType: "series", season: 2, episode: 1 });
    expect(dedupeEpisodes([ep1, ep2])).toHaveLength(2);
  });
});
```

**Step 3: Run tests to verify they pass**

```bash
cd apps/desktop && npm test -- --run
```

Expected: all new tests PASS (functions are defined in the test file itself).

**Step 4: Commit**

```bash
git add apps/desktop/src/lib/channels.test.ts
git commit -m "test: add deduplication pure function tests"
```

---

### Task 2: Simplify ChannelCard — remove internal source picker

**Files:**
- Modify: `apps/desktop/src/components/channels/ChannelCard.tsx`

`PosterCard` currently manages its own `showSources` state and renders a popover. Remove that entirely — let the parent handle source selection. The card always calls `onPlay(channel)`.

**Step 1: Replace `PosterCard` with simplified version**

In `ChannelCard.tsx`, replace the entire `PosterCard` function (lines 38–126) with:

```tsx
function PosterCard({ channel, onPlay }: { channel: Channel; onPlay: (ch: Channel) => void }) {
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
        </div>
        <p className="text-xs leading-snug line-clamp-2 text-foreground/85 group-hover:text-foreground transition-colors px-0.5">
          {channel.name}
        </p>
      </button>
    </div>
  );
}
```

Also remove the `useState` import if it's no longer used (check: `RowCard` doesn't use it either, so remove `useState` from the import on line 1).

Updated import line 1:
```tsx
import { Play, Tv2 } from "lucide-react";
```

**Step 2: Run tests**

```bash
cd apps/desktop && npm test -- --run
```

Expected: all tests PASS (no tests cover ChannelCard directly).

**Step 3: Commit**

```bash
git add apps/desktop/src/components/channels/ChannelCard.tsx
git commit -m "refactor: remove inline source picker from ChannelCard"
```

---

### Task 3: Redesign SeriesDetailModal as bottom drawer

**Files:**
- Modify: `apps/desktop/src/components/channels/SeriesDetailModal.tsx`

Complete rewrite. Same props interface — `ChannelList` does not need to change how it calls this component.

**Step 1: Replace the entire file content**

```tsx
import { useState, useMemo, useEffect } from "react";
import { X, Play, ChevronLeft, ChevronRight } from "lucide-react";
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

type Step = "episodes" | "sources";

export function SeriesDetailModal({
  showTitle,
  episodes,
  onClose,
  onPlay,
}: SeriesDetailDrawerProps) {
  const [visible, setVisible] = useState(false);
  const [step, setStep] = useState<Step>("episodes");
  const [sourceEp, setSourceEp] = useState<Channel | null>(null);

  // Trigger slide-in on next frame so CSS transition fires
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

  const [selectedSeason, setSelectedSeason] = useState<number>(
    seasons[0]?.[0] ?? 0
  );

  const currentEpisodes = useMemo(
    () => seasons.find(([s]) => s === selectedSeason)?.[1] ?? [],
    [seasons, selectedSeason]
  );

  const handleEpisodeClick = (ep: Channel) => {
    if (ep.sources.length > 0) {
      setSourceEp(ep);
      setStep("sources");
    } else {
      onPlay(ep);
      handleClose();
    }
  };

  const handleSourcePick = (channel: Channel) => {
    onPlay(channel);
    handleClose();
  };

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
        className={`absolute bottom-0 left-0 right-0 bg-card rounded-t-2xl shadow-2xl flex flex-col transition-transform duration-300 ease-out max-h-[80vh] ${
          visible ? "translate-y-0" : "translate-y-full"
        }`}
      >
        {/* Drag handle */}
        <div className="flex justify-center pt-3 pb-1 shrink-0">
          <div className="w-9 h-1 rounded-full bg-border" />
        </div>

        {/* Header */}
        <div className="flex items-center justify-between px-5 py-3 shrink-0">
          {step === "sources" ? (
            <button
              onClick={() => setStep("episodes")}
              className="flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground transition-colors"
            >
              <ChevronLeft className="h-4 w-4" />
              Back
            </button>
          ) : (
            <h2 className="text-sm font-semibold truncate flex-1">{showTitle}</h2>
          )}
          <button
            onClick={handleClose}
            className="shrink-0 text-muted-foreground hover:text-foreground transition-colors ml-3"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {step === "episodes" ? (
          <>
            {/* Season pills */}
            {seasons.length > 1 && (
              <div className="flex gap-1.5 overflow-x-auto px-4 pb-3 shrink-0 scrollbar-hide">
                {seasons.map(([s]) => (
                  <button
                    key={s}
                    onClick={() => setSelectedSeason(s)}
                    className={`px-3.5 py-1.5 text-xs font-medium rounded-full whitespace-nowrap transition-colors ${
                      selectedSeason === s
                        ? "bg-primary text-primary-foreground"
                        : "bg-secondary text-muted-foreground hover:text-foreground"
                    }`}
                  >
                    {s === 0 ? "Unknown" : `Season ${s}`}
                  </button>
                ))}
              </div>
            )}

            <div className="border-t border-border mx-4 shrink-0" />

            {/* Episode list */}
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
                <p className="text-sm text-muted-foreground text-center py-8">
                  No episodes
                </p>
              )}
            </div>
          </>
        ) : (
          /* Source picker step */
          <div className="flex flex-col flex-1 overflow-hidden">
            <div className="px-5 pb-3 shrink-0">
              <p className="text-sm font-semibold truncate">
                {sourceEp && episodeTitle(sourceEp.name)}
              </p>
              <p className="text-xs text-muted-foreground mt-0.5">
                Choose a source to play
              </p>
            </div>
            <div className="border-t border-border mx-4 shrink-0" />
            <div className="overflow-y-auto flex-1 py-2 px-3">
              {sourceEp && (
                <>
                  <button
                    className="w-full flex items-center gap-3 px-3 py-3 rounded-xl hover:bg-accent transition-colors text-left"
                    onClick={() => handleSourcePick(sourceEp)}
                  >
                    <div className="w-8 h-8 rounded-lg bg-primary/10 flex items-center justify-center shrink-0">
                      <Play className="h-3.5 w-3.5 text-primary ml-0.5" />
                    </div>
                    <div>
                      <p className="text-sm font-medium">Source 1</p>
                      <p className="text-xs text-muted-foreground">Default</p>
                    </div>
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
                      <div>
                        <p className="text-sm font-medium">Source {idx + 2}</p>
                      </div>
                    </button>
                  ))}
                </>
              )}
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

Expected: all tests PASS.

**Step 3: Commit**

```bash
git add apps/desktop/src/components/channels/SeriesDetailModal.tsx
git commit -m "feat: redesign series detail as bottom drawer with season/episode/source steps"
```

---

### Task 4: Update ChannelList — movie deduplication, movie drawer, fix counts

**Files:**
- Modify: `apps/desktop/src/components/channels/ChannelList.tsx`

This is the main wiring task. Four changes in one file:
1. Add `movieTitles` dedup memo
2. Update `activeChannels` to use `movieTitles` for movie tab
3. Fix movie tab count badge
4. Add `selectedMovie` state + movie source bottom drawer

**Step 1: Add `movieTitles` memo**

After the existing `seriesShows` useMemo (after line 55), add:

```tsx
const movieTitles = useMemo(() => {
  const seen = new Map<string, Channel>();
  for (const ch of byType.movie) {
    if (!seen.has(ch.name)) {
      seen.set(ch.name, { ...ch, sources: [...ch.sources] });
    } else {
      const existing = seen.get(ch.name)!;
      existing.sources.push(ch.url);
      existing.sources.push(...ch.sources);
      if (!existing.logoUrl && ch.logoUrl) existing.logoUrl = ch.logoUrl;
    }
  }
  return Array.from(seen.values());
}, [byType.movie]);
```

**Step 2: Add `selectedMovie` state**

After the existing `selectedSeriesShow` state declaration, add:

```tsx
const [selectedMovie, setSelectedMovie] = useState<Channel | null>(null);
```

**Step 3: Update `activeChannels`**

Replace the current `activeChannels` useMemo:

```tsx
const activeChannels = useMemo(() => {
  if (activeTab === "series") return seriesShows;
  if (activeTab === "movie") return movieTitles;
  return byType[activeTab];
}, [activeTab, seriesShows, movieTitles, byType]);
```

**Step 4: Fix tab count for movies**

In the tab bar JSX, find the line:
```tsx
const count = id === "series" ? seriesShows.length : byType[id].length;
```

Replace with:
```tsx
const count =
  id === "series" ? seriesShows.length :
  id === "movie"  ? movieTitles.length :
  byType[id].length;
```

**Step 5: Update `handlePlay` to route movies**

Replace the current `handlePlay`:

```tsx
const handlePlay = useCallback(
  (channel: Channel) => {
    if (activeTab === "series") {
      setSelectedSeriesShow(channel.name);
    } else if (activeTab === "movie" && channel.sources.length > 0) {
      setSelectedMovie(channel);
    } else {
      navigate("/player", { state: { url: channel.url, channelName: channel.name } });
    }
  },
  [activeTab, navigate]
);
```

**Step 6: Add movie source drawer JSX**

After the `{/* Series detail modal */}` block in the return, add:

```tsx
{/* Movie source drawer */}
{selectedMovie && (
  <MovieSourceDrawer
    movie={selectedMovie}
    onClose={() => setSelectedMovie(null)}
    onPlay={(ch) =>
      navigate("/player", { state: { url: ch.url, channelName: ch.name } })
    }
  />
)}
```

**Step 7: Add `MovieSourceDrawer` component**

Add this component at the top of `ChannelList.tsx` (before the `TABS` constant, after the imports):

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

  useEffect(() => {
    const id = requestAnimationFrame(() => setVisible(true));
    return () => cancelAnimationFrame(id);
  }, []);

  const handleClose = () => {
    setVisible(false);
    setTimeout(onClose, 300);
  };

  const handlePick = (ch: Channel) => {
    onPlay(ch);
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
        className={`absolute bottom-0 left-0 right-0 bg-card rounded-t-2xl shadow-2xl flex flex-col transition-transform duration-300 ease-out max-h-[60vh] ${
          visible ? "translate-y-0" : "translate-y-full"
        }`}
      >
        <div className="flex justify-center pt-3 pb-1 shrink-0">
          <div className="w-9 h-1 rounded-full bg-border" />
        </div>
        <div className="flex items-center justify-between px-5 py-3 shrink-0">
          <div className="flex-1 min-w-0 mr-3">
            <p className="text-sm font-semibold truncate">{movie.name}</p>
            <p className="text-xs text-muted-foreground mt-0.5">Choose a source to play</p>
          </div>
          <button
            onClick={handleClose}
            className="shrink-0 text-muted-foreground hover:text-foreground transition-colors"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
        <div className="border-t border-border mx-4 shrink-0" />
        <div className="overflow-y-auto flex-1 py-2 px-3 pb-4">
          <button
            className="w-full flex items-center gap-3 px-3 py-3 rounded-xl hover:bg-accent transition-colors text-left"
            onClick={() => handlePick(movie)}
          >
            <div className="w-8 h-8 rounded-lg bg-primary/10 flex items-center justify-center shrink-0">
              <Play className="h-3.5 w-3.5 text-primary ml-0.5" />
            </div>
            <div>
              <p className="text-sm font-medium">Source 1</p>
              <p className="text-xs text-muted-foreground">Default</p>
            </div>
          </button>
          {movie.sources.map((src, idx) => (
            <button
              key={idx}
              className="w-full flex items-center gap-3 px-3 py-3 rounded-xl hover:bg-accent transition-colors text-left"
              onClick={() => handlePick({ ...movie, url: src })}
            >
              <div className="w-8 h-8 rounded-lg bg-secondary flex items-center justify-center shrink-0">
                <Play className="h-3.5 w-3.5 text-muted-foreground ml-0.5" />
              </div>
              <div>
                <p className="text-sm font-medium">Source {idx + 2}</p>
              </div>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
```

**Step 8: Add missing imports to `ChannelList.tsx`**

Ensure these are in the import list at the top:
- `useState` already imported ✓
- `useEffect` — add to the React import
- `X` from lucide-react — add to lucide import
- `Play` from lucide-react — add to lucide import

Updated React import:
```tsx
import { useState, useMemo, useCallback, useRef, useEffect } from "react";
```

Updated lucide import:
```tsx
import { Loader2, Tv2, Clapperboard, MonitorPlay, X, Play } from "lucide-react";
```

**Step 9: Run tests**

```bash
cd apps/desktop && npm test -- --run
```

Expected: all tests PASS.

**Step 10: Commit**

```bash
git add apps/desktop/src/components/channels/ChannelList.tsx
git commit -m "feat: deduplicate movies, fix tab counts, add movie source drawer"
```

---

### Task 5: Final verification

**Step 1: Run full test suite**

```bash
cd apps/desktop && npm test -- --run
```

Expected: all tests PASS, no regressions.

**Step 2: Build check (TypeScript)**

```bash
cd apps/desktop && npx tsc --noEmit
```

Expected: no type errors.

**Step 3: Commit if any fixup needed, then done**
