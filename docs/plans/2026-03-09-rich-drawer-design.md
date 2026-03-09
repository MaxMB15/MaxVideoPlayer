# Rich Movie & Series Drawer Design

**Date:** 2026-03-09
**Status:** Approved

## Goal

Redesign the movie source drawer and series detail drawer to show rich content info (poster, ratings, metadata) alongside source/navigation controls. Series drawer gets a strict 3-step flow: Seasons → Episodes → Sources.

## Movie Drawer

Bottom slide-up drawer. Single screen.

**Layout:** Poster + info side-by-side (Option A), source controls below.

```
┌──────────────────────────────────┐
│          ▬▬▬  (handle)           │
│ ┌──────┐  Movie Title            │
│ │poster│  2024 · Action, Sci-Fi  │
│ │      │  ⭐ N/A  🍅 N/A  🍿 N/A│
│ └──────┘  Short placeholder plot │
├──────────────────────────────────┤
│  ┌──────────────────────────┐    │
│  │ ▼  Source 1 (default)    │    │  ← styled <select>, only if >1 source
│  └──────────────────────────┘    │
│  [▶ Play]                        │
└──────────────────────────────────┘
```

**Details:**
- Poster: uses `channel.logoUrl` if available, falls back to a placeholder icon
- Ratings row: IMDB (gold star), RT Critics (tomato), RT Audience (popcorn) — all "N/A" placeholders
- Year / genre: "—" placeholder
- If only 1 source: no dropdown, just Play button
- If multiple sources: styled `<select>` above Play button, default = source 1

## Series Drawer

Bottom slide-up drawer. Three steps.

### Step 1 — Seasons
Info card (same layout as movie) at top, season rows below.

```
┌──────────────────────────────────┐
│          ▬▬▬  (handle)           │
│ ┌──────┐  Show Title             │
│ │poster│  Series                 │
│ │      │  ⭐ N/A  🍅 N/A  🍿 N/A│
│ └──────┘  Placeholder tagline    │
├──────────────────────────────────┤
│  Season 1  ·  12 episodes     →  │
│  Season 2  ·  8 episodes      →  │
│  Unknown   ·  ? episodes      →  │  ← season 0 shown as "Unknown"
└──────────────────────────────────┘
```

### Step 2 — Episodes
Info card replaced by back button + season label. Episode rows.

```
┌──────────────────────────────────┐
│ ← Season 1               [✕]    │
├──────────────────────────────────┤
│  E01  Pilot                   →  │
│  E02  The Train Job  [2 src]  →  │
└──────────────────────────────────┘
```

- Episode number shown as "E01", "E02" etc.; "?" if undefined
- "X src" badge shown if episode has multiple sources
- Single-source episode: clicking plays directly (skips step 3)

### Step 3 — Sources
Back button + episode title. Source rows.

```
┌──────────────────────────────────┐
│ ← Pilot                  [✕]    │
├──────────────────────────────────┤
│  ▶  Source 1 (default)       →  │
│  ▶  Source 2                 →  │
└──────────────────────────────────┘
```

## Architecture

- `MovieSourceDrawer`: self-contained component in `ChannelList.tsx` — redesigned in-place
- `SeriesDetailModal.tsx`: complete rewrite — steps = `"seasons" | "episodes" | "sources"`
- No new dependencies — Tailwind + lucide-react only
- Ratings/metadata: all placeholder values hardcoded in component (no API calls)

## Data Notes

- The Rust backend already deduplicates episodes by `(series_title, season, episode)` at parse time — frontend `dedupeEpisodes` in the modal is a safety net for providers that don't populate those fields
- `season === 0` or `season === undefined` → shown as "Unknown" season
- `episode === undefined` → shown as "?" in episode number slot
