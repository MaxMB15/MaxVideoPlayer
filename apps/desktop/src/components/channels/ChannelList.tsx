import { useState, useMemo, useCallback, useRef, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Loader2, Tv2, Clapperboard, MonitorPlay, X, Play } from "lucide-react";
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
