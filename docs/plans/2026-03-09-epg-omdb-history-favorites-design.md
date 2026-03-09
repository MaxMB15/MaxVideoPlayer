# EPG, OMDB, Watch History & Favorites — Design Doc

**Date:** 2026-03-09

---

## Overview

Four interconnected features:

1. **EPG** — Electronic Programme Guide for live TV channels, sourced from M3U headers, Xtream API, or manual URL input
2. **OMDB** — Rich metadata (ratings, plot, cast) for movies and TV series via the OMDB API, lazy-fetched and cached
3. **Watch History** — Persistent cross-session history with play count and total watch time per channel
4. **Favorites** — Heart icon on cards, per-tab filter, and a dedicated Favorites tab

**Architecture decision:** All data stored in SQLite via `mvp-core`'s `store.rs`. Tauri Store plugin for the OMDB API key. EPG refresh settings follow the existing playlist refresh pattern (localStorage per-provider).

---

## 1. EPG

### Data Sources

- **M3U:** Parse `x-tvg-url` attribute from the `#EXTM3U` header line
- **Xtream:** `{server}/xmltv.php?username=X&password=Y` endpoint
- **Manual:** User enters/overrides EPG URL in ProviderSettingsModal

Detected EPG URL stored in a new `epg_url TEXT` column on the `providers` table.

### DB Schema

```sql
-- New column on existing providers table
ALTER TABLE providers ADD COLUMN epg_url TEXT;

-- New table for parsed EPG programme data
CREATE TABLE IF NOT EXISTS epg_programmes (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    channel_id  TEXT NOT NULL,      -- matches tvg_id on channels
    title       TEXT NOT NULL,
    description TEXT,
    start_time  INTEGER NOT NULL,   -- Unix timestamp (seconds)
    end_time    INTEGER NOT NULL,   -- Unix timestamp (seconds)
    category    TEXT,
    provider_id TEXT NOT NULL,
    fetched_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_epg_channel_time
    ON epg_programmes(channel_id, start_time);
```

Old EPG rows for a provider are deleted and replaced on each refresh.

### Channel Matching

`channels.tvg_id` → `epg_programmes.channel_id`. Fallback: case-insensitive match on `channels.tvg_name`.

### Refresh Settings (localStorage per-provider)

```ts
interface ProviderSettings {
  autoRefresh: boolean;
  refreshIntervalHours: number;
  epgAutoRefresh: boolean;        // default: true
  epgRefreshIntervalHours: number; // default: 24
}
```

Runs on app startup (if overdue based on `fetched_at`) and on the same interval timer used for playlist refresh.

### Tauri Commands

- `get_epg_programmes(provider_id, channel_id, date_unix_start, date_unix_end) → Vec<EpgProgram>`
- `refresh_epg(provider_id) → ()` — fetches XML, parses, replaces rows
- `get_epg_url(provider_id) → Option<String>` — returns stored URL
- `set_epg_url(provider_id, url) → ()` — saves manual URL
- `detect_epg_url(provider_id) → Option<String>` — auto-detects from M3U header or Xtream

### Live Info Drawer UI

```
┌─────────────────────────────────────────────────────┐
│  [Channel Logo]  Channel Name          [✕ close]    │
│  ─────────────────────────────────────────────────  │
│  NOW PLAYING                                        │
│  ████████████████░░░░░░  14:32 → 15:15  (43 min)   │
│  The Late Show with Stephen Colbert                 │
│  Comedy talk show. Tonight: guests include...       │
│  ─────────────────────────────────────────────────  │
│  TODAY'S SCHEDULE                                   │
│  │ 13:00  Previous Show           ✓ past            │
│  │ 14:32  The Late Show           ▶ now  (highlight)│
│  │ 15:15  Evening News                              │
│  │ 16:00  Documentary Hour                          │
└─────────────────────────────────────────────────────┘
```

- Drawer slides up same as other info drawers
- Schedule list auto-scrolls to current programme on open
- Progress bar updates every 30 seconds
- Past programmes shown in muted colour
- Empty states:
  - No EPG URL configured → prompt with link to provider settings
  - No `tvg_id` match → "Schedule unavailable for this channel"
  - Loading → spinner

---

## 2. OMDB

### API Key Storage

`tauri-plugin-store` → `settings.json` in the app's data directory. Never in localStorage or source code. Each user provides their own free OMDB key (1000 calls/day per user).

### DB Schema

```sql
CREATE TABLE IF NOT EXISTS omdb_cache (
    channel_id   TEXT PRIMARY KEY,
    data_json    TEXT NOT NULL,
    fetched_at   INTEGER NOT NULL   -- Unix timestamp; TTL = 30 days
);
```

### Fetch Strategy

- Triggered **only** when user opens `MovieInfoDrawer` or `SeriesDetailModal`
- Check cache first: if row exists and `fetched_at > now - 30 days` → use cached data
- On cache miss: call OMDB, store result, display
- If no API key configured: show existing data only + nudge "Add OMDB key in Settings for ratings & plot"
- Query: `?t={title}&type={movie|series}&apikey={key}`
- Movie title: `channel.name`; Series title: `channel.seriesTitle`

### Tauri Commands

- `get_omdb_data(channel_id) → Option<OmdbData>` — returns cached data if fresh
- `fetch_omdb_data(channel_id, title, content_type) → OmdbData` — fetches, caches, returns
- `get_omdb_api_key() → Option<String>` — reads from Tauri Store
- `set_omdb_api_key(key) → ()` — writes to Tauri Store

### OmdbData Model

