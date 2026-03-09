import { useState, useMemo, useCallback, useRef, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Loader2, Tv2, Clapperboard, MonitorPlay, X, Play, Star } from "lucide-react";
import { Button } from "@/components/ui/button";
import { SearchBar } from "./SearchBar";
import { CategoryFilter } from "./CategoryFilter";
import { ChannelCard } from "./ChannelCard";
import { SeriesDetailModal } from "./SeriesDetailModal";
import { useChannels } from "@/hooks/useChannels";
import { usePlatform } from "@/hooks/usePlatform";
import type { Channel, Category } from "@/lib/types";

type Tab = "live" | "movie" | "series";

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

const TABS: { id: Tab; label: string; icon: React.ElementType }[] = [
  { id: "live",   label: "Live",    icon: Tv2 },
  { id: "movie",  label: "Movies",  icon: Clapperboard },
  { id: "series", label: "Series",  icon: MonitorPlay },
];

function showTitle(name: string): string {
  return name.replace(/\s+S\d{1,3}E\d{1,3}.*/i, "").trim();
}

export function ChannelList() {
  const { channels, loading } = useChannels();
  const { layoutMode } = usePlatform();
  const navigate = useNavigate();

  const [activeTab, setActiveTab] = useState<Tab>("live");
  const [search, setSearch] = useState("");
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const [selectedSeriesShow, setSelectedSeriesShow] = useState<string | null>(null);
  const [selectedMovie, setSelectedMovie] = useState<Channel | null>(null);

  const byType = useMemo(() => {
    const map: Record<Tab, Channel[]> = { live: [], movie: [], series: [] };
    for (const ch of channels) {
      const t = ch.contentType as Tab;
      if (t in map) map[t].push(ch);
      else map.live.push(ch);
    }
    return map;
  }, [channels]);

  // For series: deduplicate to show-level (one entry per unique show title).
  // Clear sources so clicking a show card always opens the SeriesDetailModal.
  const seriesShows = useMemo(() => {
    const seen = new Map<string, Channel>();
    for (const ch of byType.series) {
      const title = ch.seriesTitle ?? showTitle(ch.name);
      if (!seen.has(title)) seen.set(title, { ...ch, name: title, sources: [] });
    }
    return Array.from(seen.values());
  }, [byType.series]);

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

  const activeChannels = useMemo(() => {
    if (activeTab === "series") return seriesShows;
    if (activeTab === "movie") return movieTitles;
    return byType[activeTab];
  }, [activeTab, seriesShows, movieTitles, byType]);

  const categories = useMemo<Category[]>(() => {
    if (activeTab === "series") return [];
    const counts: Record<string, number> = {};
    for (const ch of byType[activeTab]) {
      const key = ch.groupTitle || "";
      counts[key] = (counts[key] ?? 0) + 1;
    }
    return Object.entries(counts)
      .map(([name, channelCount]) => ({ id: name, name, channelCount }))
      .sort((a, b) => b.channelCount - a.channelCount);
  }, [byType, activeTab]);

  const handleTabChange = (tab: Tab) => {
    setActiveTab(tab);
    setSelectedCategory(null);
    setSearch("");
  };

  const filtered = useMemo(() => {
    let result = activeChannels;
    if (selectedCategory && activeTab !== "series") {
      result = result.filter((ch) => ch.groupTitle === selectedCategory);
    }
    if (search.trim()) {
      const lower = search.toLowerCase();
      result = result.filter((ch) => ch.name.toLowerCase().includes(lower));
    }
    return result;
  }, [activeChannels, selectedCategory, search, activeTab]);

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

  const isGrid = activeTab !== "live";
  const columnsPerRow = isGrid ? (layoutMode === "tv" ? 6 : 6) : 1;
  const rowCount = Math.ceil(filtered.length / columnsPerRow);

  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => parentRef.current,
    estimateSize: () => (isGrid ? 145 : 52),
    overscan: 4,
  });

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <Loader2 className="h-5 w-5 animate-spin text-primary" />
      </div>
    );
  }

  if (channels.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-full gap-3 p-8 text-center">
        <p className="text-base font-semibold">No channels yet</p>
        <p className="text-sm text-muted-foreground">Add a playlist to start watching.</p>
        <Button onClick={() => navigate("/playlists")} size="sm" className="mt-1">Add Playlist</Button>
      </div>
    );
  }

  const countLabel = activeTab === "live" ? "channels" : activeTab === "movie" ? "movies" : "shows";

  return (
    <div className="flex flex-col h-full">
      {/* Tab bar */}
      <div className="flex items-center gap-0 border-b border-border px-3 shrink-0">
        {TABS.map(({ id, label, icon: Icon }) => {
          const count =
            id === "series" ? seriesShows.length :
            id === "movie"  ? movieTitles.length :
            byType[id].length;
          return (
            <button
              key={id}
              onClick={() => handleTabChange(id)}
              className={`flex items-center gap-1.5 px-4 py-3 text-sm font-medium border-b-2 transition-colors ${
                activeTab === id
                  ? "border-primary text-primary"
                  : "border-transparent text-muted-foreground hover:text-foreground"
              }`}
            >
              <Icon className="h-3.5 w-3.5" />
              {label}
              <span className={`text-[11px] px-1.5 py-0.5 rounded-full tabular-nums ${
                activeTab === id ? "bg-primary/15 text-primary" : "bg-muted text-muted-foreground"
              }`}>
                {count.toLocaleString()}
              </span>
            </button>
          );
        })}
        <div className="flex-1" />
        <SearchBar value={search} onChange={setSearch} />
      </div>

      {/* Category filter — only for live and movies, not series (too many groups) */}
      {categories.length > 1 && activeTab !== "series" && (
        <div className="shrink-0 px-3 pt-2.5">
          <CategoryFilter categories={categories} selected={selectedCategory} onSelect={setSelectedCategory} />
        </div>
      )}

      {/* Result count */}
      <div className="shrink-0 px-3 pt-2 pb-1">
        <span className="text-xs text-muted-foreground">
          {filtered.length.toLocaleString()} {countLabel}
        </span>
      </div>

      {/* Series detail modal */}
      {selectedSeriesShow && (
        <SeriesDetailModal
          showTitle={selectedSeriesShow}
          episodes={byType.series.filter(
            (ep) => (ep.seriesTitle ?? showTitle(ep.name)) === selectedSeriesShow
          )}
          onClose={() => setSelectedSeriesShow(null)}
          onPlay={(ch) =>
            navigate("/player", { state: { url: ch.url, channelName: ch.name } })
          }
        />
      )}

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

      {/* Virtual list */}
      <div ref={parentRef} className="flex-1 overflow-auto scrollbar-hide px-3 pb-3">
        <div style={{ height: `${virtualizer.getTotalSize()}px`, width: "100%", position: "relative" }}>
          {virtualizer.getVirtualItems().map((virtualRow) => {
            const startIdx = virtualRow.index * columnsPerRow;
            const rowChannels = filtered.slice(startIdx, startIdx + columnsPerRow);
            return (
              <div
                key={virtualRow.key}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: "100%",
                  height: `${virtualRow.size}px`,
                  transform: `translateY(${virtualRow.start}px)`,
                }}
              >
                {isGrid ? (
                  <div
                    className="grid gap-3 pt-1"
                    style={{ gridTemplateColumns: `repeat(${columnsPerRow}, minmax(0, 1fr))` }}
                  >
                    {rowChannels.map((ch) => (
                      <ChannelCard key={ch.id} channel={ch} onPlay={handlePlay} variant="poster" />
                    ))}
                  </div>
                ) : (
                  <div className="flex flex-col">
                    {rowChannels.map((ch) => (
                      <ChannelCard key={ch.id} channel={ch} onPlay={handlePlay} variant="row" />
                    ))}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
