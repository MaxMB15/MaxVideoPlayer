import { useState, useMemo, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useRef } from "react";
import { LayoutGrid, List, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { SearchBar } from "./SearchBar";
import { CategoryFilter } from "./CategoryFilter";
import { ChannelCard } from "./ChannelCard";
import { useChannels } from "@/hooks/useChannels";
import { usePlatform } from "@/hooks/usePlatform";
import type { Channel } from "@/lib/types";

export function ChannelList() {
  const { channels, categories, loading } = useChannels();
  const { layoutMode } = usePlatform();
  const navigate = useNavigate();

  const [search, setSearch] = useState("");
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const [viewMode, setViewMode] = useState<"grid" | "list">(
    layoutMode === "tv" ? "grid" : "list"
  );

  const filtered = useMemo(() => {
    let result = channels;
    if (selectedCategory) {
      result = result.filter((ch) => ch.groupTitle === selectedCategory);
    }
    if (search.trim()) {
      const lower = search.toLowerCase();
      result = result.filter((ch) => ch.name.toLowerCase().includes(lower));
    }
    return result;
  }, [channels, selectedCategory, search]);

  const parentRef = useRef<HTMLDivElement>(null);

  const columnsPerRow = viewMode === "grid" ? (layoutMode === "tv" ? 5 : 4) : 1;
  const rowCount = Math.ceil(filtered.length / columnsPerRow);

  const virtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => parentRef.current,
    estimateSize: () => (viewMode === "grid" ? 160 : 56),
    overscan: 5,
  });

  const handlePlay = useCallback(
    (channel: Channel) => {
      navigate("/player", {
        state: { url: channel.url, channelName: channel.name },
      });
    },
    [navigate]
  );

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <Loader2 className="h-8 w-8 animate-spin text-primary" />
      </div>
    );
  }

  if (channels.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-full gap-4 p-8">
        <div className="text-center">
          <h2 className="text-xl font-semibold mb-2">No Channels</h2>
          <p className="text-muted-foreground mb-4">
            Add a playlist to get started with your channels.
          </p>
          <Button onClick={() => navigate("/playlists")}>Add Playlist</Button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full p-4 gap-4">
      <div className="flex items-center gap-3">
        <div className="flex-1">
          <SearchBar value={search} onChange={setSearch} />
        </div>
        <Button
          variant="ghost"
          size="icon"
          onClick={() => setViewMode(viewMode === "grid" ? "list" : "grid")}
        >
          {viewMode === "grid" ? (
            <List className="h-4 w-4" />
          ) : (
            <LayoutGrid className="h-4 w-4" />
          )}
        </Button>
      </div>

      {categories.length > 0 && (
        <CategoryFilter
          categories={categories}
          selected={selectedCategory}
          onSelect={setSelectedCategory}
        />
      )}

      <div className="text-xs text-muted-foreground">
        {filtered.length} channel{filtered.length !== 1 ? "s" : ""}
      </div>

      <div ref={parentRef} className="flex-1 overflow-auto scrollbar-hide">
        <div
          style={{
            height: `${virtualizer.getTotalSize()}px`,
            width: "100%",
            position: "relative",
          }}
        >
          {virtualizer.getVirtualItems().map((virtualRow) => {
            const startIdx = virtualRow.index * columnsPerRow;
            const rowChannels = filtered.slice(
              startIdx,
              startIdx + columnsPerRow
            );
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
                {viewMode === "grid" ? (
                  <div
                    className="grid gap-3"
                    style={{
                      gridTemplateColumns: `repeat(${columnsPerRow}, minmax(0, 1fr))`,
                    }}
                  >
                    {rowChannels.map((ch) => (
                      <ChannelCard
                        key={ch.id}
                        channel={ch}
                        onPlay={handlePlay}
                        variant="grid"
                      />
                    ))}
                  </div>
                ) : (
                  rowChannels.map((ch) => (
                    <ChannelCard
                      key={ch.id}
                      channel={ch}
                      onPlay={handlePlay}
                      variant="list"
                    />
                  ))
                )}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