```rust
pub struct OmdbData {
    pub title: String,
    pub year: Option<String>,
    pub rated: Option<String>,
    pub runtime: Option<String>,
    pub genre: Option<String>,
    pub director: Option<String>,
    pub actors: Option<String>,
    pub plot: Option<String>,
    pub poster_url: Option<String>,
    pub imdb_rating: Option<String>,
    pub rotten_tomatoes: Option<String>,  // extracted from Ratings array
}
```

### Enhanced Drawer Display

```
┌── [Poster] ────────┬───────────────────────────────┐
│                    │  Movie Title  (2023)  PG-13    │
│  [poster image]    │  ★ 7.4  🍅 84%                │
│                    │  Action · Thriller · 2h 18m    │
│                    │  Dir: Christopher Nolan        │
│                    │  Cast: Cillian Murphy, ...     │
├────────────────────┴───────────────────────────────┤
│  Plot summary text goes here...                    │
│                         [Source ▾]  [▶ Play]       │
└────────────────────────────────────────────────────┘
```

- Poster: OMDB `Poster` URL if available, else `channel.logoUrl`
- Ratings shown as compact badges
- Fields with value `"N/A"` are hidden

---

## 3. Watch History

### DB Schema

```sql
CREATE TABLE IF NOT EXISTS watch_history (
    id                     INTEGER PRIMARY KEY AUTOINCREMENT,
    channel_id             TEXT NOT NULL UNIQUE,
    channel_name           TEXT NOT NULL,
    channel_logo           TEXT,
    content_type           TEXT NOT NULL,
    first_watched_at       INTEGER NOT NULL,
    last_watched_at        INTEGER NOT NULL,
    total_duration_seconds INTEGER NOT NULL DEFAULT 0,
    play_count             INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX IF NOT EXISTS idx_history_last_watched
    ON watch_history(last_watched_at DESC);
```

One row per channel. `play_count` increments and `total_duration_seconds` accumulates each session. Capped at configurable max (default 500); oldest `last_watched_at` rows pruned when exceeded.

### Tracking Lifecycle

- **On play start** (`VideoPlayer` → `mpv.load()`): call `record_play_start(channel_id, name, logo, content_type)` — upserts row, increments `play_count`, sets `last_watched_at`
- **On play end** (stop/navigate away): call `record_play_end(channel_id, duration_seconds)` — adds elapsed seconds to `total_duration_seconds`
- Duration measured by wall-clock time (works for live TV with no MPV position)

### Tauri Commands

- `record_play_start(channel_id, name, logo, content_type) → ()`
- `record_play_end(channel_id, duration_seconds) → ()`
- `get_watch_history(limit) → Vec<WatchHistoryEntry>`
- `delete_history_entry(channel_id) → ()`
- `clear_watch_history() → ()`

### History Tab UI

New tab in `ChannelList`: `Live | Movies | Series | Favorites | History`

```
┌─────────────────────────────────────────────────────┐
│  History                             [Clear All 🗑] │
│  ─────────────────────────────────────────────────  │
│  [Logo] BBC News              LIVE  Today 14:32  🗑 │
│         23 plays · 4h 12m total                     │
│  [Logo] Oppenheimer           MOVIE Yesterday    🗑 │
│         1 play · 2h 50m total                       │
│  [Logo] Suits LA              SERIES Mar 7       🗑 │
│         8 plays · 6h 30m total                      │
└─────────────────────────────────────────────────────┘
```

- Sorted by `last_watched_at` DESC
- Each row: logo, name, content type badge, relative timestamp, play count + total time
- Trash icon per row for individual deletion
- "Clear All" with confirmation dialog
- Clicking a row navigates to player (series → modal, movie → drawer, live → player)
- No count badge in the tab header

---

## 4. Favorites

### Existing

`is_favorite` column on channels, `toggle_favorite` Tauri command, `isFavorite` in `Channel` type — all already implemented.

### New

**Heart icon on ChannelCard:**
- Both `row` and `poster` variants
- `poster`: overlaid top-right corner
- `row`: rightmost action
- Filled `HeartFilled` when `isFavorite`, outline `Heart` when not
- Optimistic update on click

**Per-tab favorites filter:**
- Small heart toggle button in the tab bar (right of the search bar)
- When active: filters current tab to `isFavorite === true` only
- Resets on tab switch
- Empty state: "No favorites yet — tap ♡ on any channel"

**Favorites tab:**
- Shows all favorited channels across all content types
- Grouped by `contentType` with section headers
- Same card rendering as respective tab (row for live, poster for movies/series)
- Count badge shows total favorites

**Tab order:** `Live | Movies | Series | Favorites | History`

---

## 5. Settings

### ProviderSettingsModal (per-provider) — EPG section

```
EPG Schedule
  EPG URL:        [________________________] [Auto-detect]
  Auto-refresh:   [toggle]
  Refresh every:  [24 ▾] hours
```

### Main Settings Page — Integrations section

```
OMDB API
  API Key:  [••••••••••••••••] [Show] [Save] [Test]
  Status:   ✓ Valid key · 1000 calls/day limit
  Get a free key at omdbapi.com
```

### Main Settings Page — History section

```
Watch History
  Keep history for:  [Forever ▾]  (30 days / 90 days / 1 year / Forever)
  Max entries:       [500 ▾]      (100 / 500 / Unlimited)
  [Clear All History…]
```

### Settings Page Layout

New top-level sections: `General | Integrations | History | About`

---

## Dependencies

- `tauri-plugin-store` — for OMDB API key storage (add to `Cargo.toml` and `tauri.conf.json`)
- `quick_xml` — already used in `epg.rs`
- `reqwest` — already used in `xtream.rs`
- `chrono` — already in workspace
