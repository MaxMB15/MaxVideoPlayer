# Design: Content Deduplication & Bottom Drawer UI

**Date:** 2026-03-09
**Status:** Approved

## Problem

1. Movies with the same title from multiple providers appear as separate cards instead of one merged entry.
2. Series episodes with the same season/episode number from multiple providers appear as duplicates inside the series modal.
3. The series detail modal is a centered dialog — user wants a modern bottom drawer.
4. Movie and series tab counts reflect raw (undeduped) numbers.

## Approach

Single unified bottom drawer (`ContentDrawer` pattern) for both movies and series. All source selection flows through the drawer. Step navigation happens as state transitions within the drawer (no separate mount/unmount).

## Data Layer

### Movie deduplication
`useMemo` in `ChannelList` makes a single O(n) pass over `byType.movie` using a `Map<title, Channel>`. First occurrence is the primary entry. Each subsequent duplicate with the same exact title has its `url` pushed into the primary entry's `sources[]`. Logo is taken from the first entry that has one. Result: `movieTitles` — a flat deduplicated array used everywhere movies are displayed.

### Series (show level)
Already deduplicated by `seriesTitle` (no change).

### Series (episode level)
Inside `SeriesDetailModal`, before grouping into seasons, episodes are deduplicated by `${season}x${episode}` key using the same Map approach. Duplicate episodes merge their URLs into `sources[]`. This ensures each episode appears once regardless of how many providers carry it.

### Count fixes
- Movie tab badge → `movieTitles.length`
- Series tab badge → `seriesShows.length` (already correct)
- Filtered result count → `filtered.length` (naturally reflects deduplicated list)

## Component Architecture

| Component | Change |
|-----------|--------|
| `ChannelList.tsx` | Add `movieTitles` memo; add `selectedMovie` state; inline movie source drawer JSX; fix tab counts; update `handlePlay` routing |
| `ChannelCard.tsx` | `PosterCard` drops internal source picker; always calls `onPlay(channel)` |
| `SeriesDetailModal.tsx` | Full redesign as bottom drawer with step navigation |

No new files needed. No backend changes.

## Drawer UI/UX

### Shell (shared)
- Fixed to bottom: `fixed bottom-0 left-0 right-0`
- Slides up with 300ms ease-out; backdrop fades in simultaneously
- Drag handle pill at top center
- `max-h-[80vh]`, `rounded-t-2xl`
- Dismiss: click backdrop or `×` button — animates back down before unmount

### Movie drawer (1 step)
- Header: movie title + "Choose a source to play"
- Source rows: play icon, "Source 1 (default)", "Source 2", …
- Tapping any source plays immediately and closes drawer
- **If movie has only 1 source: skip drawer entirely, play immediately**

### Series drawer — Step 1 (episodes)
- Header: show title + `×`
- Season pills (horizontal scroll, only shown if >1 season)
- Episode list: number badge | episode title | `N src` pill (if multiple sources) | `›` chevron
- Single-source episode → play immediately + close
- Multi-source episode → advance to Step 2

### Series drawer — Step 2 (sources)
- Header: `← Back` + `×`
- Episode name as subtitle
- Same source row layout as movie drawer
- Back returns to Step 1 (state-only, no animation)

## Performance Notes

- O(n) Map-based deduplication; runs in <5ms for 50k channels
- `useMemo` ensures dedup only re-runs on channel data changes, not on filters/search/renders
- Virtual list (`@tanstack/react-virtual`) already in place — only visible cards render
- No deep object copies; sources are pushed in-place on Map entries
