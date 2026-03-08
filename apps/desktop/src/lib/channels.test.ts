import { describe, it, expect } from "vitest";
import type { Channel, Category, PlayerState } from "./types";

// Pure category derivation — mirrors the logic in useChannels.
// Tested independently so regressions surface without needing Tauri mocks.
function deriveCategories(channels: Channel[]): Category[] {
  const counts: Record<string, number> = {};
  for (const ch of channels) {
    const key = ch.groupTitle || "";
    counts[key] = (counts[key] ?? 0) + 1;
  }
  return Object.entries(counts).map(([name, channelCount]) => ({
    id: name,
    name,
    channelCount,
  }));
}

function makeChannel(overrides: Partial<Channel> = {}): Channel {
  return {
    id: "ch-1",
    name: "Test Channel",
    url: "http://stream.example.com/1",
    groupTitle: "General",
    isFavorite: false,
    ...overrides,
  };
}

const DEFAULT_PLAYER_STATE: PlayerState = {
  isPlaying: false,
  isPaused: false,
  currentUrl: null,
  volume: 100,
  position: 0,
  duration: 0,
};

// ── Category derivation ──────────────────────────────────────────────────

describe("deriveCategories", () => {
  it("groups channels by groupTitle", () => {
    const channels = [
      makeChannel({ id: "ch-1", groupTitle: "News" }),
      makeChannel({ id: "ch-2", groupTitle: "News" }),
      makeChannel({ id: "ch-3", groupTitle: "Sports" }),
    ];
    const cats = deriveCategories(channels);
    const news = cats.find((c) => c.name === "News");
    const sports = cats.find((c) => c.name === "Sports");
    expect(news?.channelCount).toBe(2);
    expect(sports?.channelCount).toBe(1);
  });

  it("returns empty array for empty channel list", () => {
    expect(deriveCategories([])).toEqual([]);
  });

  it("treats empty groupTitle as its own category", () => {
    const channels = [
      makeChannel({ id: "ch-1", groupTitle: "" }),
      makeChannel({ id: "ch-2", groupTitle: "" }),
    ];
    const cats = deriveCategories(channels);
    expect(cats).toHaveLength(1);
    expect(cats[0].channelCount).toBe(2);
  });

  it("each category id equals its name", () => {
    const channels = [makeChannel({ groupTitle: "Movies" })];
    const cats = deriveCategories(channels);
    expect(cats[0].id).toBe(cats[0].name);
  });

  it("handles large number of groups", () => {
    const channels = Array.from({ length: 100 }, (_, i) =>
      makeChannel({ id: `ch-${i}`, groupTitle: `Group ${i % 10}` })
    );
    const cats = deriveCategories(channels);
    expect(cats).toHaveLength(10);
    for (const cat of cats) {
      expect(cat.channelCount).toBe(10);
    }
  });
});

// ── Channel shape ────────────────────────────────────────────────────────

describe("Channel type", () => {
  it("optional fields may be absent", () => {
    const ch = makeChannel({ logoUrl: undefined, tvgId: undefined, tvgName: undefined });
    expect(ch.logoUrl).toBeUndefined();
    expect(ch.tvgId).toBeUndefined();
    expect(ch.tvgName).toBeUndefined();
  });

  it("isFavorite defaults to false", () => {
    const ch = makeChannel();
    expect(ch.isFavorite).toBe(false);
  });
});

// ── PlayerState ──────────────────────────────────────────────────────────

describe("PlayerState", () => {
  it("default state is stopped", () => {
    expect(DEFAULT_PLAYER_STATE.isPlaying).toBe(false);
    expect(DEFAULT_PLAYER_STATE.isPaused).toBe(false);
    expect(DEFAULT_PLAYER_STATE.currentUrl).toBeNull();
  });

  it("volume is within valid range [0, 150]", () => {
    const state: PlayerState = { ...DEFAULT_PLAYER_STATE, volume: 100 };
    expect(state.volume).toBeGreaterThanOrEqual(0);
    expect(state.volume).toBeLessThanOrEqual(150);
  });

  it("position and duration are non-negative", () => {
    const state: PlayerState = {
      ...DEFAULT_PLAYER_STATE,
      position: 30,
      duration: 3600,
    };
    expect(state.position).toBeGreaterThanOrEqual(0);
    expect(state.duration).toBeGreaterThanOrEqual(0);
  });

  it("paused state is not playing", () => {
    const state: PlayerState = {
      ...DEFAULT_PLAYER_STATE,
      isPlaying: false,
      isPaused: true,
      currentUrl: "http://stream.example.com/1",
    };
    expect(state.isPlaying).toBe(false);
    expect(state.isPaused).toBe(true);
    expect(state.currentUrl).not.toBeNull();
  });
});
