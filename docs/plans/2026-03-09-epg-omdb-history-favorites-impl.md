# EPG, OMDB, Watch History & Favorites — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add EPG schedule display for live TV, OMDB metadata for movies/series, persistent watch history with stats, and favorites with heart icons and a dedicated tab.

**Architecture:** All persistent data in SQLite via `mvp-core`'s `CacheStore`. OMDB API key stored via `tauri-plugin-store` (OS-level secure file). EPG refresh follows the same `localStorage`-per-provider pattern as playlist refresh. Frontend components lazy-fetch OMDB only on drawer open; never in the background.

**Tech Stack:** Rust (`rusqlite`, `reqwest`, `quick-xml`, `chrono`, `tauri-plugin-store`), React/TypeScript, Tailwind CSS, Tauri v2 commands.

---

## Codebase Orientation

Before starting, read these files to understand existing patterns:
- `crates/core/src/cache/store.rs` — DB schema, init_tables pattern (ALTER TABLE migrations), store methods
- `crates/core/src/iptv/epg.rs` — XMLTV parser, `EpgProgram` struct (currently uses String timestamps — we change to i64)
- `apps/desktop/src-tauri/src/commands.rs` — command pattern, `AppState`
- `apps/desktop/src/hooks/useChannels.ts` — `loadProviderSettings`/`saveProviderSettings` pattern
- `apps/desktop/src/components/playlist/ProviderSettingsModal.tsx` — existing settings modal UI pattern
- `apps/desktop/src/components/channels/ChannelCard.tsx` — card variants (row/poster)
- `apps/desktop/src/components/channels/ChannelList.tsx` — tab structure, virtualizer

---

## Task 1: EPG — Provider model + DB schema

**Files:**
- Modify: `crates/core/src/models/playlist.rs`
- Modify: `crates/core/src/cache/store.rs`

### Step 1: Add `epg_url` to `Provider` model

In `crates/core/src/models/playlist.rs`, add the `epg_url` field:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: ProviderType,
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub last_updated: Option<String>,
    pub channel_count: usize,
    pub epg_url: Option<String>,  // NEW
}
```

### Step 2: Write failing tests for EPG schema

In `crates/core/src/cache/store.rs`, in the `#[cfg(test)]` block, add:

```rust
#[test]
fn test_epg_programmes_stored_and_retrieved() {
    let store = CacheStore::open_in_memory().unwrap();
    let prog = StoredEpgProgram {
        channel_id: "ch1".into(),
        title: "Morning News".into(),
        description: Some("Daily news".into()),
        start_time: 1700000000,
        end_time: 1700003600,
        category: Some("News".into()),
        provider_id: "p1".into(),
        fetched_at: 1700000000,
    };
    store.save_epg_programmes("p1", &[prog.clone()]).unwrap();
    let result = store.get_epg_programmes("ch1", 1699999000, 1700010000).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].title, "Morning News");
}

#[test]
fn test_epg_programmes_cleared_on_refresh() {
    let store = CacheStore::open_in_memory().unwrap();
    let prog = StoredEpgProgram {
        channel_id: "ch1".into(),
        title: "Old Show".into(),
        description: None,
        start_time: 1700000000,
        end_time: 1700003600,
        category: None,
        provider_id: "p1".into(),
        fetched_at: 1699000000,
    };
    store.save_epg_programmes("p1", &[prog]).unwrap();
    // Saving again should replace
    let new_prog = StoredEpgProgram {
        channel_id: "ch1".into(),
        title: "New Show".into(),
        description: None,
        start_time: 1700000000,
        end_time: 1700003600,
        category: None,
        provider_id: "p1".into(),
        fetched_at: 1700000001,
    };
    store.save_epg_programmes("p1", &[new_prog]).unwrap();
    let result = store.get_epg_programmes("ch1", 1699999000, 1700010000).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].title, "New Show");
}

#[test]
fn test_provider_epg_url_saved_and_retrieved() {
    let store = CacheStore::open_in_memory().unwrap();
    let mut provider = make_test_provider("p1");
    provider.epg_url = Some("http://example.com/epg.xml".into());
    store.upsert_provider(&provider).unwrap();
    let providers = store.get_providers().unwrap();
    assert_eq!(providers[0].epg_url.as_deref(), Some("http://example.com/epg.xml"));
}
```

Run: `cargo test -p mvp-core -- test_epg_programmes` (expect FAIL — no `StoredEpgProgram` yet)

### Step 3: Add `StoredEpgProgram` struct and store methods

Add `StoredEpgProgram` near the top of `store.rs` (after imports):

```rust
#[derive(Debug, Clone)]
pub struct StoredEpgProgram {
    pub channel_id: String,
    pub title: String,
    pub description: Option<String>,
    pub start_time: i64,   // Unix timestamp seconds
    pub end_time: i64,
    pub category: Option<String>,
    pub provider_id: String,
    pub fetched_at: i64,
}
```

In `init_tables`, add after the existing `epg_cache` table definition:

```rust
self.conn.execute_batch(
    "CREATE TABLE IF NOT EXISTS epg_programmes (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        channel_id  TEXT NOT NULL,
        title       TEXT NOT NULL,
        description TEXT,
        start_time  INTEGER NOT NULL,
        end_time    INTEGER NOT NULL,
        category    TEXT,
        provider_id TEXT NOT NULL,
        fetched_at  INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_epg_channel_time
        ON epg_programmes(channel_id, start_time);"
)?;
```

Add migration for `epg_url` on providers table (after existing ALTER TABLE migrations):

```rust
let _ = self.conn.execute_batch(
    "ALTER TABLE providers ADD COLUMN epg_url TEXT;"
);
```

Add store methods:

```rust
pub fn save_epg_programmes(
    &self,
    provider_id: &str,
    programmes: &[StoredEpgProgram],
) -> Result<(), CacheError> {
    // Delete old data for this provider first
    self.conn.execute(
        "DELETE FROM epg_programmes WHERE provider_id = ?1",
        params![provider_id],
    )?;
    for prog in programmes {
        self.conn.execute(
            "INSERT INTO epg_programmes
             (channel_id, title, description, start_time, end_time, category, provider_id, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                prog.channel_id,
                prog.title,
                prog.description,
                prog.start_time,
                prog.end_time,
                prog.category,
                prog.provider_id,
                prog.fetched_at,
            ],
        )?;
    }
    Ok(())
}

pub fn get_epg_programmes(
    &self,
    channel_id: &str,
    range_start: i64,
    range_end: i64,
) -> Result<Vec<StoredEpgProgram>, CacheError> {
    let mut stmt = self.conn.prepare(
        "SELECT channel_id, title, description, start_time, end_time, category, provider_id, fetched_at
         FROM epg_programmes
         WHERE channel_id = ?1 AND start_time < ?3 AND end_time > ?2
         ORDER BY start_time ASC",
    )?;
    let rows = stmt.query_map(params![channel_id, range_start, range_end], |row| {
        Ok(StoredEpgProgram {
            channel_id: row.get(0)?,
            title: row.get(1)?,
            description: row.get(2)?,
            start_time: row.get(3)?,
            end_time: row.get(4)?,
            category: row.get(5)?,
            provider_id: row.get(6)?,
            fetched_at: row.get(7)?,
        })
    })?;
    rows.collect::<SqlResult<Vec<_>>>().map_err(CacheError::Db)
}

pub fn set_provider_epg_url(
    &self,
    provider_id: &str,
    epg_url: Option<&str>,
) -> Result<(), CacheError> {
    self.conn.execute(
        "UPDATE providers SET epg_url = ?1 WHERE id = ?2",
        params![epg_url, provider_id],
    )?;
    Ok(())
}
```

Also update `upsert_provider` to include `epg_url` in INSERT/UPDATE and update `get_providers` to read `epg_url`. Find `upsert_provider` and update the SQL:

```rust
pub fn upsert_provider(&self, provider: &Provider) -> Result<(), CacheError> {
    self.conn.execute(
        "INSERT INTO providers (id, name, provider_type, url, username, password, last_updated, channel_count, epg_url)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET
           name=excluded.name, provider_type=excluded.provider_type,
           url=excluded.url, username=excluded.username, password=excluded.password,
           last_updated=excluded.last_updated, channel_count=excluded.channel_count,
           epg_url=excluded.epg_url",
        params![
            provider.id, provider.name,
            match provider.provider_type { ProviderType::M3u => "m3u", ProviderType::Xtream => "xtream" },
            provider.url, provider.username, provider.password,
            provider.last_updated, provider.channel_count as i64,
            provider.epg_url,
        ],
    )?;
    Ok(())
}
```

Update `get_providers` SELECT to include `epg_url` (column index 8):

```rust
// In the row closure, add:
epg_url: row.get(8)?,
```

### Step 4: Run tests

```bash
cargo test -p mvp-core -- test_epg_programmes test_provider_epg_url
```
Expected: all pass.

### Step 5: Commit

```bash
git add crates/core/src/models/playlist.rs crates/core/src/cache/store.rs
git commit -m "feat(epg): add epg_programmes table and epg_url to provider model"
```

---

## Task 2: EPG — Parser update to Unix timestamps

**Files:**
- Modify: `crates/core/src/iptv/epg.rs`

### Step 1: Write failing tests for Unix timestamp parsing

In the `#[cfg(test)]` block in `epg.rs`, add:

```rust
#[test]
fn test_parse_epg_unix_timestamps() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tv>
  <channel id="ch1"><display-name>Channel 1</display-name></channel>
  <programme start="20260304120000 +0000" stop="20260304130000 +0000" channel="ch1">
    <title>Noon News</title>
  </programme>
</tv>"#;
    let data = parse_epg(xml).unwrap();
    // 2026-03-04T12:00:00 UTC = 1772308800
    assert_eq!(data.programs[0].start_time, 1772308800);
    // 2026-03-04T13:00:00 UTC = 1772312400
    assert_eq!(data.programs[0].end_time, 1772312400);
}

#[test]
fn test_epg_to_stored_programmes() {
    let xml = r#"<?xml version="1.0"?>
<tv>
  <programme start="20260304120000 +0000" stop="20260304130000 +0000" channel="ch1">
    <title>Test</title>
  </programme>
</tv>"#;
    let data = parse_epg(xml).unwrap();
    let stored = epg_data_to_stored(&data, "p1");
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].provider_id, "p1");
    assert_eq!(stored[0].channel_id, "ch1");
}
```

Run: `cargo test -p mvp-core -- test_parse_epg_unix` (expect FAIL)

### Step 2: Update `EpgProgram` and parser

Replace the `EpgProgram` struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpgProgram {
    pub channel_id: String,
    pub title: String,
    pub description: String,
    pub start_time: i64,   // Unix timestamp (seconds UTC)
    pub end_time: i64,
    pub category: Option<String>,
}
```

Replace `normalize_epg_time` with:

```rust
/// Parse XMLTV timestamp like "20260304120000 +0000" to Unix timestamp (UTC seconds).
fn parse_epg_time(raw: &str) -> i64 {
    use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
    let parts: Vec<&str> = raw.splitn(2, ' ').collect();
    let dt_str = parts[0];
    let tz_offset_str = parts.get(1).copied().unwrap_or("+0000");

    if dt_str.len() < 14 {
        return 0;
    }
    let Ok(naive) = NaiveDateTime::parse_from_str(&dt_str[..14], "%Y%m%d%H%M%S") else {
        return 0;
    };

    // Parse timezone offset like "+0100" → seconds
    let tz_offset_secs: i64 = if tz_offset_str.len() == 5 {
        let sign: i64 = if tz_offset_str.starts_with('-') { -1 } else { 1 };
        let hh: i64 = tz_offset_str[1..3].parse().unwrap_or(0);
        let mm: i64 = tz_offset_str[3..5].parse().unwrap_or(0);
        sign * (hh * 3600 + mm * 60)
    } else {
        0
    };

    // naive is in local tz (as given by offset), convert to UTC
    naive.and_utc().timestamp() - tz_offset_secs
}
```

In the `parse_epg` function, update the `programme` Start arm to use `parse_epg_time`:

```rust
"programme" => {
    in_programme = true;
    let ch = extract_xml_attr(&e, "channel").unwrap_or_default();
    let start = extract_xml_attr(&e, "start").unwrap_or_default();
    let stop = extract_xml_attr(&e, "stop").unwrap_or_default();
    current_prog = EpgProgram {
        channel_id: ch,
        title: String::new(),
        description: String::new(),
        start_time: parse_epg_time(&start),
        end_time: parse_epg_time(&stop),
        category: None,
    };
}
```

Add `epg_data_to_stored` conversion function (after `fetch_and_parse_epg`):

```rust
use crate::cache::store::StoredEpgProgram;

/// Convert parsed EpgData into StoredEpgPrograms ready for DB insertion.
pub fn epg_data_to_stored(data: &EpgData, provider_id: &str) -> Vec<StoredEpgProgram> {
    let now = chrono::Utc::now().timestamp();
    data.programs
        .iter()
        .map(|p| StoredEpgProgram {
            channel_id: p.channel_id.clone(),
            title: p.title.clone(),
            description: if p.description.is_empty() { None } else { Some(p.description.clone()) },
            start_time: p.start_time,
            end_time: p.end_time,
            category: p.category.clone(),
            provider_id: provider_id.to_string(),
            fetched_at: now,
        })
        .collect()
}
```

Update existing tests that check `start_time` as string (they checked `"2026-03-04T12:00:00"`) — remove those assertions or update to check `i64` values.

### Step 3: Run tests

```bash
cargo test -p mvp-core -- epg
```
Expected: all pass.

### Step 4: Commit

```bash
git add crates/core/src/iptv/epg.rs
git commit -m "feat(epg): change EpgProgram timestamps to Unix i64, add epg_data_to_stored"
```

---

## Task 3: EPG — M3U header EPG URL extraction

**Files:**
- Modify: `crates/core/src/iptv/m3u.rs`
- Modify: `apps/desktop/src-tauri/src/commands.rs`

### Step 1: Write failing test for EPG URL extraction

In `m3u.rs` tests:

```rust
#[test]
fn test_extract_epg_url_from_header() {
    let content = "#EXTM3U x-tvg-url=\"http://example.com/epg.xml\"\n#EXTINF:-1,Channel\nhttp://url.com\n";
    let url = extract_epg_url(content);
    assert_eq!(url, Some("http://example.com/epg.xml".to_string()));
}

#[test]
fn test_extract_epg_url_url_tvg_attr() {
    // Some providers use "url-tvg" instead of "x-tvg-url"
    let content = "#EXTM3U url-tvg=\"http://alt.com/guide.xml\"\n#EXTINF:-1,Channel\nhttp://url.com\n";
    let url = extract_epg_url(content);
    assert_eq!(url, Some("http://alt.com/guide.xml".to_string()));
}

#[test]
fn test_extract_epg_url_missing_returns_none() {
    let content = "#EXTM3U\n#EXTINF:-1,Channel\nhttp://url.com\n";
    let url = extract_epg_url(content);
    assert_eq!(url, None);
}
```

Run: `cargo test -p mvp-core -- test_extract_epg_url` (expect FAIL)

### Step 2: Implement `extract_epg_url`

Add to `m3u.rs` (after the `extract_attr` helper):

```rust
/// Extract the EPG URL from the #EXTM3U header line.
/// Looks for `x-tvg-url="..."` or `url-tvg="..."` attributes.
pub fn extract_epg_url(content: &str) -> Option<String> {
    let header_line = content
        .lines()
        .find(|l| l.trim_start_matches('\u{feff}').starts_with("#EXTM3U"))?;
    extract_attr(header_line, "x-tvg-url")
        .or_else(|| extract_attr(header_line, "url-tvg"))
        .filter(|s| !s.is_empty())
}
```

### Step 3: Update `fetch_and_parse_m3u` to return EPG URL alongside channels

Add a new return type:

```rust
#[derive(Debug)]
pub struct M3uPlaylist {
    pub channels: Vec<Channel>,
    pub epg_url: Option<String>,
}
```

Add `pub async fn fetch_and_parse_m3u_with_epg(url: &str) -> Result<M3uPlaylist, M3uError>`:

```rust
pub async fn fetch_and_parse_m3u_with_epg(url: &str) -> Result<M3uPlaylist, M3uError> {
    let response = reqwest::get(url).await?.bytes().await?;
    let content = String::from_utf8_lossy(&response);
    let epg_url = extract_epg_url(&content);
    let channels = parse_m3u(&content)?;
    Ok(M3uPlaylist { channels, epg_url })
}
```

> Keep existing `fetch_and_parse_m3u` unchanged so callers don't break yet.

### Step 4: Update `load_m3u_playlist` command to extract and save EPG URL

In `commands.rs`, replace the call to `fetch_and_parse_m3u` in `load_m3u_playlist` with `fetch_and_parse_m3u_with_epg`:

```rust
use mvp_core::iptv::m3u::{fetch_and_parse_m3u_with_epg, parse_m3u_file};

// In load_m3u_playlist:
let playlist = fetch_and_parse_m3u_with_epg(&url)
    .await
    .map_err(|e| format!("Failed to load playlist: {e}"))?;
let mut channels = playlist.channels;
let epg_url = playlist.epg_url;

// ... existing prefix + provider creation ...

let mut provider = Provider {
    // ... existing fields ...
    epg_url,
};
```

Also update `refresh_provider` command similarly (it also calls `fetch_and_parse_m3u`).

### Step 5: Run tests

```bash
cargo test -p mvp-core -- test_extract_epg
cargo check -p max-video-player
```
Expected: pass.

### Step 6: Commit

```bash
git add crates/core/src/iptv/m3u.rs apps/desktop/src-tauri/src/commands.rs
git commit -m "feat(epg): extract EPG URL from M3U header, save to provider on load"
```

---

## Task 4: EPG — Xtream EPG URL + Tauri commands for EPG

**Files:**
- Modify: `crates/core/src/iptv/xtream.rs`
- Modify: `apps/desktop/src-tauri/src/commands.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src/lib/tauri.ts`
- Modify: `apps/desktop/src/lib/types.ts`

### Step 1: Add Xtream EPG URL helper

In `xtream.rs`, add:

```rust
/// Returns the XMLTV EPG URL for an Xtream provider.
pub fn get_xtream_epg_url(server: &str, username: &str, password: &str) -> String {
    format!("{}/xmltv.php?username={}&password={}", server.trim_end_matches('/'), username, password)
}
```

Update `load_xtream_provider` in `commands.rs` to save the EPG URL when creating the provider:

```rust
// After building the XtreamCredentials:
let epg_url = Some(get_xtream_epg_url(&url, &username, &password));
let provider = Provider {
    // ... existing fields ...
    epg_url,
};
```

Import: `use mvp_core::iptv::xtream::{fetch_xtream_channels, fetch_xtream_series_episodes, get_xtream_epg_url};`

### Step 2: Add EPG Tauri commands

In `commands.rs`, add:

```rust
/// Fetch and store EPG programmes for a provider.
#[command]
pub async fn refresh_epg(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<(), String> {
    use mvp_core::iptv::epg::{fetch_and_parse_epg, epg_data_to_stored};

    let epg_url = {
        let cache = state.cache.lock().map_err(|e| e.to_string())?;
        let providers = cache.get_providers().map_err(|e| e.to_string())?;
        providers
            .into_iter()
            .find(|p| p.id == provider_id)
            .and_then(|p| p.epg_url)
    };

    let Some(url) = epg_url else {
        return Err("No EPG URL configured for this provider".into());
    };

    tracing::info!("[EPG] fetching EPG from {}", url);
    let epg_data = fetch_and_parse_epg(&url)
        .await
        .map_err(|e| format!("Failed to fetch EPG: {e}"))?;

    tracing::info!("[EPG] parsed {} programmes", epg_data.programs.len());
    let stored = epg_data_to_stored(&epg_data, &provider_id);

    let cache = state.cache.lock().map_err(|e| e.to_string())?;
    cache.save_epg_programmes(&provider_id, &stored)
        .map_err(|e| e.to_string())
}

/// Get EPG programmes for a channel within a time range.
#[command]
pub async fn get_epg_programmes(
    state: State<'_, AppState>,
    channel_id: String,
    range_start: i64,
    range_end: i64,
) -> Result<Vec<StoredEpgProgramDto>, String> {
    let cache = state.cache.lock().map_err(|e| e.to_string())?;
    let progs = cache
        .get_epg_programmes(&channel_id, range_start, range_end)
        .map_err(|e| e.to_string())?;
    Ok(progs
        .into_iter()
        .map(|p| StoredEpgProgramDto {
            channel_id: p.channel_id,
            title: p.title,
            description: p.description,
            start_time: p.start_time,
            end_time: p.end_time,
            category: p.category,
        })
        .collect())
}

/// Set EPG URL for a provider (manual override).
#[command]
pub async fn set_epg_url(
    state: State<'_, AppState>,
    provider_id: String,
    epg_url: Option<String>,
) -> Result<(), String> {
    let cache = state.cache.lock().map_err(|e| e.to_string())?;
    cache
        .set_provider_epg_url(&provider_id, epg_url.as_deref())
        .map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredEpgProgramDto {
    pub channel_id: String,
    pub title: String,
    pub description: Option<String>,
    pub start_time: i64,
    pub end_time: i64,
    pub category: Option<String>,
}
```

Register in `lib.rs` invoke_handler:

```rust
commands::refresh_epg,
commands::get_epg_programmes,
commands::set_epg_url,
```

### Step 3: Update `types.ts` with new EPG types

Add to `apps/desktop/src/lib/types.ts`:

```ts
export interface EpgProgram {
  channelId: string;
  title: string;
  description?: string;
  startTime: number;   // Unix timestamp seconds
  endTime: number;
  category?: string;
}

// Update Provider to include epgUrl
export interface Provider {
  id: string;
  name: string;
  type: "m3u" | "xtream";
  url: string;
  username?: string;
  password?: string;
  lastUpdated?: string;
  channelCount: number;
  epgUrl?: string;     // NEW
}
```

(Replace the existing `EpgProgram` interface that had string timestamps.)

### Step 4: Update `tauri.ts` with EPG commands

Add to `apps/desktop/src/lib/tauri.ts`:

```ts
export async function refreshEpg(providerId: string): Promise<void> {
  return invoke("refresh_epg", { providerId });
}

export async function getEpgProgrammes(
  channelId: string,
  rangeStart: number,
  rangeEnd: number
): Promise<EpgProgram[]> {
  return invoke("get_epg_programmes", { channelId, rangeStart, rangeEnd });
}

export async function setEpgUrl(
  providerId: string,
  epgUrl: string | null
): Promise<void> {
  return invoke("set_epg_url", { providerId, epgUrl });
}
```

### Step 5: Build check

```bash
cargo check -p max-video-player
cd apps/desktop && npx tsc --noEmit
```
Expected: no errors.

### Step 6: Commit

```bash
git add crates/core/src/iptv/xtream.rs apps/desktop/src-tauri/src/commands.rs \
  apps/desktop/src-tauri/src/lib.rs apps/desktop/src/lib/tauri.ts \
  apps/desktop/src/lib/types.ts
git commit -m "feat(epg): Xtream EPG URL, refresh_epg/get_epg_programmes/set_epg_url commands"
```

---

## Task 5: OMDB — DB + module + tauri-plugin-store + commands

**Files:**
- Create: `crates/core/src/iptv/omdb.rs`
- Modify: `crates/core/src/iptv/mod.rs` (add `pub mod omdb`)
- Modify: `crates/core/src/cache/store.rs`
- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Modify: `apps/desktop/src-tauri/capabilities/default.json`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src-tauri/src/commands.rs`
- Modify: `apps/desktop/src/lib/tauri.ts`
- Modify: `apps/desktop/src/lib/types.ts`

### Step 1: Add `tauri-plugin-store` dependency

In `apps/desktop/src-tauri/Cargo.toml`, add to `[dependencies]`:

```toml
tauri-plugin-store = "2"
```

In `apps/desktop/src-tauri/capabilities/default.json`, add to permissions array:

```json
"store:default"
```

In `apps/desktop/src-tauri/src/lib.rs`, add plugin init (after `tauri_plugin_dialog::init()`):

```rust
.plugin(tauri_plugin_store::Builder::new().build())
```

### Step 2: Write failing tests for OMDB cache

In `store.rs` tests:

```rust
#[test]
fn test_omdb_cache_saved_and_retrieved() {
    let store = CacheStore::open_in_memory().unwrap();
    let data = r#"{"title":"Oppenheimer","year":"2023"}"#;
    store.save_omdb_cache("ch1", data).unwrap();
    let result = store.get_omdb_cache("ch1").unwrap();
    assert!(result.is_some());
    assert!(result.unwrap().contains("Oppenheimer"));
}

#[test]
fn test_omdb_cache_stale_returns_none() {
    let store = CacheStore::open_in_memory().unwrap();
    // Insert with very old fetched_at (31 days ago)
    let old_ts = chrono::Utc::now().timestamp() - (31 * 24 * 3600);
    store.conn.execute(
        "INSERT INTO omdb_cache (channel_id, data_json, fetched_at) VALUES ('ch1', '{}', ?1)",
        rusqlite::params![old_ts],
    ).unwrap();
    let result = store.get_omdb_cache("ch1").unwrap();
    assert!(result.is_none(), "stale cache should return None");
}
```

Note: `store.conn` is private. Make it `pub(crate)` temporarily for the test, or expose a `insert_omdb_cache_with_timestamp` test helper. Simplest: make `conn` `pub(crate)` inside the `CacheStore` struct.

Change `pub struct CacheStore { conn: Connection }` to `pub struct CacheStore { pub(crate) conn: Connection }`.

Run: `cargo test -p mvp-core -- test_omdb_cache` (expect FAIL)

### Step 3: Add `omdb_cache` table and methods to `store.rs`

In `init_tables`, add:

```rust
self.conn.execute_batch(
    "CREATE TABLE IF NOT EXISTS omdb_cache (
        channel_id   TEXT PRIMARY KEY,
        data_json    TEXT NOT NULL,
        fetched_at   INTEGER NOT NULL
    );"
)?;
```

Add methods:

```rust
/// Returns cached OMDB JSON if it exists and is < 30 days old.
pub fn get_omdb_cache(&self, channel_id: &str) -> Result<Option<String>, CacheError> {
    let cutoff = chrono::Utc::now().timestamp() - (30 * 24 * 3600);
    let result = self.conn.query_row(
        "SELECT data_json FROM omdb_cache WHERE channel_id = ?1 AND fetched_at > ?2",
        params![channel_id, cutoff],
        |row| row.get::<_, String>(0),
    );
    match result {
        Ok(json) => Ok(Some(json)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(CacheError::Db(e)),
    }
}

pub fn save_omdb_cache(&self, channel_id: &str, data_json: &str) -> Result<(), CacheError> {
    let now = chrono::Utc::now().timestamp();
    self.conn.execute(
        "INSERT OR REPLACE INTO omdb_cache (channel_id, data_json, fetched_at) VALUES (?1, ?2, ?3)",
        params![channel_id, data_json, now],
    )?;
    Ok(())
}
```

### Step 4: Create `crates/core/src/iptv/omdb.rs`

```rust
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OmdbError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("OMDB error: {0}")]
    Api(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    pub rotten_tomatoes: Option<String>,
}

#[derive(Deserialize)]
struct OmdbResponse {
    #[serde(rename = "Response")]
    response: String,
    #[serde(rename = "Error")]
    error: Option<String>,
    #[serde(rename = "Title")]
    title: Option<String>,
    #[serde(rename = "Year")]
    year: Option<String>,
    #[serde(rename = "Rated")]
    rated: Option<String>,
    #[serde(rename = "Runtime")]
    runtime: Option<String>,
    #[serde(rename = "Genre")]
    genre: Option<String>,
    #[serde(rename = "Director")]
    director: Option<String>,
    #[serde(rename = "Actors")]
    actors: Option<String>,
    #[serde(rename = "Plot")]
    plot: Option<String>,
    #[serde(rename = "Poster")]
    poster: Option<String>,
    #[serde(rename = "Ratings")]
    ratings: Option<Vec<OmdbRating>>,
    #[serde(rename = "imdbRating")]
    imdb_rating: Option<String>,
}

#[derive(Deserialize)]
struct OmdbRating {
    #[serde(rename = "Source")]
    source: String,
    #[serde(rename = "Value")]
    value: String,
}

fn na_to_none(s: Option<String>) -> Option<String> {
    s.filter(|v| v != "N/A" && !v.is_empty())
}

/// Fetch metadata from OMDB for a title.
/// content_type should be "movie" or "series".
pub async fn fetch_omdb(
    title: &str,
    content_type: &str,
    api_key: &str,
) -> Result<OmdbData, OmdbError> {
    let url = format!(
        "https://www.omdbapi.com/?t={}&type={}&apikey={}",
        urlencoding::encode(title),
        content_type,
        api_key
    );
    let resp: OmdbResponse = reqwest::get(&url).await?.json().await?;
    if resp.response != "True" {
        return Err(OmdbError::Api(resp.error.unwrap_or_else(|| "Unknown error".into())));
    }
    let rotten = resp.ratings.as_ref().and_then(|ratings| {
        ratings
            .iter()
            .find(|r| r.source == "Rotten Tomatoes")
            .map(|r| r.value.clone())
    });
    Ok(OmdbData {
        title: resp.title.unwrap_or_default(),
        year: na_to_none(resp.year),
        rated: na_to_none(resp.rated),
        runtime: na_to_none(resp.runtime),
        genre: na_to_none(resp.genre),
        director: na_to_none(resp.director),
        actors: na_to_none(resp.actors),
        plot: na_to_none(resp.plot),
        poster_url: na_to_none(resp.poster),
        imdb_rating: na_to_none(resp.imdb_rating),
        rotten_tomatoes: rotten,
    })
}
```

Add `urlencoding = "2"` to `crates/core/Cargo.toml` dependencies.

Add `pub mod omdb;` to `crates/core/src/iptv/mod.rs`.

### Step 5: Add OMDB Tauri commands

In `commands.rs`:

```rust
use mvp_core::iptv::omdb::{fetch_omdb, OmdbData};
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "settings.json";
const OMDB_KEY_STORE_KEY: &str = "omdbApiKey";

#[command]
pub async fn get_omdb_api_key(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    Ok(store.get(OMDB_KEY_STORE_KEY).and_then(|v| v.as_str().map(|s| s.to_string())))
}

#[command]
pub async fn set_omdb_api_key(
    app: tauri::AppHandle,
    api_key: String,
) -> Result<(), String> {
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    store.set(OMDB_KEY_STORE_KEY, serde_json::Value::String(api_key));
    store.save().map_err(|e| e.to_string())
}

/// Fetch OMDB data for a channel (checks cache first).
#[command]
pub async fn fetch_omdb_data(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    channel_id: String,
    title: String,
    content_type: String,
) -> Result<Option<OmdbData>, String> {
    // Check cache first
    {
        let cache = state.cache.lock().map_err(|e| e.to_string())?;
        if let Ok(Some(json)) = cache.get_omdb_cache(&channel_id) {
            if let Ok(data) = serde_json::from_str::<OmdbData>(&json) {
                return Ok(Some(data));
            }
        }
    }

    // Get API key
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    let api_key = store
        .get(OMDB_KEY_STORE_KEY)
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    let Some(key) = api_key else {
        return Ok(None); // No key configured — caller shows nudge
    };

    let ct = match content_type.as_str() {
        "movie" => "movie",
        "series" => "series",
        _ => "movie",
    };

    match fetch_omdb(&title, ct, &key).await {
        Ok(data) => {
            // Cache the result
            if let Ok(json) = serde_json::to_string(&data) {
                let cache = state.cache.lock().map_err(|e| e.to_string())?;
                let _ = cache.save_omdb_cache(&channel_id, &json);
            }
            Ok(Some(data))
        }
        Err(e) => {
            tracing::warn!("[OMDB] fetch failed for {:?}: {}", title, e);
            Ok(None) // Non-fatal — UI shows without enrichment
        }
    }
}
```

Register in `lib.rs`:

```rust
commands::get_omdb_api_key,
commands::set_omdb_api_key,
commands::fetch_omdb_data,
```

### Step 6: Update `tauri.ts` and `types.ts`

In `types.ts`, add:

```ts
export interface OmdbData {
  title: string;
  year?: string;
  rated?: string;
  runtime?: string;
  genre?: string;
  director?: string;
  actors?: string;
  plot?: string;
  posterUrl?: string;
  imdbRating?: string;
  rottenTomatoes?: string;
}
```

In `tauri.ts`, add:

```ts
import type { ..., OmdbData } from "./types";

export async function getOmdbApiKey(): Promise<string | null> {
  return invoke("get_omdb_api_key");
}

export async function setOmdbApiKey(apiKey: string): Promise<void> {
  return invoke("set_omdb_api_key", { apiKey });
}

export async function fetchOmdbData(
  channelId: string,
  title: string,
  contentType: string
): Promise<OmdbData | null> {
  return invoke("fetch_omdb_data", { channelId, title, contentType });
}
```

### Step 7: Build check

```bash
cargo check -p max-video-player
cd apps/desktop && npx tsc --noEmit
```

### Step 8: Commit

```bash
git add crates/core/src/iptv/omdb.rs crates/core/src/iptv/mod.rs \
  crates/core/src/cache/store.rs crates/core/Cargo.toml \
  apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/capabilities/default.json \
  apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/src/commands.rs \
  apps/desktop/src/lib/tauri.ts apps/desktop/src/lib/types.ts
git commit -m "feat(omdb): OMDB module, cache table, tauri-plugin-store, fetch/cache commands"
```

---

## Task 6: Watch History — DB + Tauri commands

**Files:**
- Modify: `crates/core/src/cache/store.rs`
- Modify: `apps/desktop/src-tauri/src/commands.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src/lib/tauri.ts`
- Modify: `apps/desktop/src/lib/types.ts`

### Step 1: Write failing tests

In `store.rs` tests:

```rust
#[test]
fn test_record_play_start_creates_entry() {
    let store = CacheStore::open_in_memory().unwrap();
    store.record_play_start("ch1", "BBC News", None, "live").unwrap();
    let history = store.get_watch_history(10).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].channel_id, "ch1");
    assert_eq!(history[0].play_count, 1);
}

#[test]
fn test_record_play_end_accumulates_duration() {
    let store = CacheStore::open_in_memory().unwrap();
    store.record_play_start("ch1", "BBC News", None, "live").unwrap();
    store.record_play_end("ch1", 120).unwrap();
    let history = store.get_watch_history(10).unwrap();
    assert_eq!(history[0].total_duration_seconds, 120);
}

#[test]
fn test_play_count_increments_on_second_session() {
    let store = CacheStore::open_in_memory().unwrap();
    store.record_play_start("ch1", "BBC News", None, "live").unwrap();
    store.record_play_end("ch1", 60).unwrap();
    store.record_play_start("ch1", "BBC News", None, "live").unwrap();
    let history = store.get_watch_history(10).unwrap();
    assert_eq!(history[0].play_count, 2);
}

#[test]
fn test_delete_history_entry() {
    let store = CacheStore::open_in_memory().unwrap();
    store.record_play_start("ch1", "BBC News", None, "live").unwrap();
    store.delete_history_entry("ch1").unwrap();
    let history = store.get_watch_history(10).unwrap();
    assert!(history.is_empty());
}

#[test]
fn test_clear_watch_history() {
    let store = CacheStore::open_in_memory().unwrap();
    store.record_play_start("ch1", "Channel 1", None, "live").unwrap();
    store.record_play_start("ch2", "Channel 2", None, "movie").unwrap();
    store.clear_watch_history().unwrap();
    let history = store.get_watch_history(100).unwrap();
    assert!(history.is_empty());
}
```

Run: `cargo test -p mvp-core -- test_record_play test_delete_history test_clear_watch` (expect FAIL)

### Step 2: Add `watch_history` table and `WatchHistoryEntry` struct

In `init_tables`, add:

```rust
self.conn.execute_batch(
    "CREATE TABLE IF NOT EXISTS watch_history (
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
        ON watch_history(last_watched_at DESC);"
)?;
```

Add struct near top of `store.rs`:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchHistoryEntry {
    pub channel_id: String,
    pub channel_name: String,
    pub channel_logo: Option<String>,
    pub content_type: String,
    pub first_watched_at: i64,
    pub last_watched_at: i64,
    pub total_duration_seconds: i64,
    pub play_count: i64,
}
```

Add methods:

```rust
pub fn record_play_start(
    &self,
    channel_id: &str,
    channel_name: &str,
    channel_logo: Option<&str>,
    content_type: &str,
) -> Result<(), CacheError> {
    let now = chrono::Utc::now().timestamp();
    self.conn.execute(
        "INSERT INTO watch_history
             (channel_id, channel_name, channel_logo, content_type, first_watched_at, last_watched_at, total_duration_seconds, play_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5, 0, 1)
         ON CONFLICT(channel_id) DO UPDATE SET
             play_count = play_count + 1,
             last_watched_at = ?5,
             channel_name = ?2,
             channel_logo = ?3",
        params![channel_id, channel_name, channel_logo, content_type, now],
    )?;
    Ok(())
}

pub fn record_play_end(
    &self,
    channel_id: &str,
    duration_seconds: i64,
) -> Result<(), CacheError> {
    self.conn.execute(
        "UPDATE watch_history SET total_duration_seconds = total_duration_seconds + ?1 WHERE channel_id = ?2",
        params![duration_seconds, channel_id],
    )?;
    Ok(())
}

pub fn get_watch_history(&self, limit: i64) -> Result<Vec<WatchHistoryEntry>, CacheError> {
    let mut stmt = self.conn.prepare(
        "SELECT channel_id, channel_name, channel_logo, content_type,
                first_watched_at, last_watched_at, total_duration_seconds, play_count
         FROM watch_history
         ORDER BY last_watched_at DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], |row| {
        Ok(WatchHistoryEntry {
            channel_id: row.get(0)?,
            channel_name: row.get(1)?,
            channel_logo: row.get(2)?,
            content_type: row.get(3)?,
            first_watched_at: row.get(4)?,
            last_watched_at: row.get(5)?,
            total_duration_seconds: row.get(6)?,
            play_count: row.get(7)?,
        })
    })?;
    rows.collect::<SqlResult<Vec<_>>>().map_err(CacheError::Db)
}

pub fn delete_history_entry(&self, channel_id: &str) -> Result<(), CacheError> {
    self.conn.execute(
        "DELETE FROM watch_history WHERE channel_id = ?1",
        params![channel_id],
    )?;
    Ok(())
}

pub fn clear_watch_history(&self) -> Result<(), CacheError> {
    self.conn.execute("DELETE FROM watch_history", [])?;
    Ok(())
}

pub fn prune_watch_history(&self, max_entries: i64) -> Result<(), CacheError> {
    self.conn.execute(
        "DELETE FROM watch_history WHERE channel_id NOT IN (
             SELECT channel_id FROM watch_history ORDER BY last_watched_at DESC LIMIT ?1
         )",
        params![max_entries],
    )?;
    Ok(())
}
```

### Step 3: Run tests

```bash
cargo test -p mvp-core -- test_record_play test_delete_history test_clear_watch test_play_count
```
Expected: all pass.

### Step 4: Add Tauri commands

In `commands.rs`, add:

```rust
use mvp_core::cache::store::WatchHistoryEntry;

#[command]
pub async fn record_play_start(
    state: State<'_, AppState>,
    channel_id: String,
    channel_name: String,
    channel_logo: Option<String>,
    content_type: String,
) -> Result<(), String> {
    let cache = state.cache.lock().map_err(|e| e.to_string())?;
    cache.record_play_start(&channel_id, &channel_name, channel_logo.as_deref(), &content_type)
        .map_err(|e| e.to_string())
}

#[command]
pub async fn record_play_end(
    state: State<'_, AppState>,
    channel_id: String,
    duration_seconds: i64,
) -> Result<(), String> {
    let cache = state.cache.lock().map_err(|e| e.to_string())?;
    cache.record_play_end(&channel_id, duration_seconds)
        .map_err(|e| e.to_string())
}

#[command]
pub async fn get_watch_history(
    state: State<'_, AppState>,
    limit: i64,
) -> Result<Vec<WatchHistoryEntry>, String> {
    let cache = state.cache.lock().map_err(|e| e.to_string())?;
    cache.get_watch_history(limit).map_err(|e| e.to_string())
}

#[command]
pub async fn delete_history_entry(
    state: State<'_, AppState>,
    channel_id: String,
) -> Result<(), String> {
    let cache = state.cache.lock().map_err(|e| e.to_string())?;
    cache.delete_history_entry(&channel_id).map_err(|e| e.to_string())
}

#[command]
pub async fn clear_watch_history(state: State<'_, AppState>) -> Result<(), String> {
    let cache = state.cache.lock().map_err(|e| e.to_string())?;
    cache.clear_watch_history().map_err(|e| e.to_string())
}
```

Register in `lib.rs`:

```rust
commands::record_play_start,
commands::record_play_end,
commands::get_watch_history,
commands::delete_history_entry,
commands::clear_watch_history,
```

### Step 5: Update `types.ts` and `tauri.ts`

In `types.ts`, add:

```ts
export interface WatchHistoryEntry {
  channelId: string;
  channelName: string;
  channelLogo?: string;
  contentType: "live" | "movie" | "series";
  firstWatchedAt: number;
  lastWatchedAt: number;
  totalDurationSeconds: number;
  playCount: number;
}
```

In `tauri.ts`, add:

```ts
import type { ..., WatchHistoryEntry } from "./types";

export async function recordPlayStart(
  channelId: string,
  channelName: string,
  channelLogo: string | null,
  contentType: string
): Promise<void> {
  return invoke("record_play_start", { channelId, channelName, channelLogo, contentType });
}

export async function recordPlayEnd(
  channelId: string,
  durationSeconds: number
): Promise<void> {
  return invoke("record_play_end", { channelId, durationSeconds });
}

export async function getWatchHistory(limit: number): Promise<WatchHistoryEntry[]> {
  return invoke("get_watch_history", { limit });
}

export async function deleteHistoryEntry(channelId: string): Promise<void> {
  return invoke("delete_history_entry", { channelId });
}

export async function clearWatchHistory(): Promise<void> {
  return invoke("clear_watch_history");
}
```

### Step 6: Build check + commit

```bash
cargo check -p max-video-player
cd apps/desktop && npx tsc --noEmit
git add crates/core/src/cache/store.rs apps/desktop/src-tauri/src/commands.rs \
  apps/desktop/src-tauri/src/lib.rs apps/desktop/src/lib/tauri.ts \
  apps/desktop/src/lib/types.ts
git commit -m "feat(history): watch_history table, store methods, Tauri commands"
```

---

## Task 7: Favorites — heart icon on ChannelCard + per-tab filter + Favorites tab

**Files:**
- Modify: `apps/desktop/src/components/channels/ChannelCard.tsx`
- Modify: `apps/desktop/src/components/channels/ChannelList.tsx`
- Modify: `apps/desktop/src/hooks/useChannels.ts`

### Step 1: Add heart icon to `ChannelCard`

In `ChannelCard.tsx`, update the component:

Add import: `import { Play, Tv2, Heart } from "lucide-react";`

Update props interface:

```tsx
interface ChannelCardProps {
  channel: Channel;
  onPlay: (channel: Channel) => void;
  onToggleFavorite?: (channel: Channel) => void;
  variant?: "row" | "poster";
}
```

In `RowCard`, add heart button as the last item before the LIVE badge:

```tsx
function RowCard({
  channel,
  onPlay,
  onToggleFavorite,
}: {
  channel: Channel;
  onPlay: (ch: Channel) => void;
  onToggleFavorite?: (ch: Channel) => void;
}) {
  return (
    <div className="group flex items-center gap-3 w-full px-2 py-2 rounded-lg hover:bg-accent transition-colors">
      <button
        onClick={() => onPlay(channel)}
        className="flex items-center gap-3 flex-1 min-w-0 text-left focus-visible:outline-none"
      >
        <div className="h-8 w-8 rounded bg-secondary flex items-center justify-center overflow-hidden shrink-0">
          {channel.logoUrl ? (
            <img src={channel.logoUrl} alt="" className="h-full w-full object-contain" loading="lazy" />
          ) : (
            <Tv2 className="h-3.5 w-3.5 text-muted-foreground" />
          )}
        </div>
        <div className="flex-1 min-w-0">
          <p className="text-sm leading-tight truncate">{channel.name}</p>
          {channel.groupTitle && (
            <p className="text-[11px] text-muted-foreground truncate mt-0.5">{channel.groupTitle}</p>
          )}
        </div>
        <span className="flex items-center gap-1 text-[10px] font-medium text-red-400 shrink-0">
          <span className="h-1.5 w-1.5 rounded-full bg-red-400 animate-pulse" />
          LIVE
        </span>
      </button>
      {onToggleFavorite && (
        <button
          onClick={(e) => { e.stopPropagation(); onToggleFavorite(channel); }}
          className="shrink-0 p-1 rounded text-muted-foreground hover:text-foreground transition-colors"
          aria-label={channel.isFavorite ? "Remove from favorites" : "Add to favorites"}
        >
          <Heart
            className={cn("h-3.5 w-3.5", channel.isFavorite ? "fill-red-400 text-red-400" : "")}
          />
        </button>
      )}
    </div>
  );
}
```

In `PosterCard`, add heart overlay on top-right:

```tsx
{onToggleFavorite && (
  <button
    onClick={(e) => { e.stopPropagation(); onToggleFavorite(channel); }}
    className="absolute top-1.5 left-1.5 z-10 p-1 rounded-full bg-black/50 text-white hover:bg-black/70 transition-colors"
    aria-label={channel.isFavorite ? "Remove from favorites" : "Add to favorites"}
  >
    <Heart
      className={cn("h-3 w-3", channel.isFavorite ? "fill-red-400 text-red-400" : "")}
    />
  </button>
)}
```

Update `ChannelCard` to pass `onToggleFavorite` down to both variants.

### Step 2: Add `toggleFavorite` to `useChannels`

Check if `useChannels.ts` already exposes a `toggleFavorite` method. If not, add:

```ts
const toggleFavoriteChannel = useCallback(async (channelId: string) => {
  const isFav = await toggleFavoriteApi(channelId);
  setChannels(prev =>
    prev.map(ch => ch.id === channelId ? { ...ch, isFavorite: isFav } : ch)
  );
}, []);
```

And expose it from the context value: `toggleFavorite: toggleFavoriteChannel`.

Update the context type to include `toggleFavorite: (channelId: string) => Promise<void>`.

### Step 3: Add Favorites tab + per-tab filter to `ChannelList`

In `ChannelList.tsx`:

Add `"favorites"` and `"history"` to the `Tab` type:

```ts
type Tab = "live" | "movie" | "series" | "favorites" | "history";
```

Add to `TABS` array:

```ts
{ id: "favorites", label: "Favorites", icon: Heart },
{ id: "history",   label: "History",   icon: Clock },
```

Import: `import { Tv2, MonitorPlay, Clapperboard, Heart, Clock } from "lucide-react";`

Add favorites filter state:

```ts
const [favoritesOnly, setFavoritesOnly] = useState(false);
```

Reset on tab change:

```ts
const handleTabChange = (tab: Tab) => {
  setActiveTab(tab);
  setSelectedCategory(null);
  setSearch("");
  setFavoritesOnly(false);
};
```

Add favorites memo:

```ts
const allFavorites = useMemo(
  () => channels.filter(ch => ch.isFavorite),
  [channels]
);
```

Update `activeChannels`:

```ts
const activeChannels = useMemo(() => {
  if (activeTab === "series") return seriesShows;
  if (activeTab === "movie") return movieTitles;
  if (activeTab === "favorites") return allFavorites;
  if (activeTab === "history") return []; // history handled separately
  return byType[activeTab];
}, [activeTab, seriesShows, movieTitles, allFavorites, byType]);
```

Update `filtered` to apply `favoritesOnly`:

```ts
const filtered = useMemo(() => {
  let result = activeChannels;
  if (selectedCategory && activeTab !== "series" && activeTab !== "favorites") {
    result = result.filter(ch => ch.groupTitle === selectedCategory);
  }
  if (favoritesOnly && activeTab !== "favorites") {
    result = result.filter(ch => ch.isFavorite);
  }
  if (search.trim()) {
    const lower = search.toLowerCase();
    result = result.filter(ch => ch.name.toLowerCase().includes(lower));
  }
  return result;
}, [activeChannels, selectedCategory, search, activeTab, favoritesOnly]);
```

Update tab count for `favorites`:

```ts
const count =
  id === "series" ? seriesShows.length :
  id === "movie"  ? movieTitles.length :
  id === "favorites" ? allFavorites.length :
  id === "history" ? 0 :
  byType[id].length;
```

Add heart filter toggle button in the tab bar (right of SearchBar, only for non-favorites/non-history tabs):

```tsx
{activeTab !== "favorites" && activeTab !== "history" && (
  <button
    onClick={() => setFavoritesOnly(v => !v)}
    className={cn(
      "p-1.5 rounded transition-colors",
      favoritesOnly ? "text-red-400" : "text-muted-foreground hover:text-foreground"
    )}
    aria-label="Show favorites only"
  >
    <Heart className={cn("h-3.5 w-3.5", favoritesOnly && "fill-red-400")} />
  </button>
)}
```

Pass `onToggleFavorite` to every `ChannelCard`:

```tsx
<ChannelCard
  key={ch.id}
  channel={ch}
  onPlay={handlePlay}
  onToggleFavorite={handleToggleFavorite}
  variant={...}
/>
```

Add `handleToggleFavorite`:

```ts
const { toggleFavorite } = useChannels();
const handleToggleFavorite = useCallback(
  (channel: Channel) => toggleFavorite(channel.id),
  [toggleFavorite]
);
```

Empty state for Favorites tab (when no favorites):

```tsx
if (activeTab === "favorites" && allFavorites.length === 0) {
  return (
    <div className="flex flex-col items-center justify-center h-40 gap-2 text-center px-6">
      <Heart className="h-8 w-8 text-muted-foreground/30" />
      <p className="text-sm text-muted-foreground">No favorites yet</p>
      <p className="text-xs text-muted-foreground/60">Tap ♡ on any channel to add it here</p>
    </div>
  );
}
```

### Step 4: Build check + commit

```bash
cd apps/desktop && npx tsc --noEmit
git add apps/desktop/src/components/channels/ChannelCard.tsx \
  apps/desktop/src/components/channels/ChannelList.tsx \
  apps/desktop/src/hooks/useChannels.ts
git commit -m "feat(favorites): heart icon on cards, per-tab filter, Favorites tab"
```

---

## Task 8: Watch History tab UI in ChannelList

**Files:**
- Modify: `apps/desktop/src/components/channels/ChannelList.tsx`
- Create: `apps/desktop/src/components/channels/HistoryTab.tsx`

### Step 1: Create `HistoryTab.tsx`

```tsx
import { useState, useEffect, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { Trash2, Clock } from "lucide-react";
import { Tv2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  getWatchHistory,
  deleteHistoryEntry,
  clearWatchHistory,
} from "@/lib/tauri";
import type { WatchHistoryEntry } from "@/lib/types";

function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return m > 0 ? `${h}h ${m}m` : `${h}h`;
}

function formatRelativeTime(ts: number): string {
  const now = Date.now() / 1000;
  const diff = now - ts;
  if (diff < 60) return "Just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 86400 * 7) return `${Math.floor(diff / 86400)}d ago`;
  return new Date(ts * 1000).toLocaleDateString();
}

const CONTENT_TYPE_LABELS: Record<string, { label: string; color: string }> = {
  live:   { label: "LIVE",   color: "text-red-400 bg-red-400/10" },
  movie:  { label: "MOVIE",  color: "text-blue-400 bg-blue-400/10" },
  series: { label: "SERIES", color: "text-purple-400 bg-purple-400/10" },
};

export function HistoryTab() {
  const navigate = useNavigate();
  const [history, setHistory] = useState<WatchHistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [confirmClear, setConfirmClear] = useState(false);

  const load = useCallback(async () => {
    try {
      const entries = await getWatchHistory(500);
      setHistory(entries);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const handleDelete = async (channelId: string) => {
    await deleteHistoryEntry(channelId);
    setHistory(prev => prev.filter(e => e.channelId !== channelId));
  };

  const handleClearAll = async () => {
    if (!confirmClear) { setConfirmClear(true); return; }
    await clearWatchHistory();
    setHistory([]);
    setConfirmClear(false);
  };

  const handlePlay = (entry: WatchHistoryEntry) => {
    // Navigate to player — history entries are simple channels
    navigate("/player", {
      state: { url: "", channelName: entry.channelName },
    });
  };

  if (loading) return null;

  if (history.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-40 gap-2 text-center px-6">
        <Clock className="h-8 w-8 text-muted-foreground/30" />
        <p className="text-sm text-muted-foreground">No watch history yet</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-end px-3 py-1 shrink-0">
        <Button
          variant="ghost"
          size="sm"
          onClick={handleClearAll}
          className={confirmClear ? "text-destructive" : "text-muted-foreground"}
        >
          <Trash2 className="h-3.5 w-3.5 mr-1.5" />
          {confirmClear ? "Confirm clear all" : "Clear all"}
        </Button>
        {confirmClear && (
          <Button variant="ghost" size="sm" onClick={() => setConfirmClear(false)}>
            Cancel
          </Button>
        )}
      </div>

      {/* List */}
      <div className="flex-1 overflow-auto scrollbar-hide px-3 pb-3">
        {history.map((entry) => {
          const badge = CONTENT_TYPE_LABELS[entry.contentType] ?? CONTENT_TYPE_LABELS.live;
          return (
            <div
              key={entry.channelId}
              className="flex items-center gap-3 py-2 px-2 rounded-lg hover:bg-accent transition-colors group"
            >
              <button
                className="flex items-center gap-3 flex-1 min-w-0 text-left"
                onClick={() => handlePlay(entry)}
              >
                <div className="h-8 w-8 rounded bg-secondary flex items-center justify-center overflow-hidden shrink-0">
                  {entry.channelLogo ? (
                    <img src={entry.channelLogo} alt="" className="h-full w-full object-contain" loading="lazy" />
                  ) : (
                    <Tv2 className="h-3.5 w-3.5 text-muted-foreground" />
                  )}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-1.5 mb-0.5">
                    <p className="text-sm leading-tight truncate">{entry.channelName}</p>
                    <span className={`text-[9px] font-semibold px-1.5 py-0.5 rounded-full shrink-0 ${badge.color}`}>
                      {badge.label}
                    </span>
                  </div>
                  <p className="text-[11px] text-muted-foreground">
                    {entry.playCount} {entry.playCount === 1 ? "play" : "plays"}
                    {entry.totalDurationSeconds > 0 && ` · ${formatDuration(entry.totalDurationSeconds)}`}
                    {" · "}{formatRelativeTime(entry.lastWatchedAt)}
                  </p>
                </div>
              </button>
              <button
                onClick={() => handleDelete(entry.channelId)}
                className="shrink-0 p-1.5 rounded text-muted-foreground opacity-0 group-hover:opacity-100 hover:text-destructive transition-all"
                aria-label="Remove from history"
              >
                <Trash2 className="h-3.5 w-3.5" />
              </button>
            </div>
          );
        })}
      </div>
    </div>
  );
}
```

### Step 2: Wire History tab into `ChannelList`

In `ChannelList.tsx`:

```tsx
import { HistoryTab } from "./HistoryTab";
```

In the virtual list section, add a special render path for History tab:

```tsx
{activeTab === "history" ? (
  <HistoryTab />
) : activeTab === "favorites" && allFavorites.length === 0 ? (
  /* empty favorites state */
) : (
  /* existing virtual list */
)}
```

The result count label and category filter should not show for `history` or `favorites` tabs.

### Step 3: Build check + commit

```bash
cd apps/desktop && npx tsc --noEmit
git add apps/desktop/src/components/channels/HistoryTab.tsx \
  apps/desktop/src/components/channels/ChannelList.tsx
git commit -m "feat(history): History tab UI with play count, duration, delete, clear all"
```

---

## Task 9: VideoPlayer — Watch History tracking

**Files:**
- Modify: `apps/desktop/src/components/player/VideoPlayer.tsx`

### Step 1: Add tracking to VideoPlayer

Import at top:

```tsx
import { recordPlayStart, recordPlayEnd } from "@/lib/tauri";
```

Add a ref to track play start time and current channel id for duration calculation:

```tsx
const playStartTimeRef = useRef<number | null>(null);
const playingChannelIdRef = useRef<string | null>(null);
```

After the `useEffect` that loads from `navState?.url`, add a separate tracking effect:

```tsx
// Record play start when a new channel loads
useEffect(() => {
  if (!activeChannel) return;

  // End previous session if channel changed
  if (
    playingChannelIdRef.current &&
    playingChannelIdRef.current !== activeChannel.id &&
    playStartTimeRef.current
  ) {
    const elapsed = Math.floor((Date.now() - playStartTimeRef.current) / 1000);
    recordPlayEnd(playingChannelIdRef.current, elapsed).catch(() => {});
  }

  playStartTimeRef.current = Date.now();
  playingChannelIdRef.current = activeChannel.id;
  recordPlayStart(
    activeChannel.id,
    activeChannel.name,
    activeChannel.logoUrl ?? null,
    activeChannel.contentType
  ).catch(() => {});
}, [activeChannel?.id]);
```

In `handleStop` and on navigate away, record play end:

```tsx
const handleStop = useCallback(() => {
  if (playingChannelIdRef.current && playStartTimeRef.current) {
    const elapsed = Math.floor((Date.now() - playStartTimeRef.current) / 1000);
    recordPlayEnd(playingChannelIdRef.current, elapsed).catch(() => {});
    playingChannelIdRef.current = null;
    playStartTimeRef.current = null;
  }
  mpv.stop();
  navigate("/");
}, [mpv, navigate]);
```

Also record play end on component unmount:

```tsx
useEffect(() => {
  return () => {
    if (playingChannelIdRef.current && playStartTimeRef.current) {
      const elapsed = Math.floor((Date.now() - playStartTimeRef.current) / 1000);
      recordPlayEnd(playingChannelIdRef.current, elapsed).catch(() => {});
    }
  };
}, []);
```

### Step 2: Build check + commit

```bash
cd apps/desktop && npx tsc --noEmit
git add apps/desktop/src/components/player/VideoPlayer.tsx
git commit -m "feat(history): track play start/end in VideoPlayer"
```

---

## Task 10: LiveInfoDrawer — Full EPG schedule

**Files:**
- Modify: `apps/desktop/src/components/channels/LiveInfoDrawer.tsx`

### Step 1: Implement full EPG schedule drawer

Replace the entire `LiveInfoDrawer.tsx` with:

```tsx
import { useState, useEffect, useRef, useCallback } from "react";
import { X, Tv2, CalendarClock, Settings } from "lucide-react";
import { useNavigate } from "react-router-dom";
import type { Channel, EpgProgram } from "@/lib/types";
import { getEpgProgrammes, refreshEpg } from "@/lib/tauri";
import { useChannels } from "@/hooks/useChannels";
import { cn } from "@/lib/utils";

interface LiveInfoDrawerProps {
  channel: Channel;
  onClose: () => void;
}

function formatTime(ts: number): string {
  return new Date(ts * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function formatDuration(start: number, end: number): string {
  const mins = Math.floor((end - start) / 60);
  if (mins < 60) return `${mins} min`;
  const h = Math.floor(mins / 60);
  const m = mins % 60;
  return m > 0 ? `${h}h ${m}m` : `${h}h`;
}

export function LiveInfoDrawer({ channel, onClose }: LiveInfoDrawerProps) {
  const navigate = useNavigate();
  const { providers } = useChannels();
  const [visible, setVisible] = useState(false);
  const [programmes, setProgrammes] = useState<EpgProgram[]>([]);
  const [loading, setLoading] = useState(true);
  const [noEpgUrl, setNoEpgUrl] = useState(false);
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));
  const currentRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const id = requestAnimationFrame(() => setVisible(true));
    return () => cancelAnimationFrame(id);
  }, []);

  // Refresh "now" every 30 seconds for progress bar
  useEffect(() => {
    const timer = setInterval(() => setNow(Math.floor(Date.now() / 1000)), 30_000);
    return () => clearInterval(timer);
  }, []);

  // Fetch EPG programmes on open
  useEffect(() => {
    const tvgId = channel.tvgId;
    if (!tvgId) {
      setLoading(false);
      return;
    }

    // Check if any provider has an EPG URL for this channel
    const hasEpgUrl = providers.some(p => p.epgUrl);
    if (!hasEpgUrl) {
      setNoEpgUrl(true);
      setLoading(false);
      return;
    }

    const dayStart = Math.floor(new Date().setHours(0, 0, 0, 0) / 1000);
    const dayEnd = dayStart + 86400;

    getEpgProgrammes(tvgId, dayStart, dayEnd)
      .then(progs => setProgrammes(progs))
      .catch(() => {})
      .finally(() => setLoading(false));
  }, [channel.tvgId, providers]);

  // Auto-scroll to current programme
  useEffect(() => {
    if (!loading && currentRef.current) {
      currentRef.current.scrollIntoView({ block: "center", behavior: "smooth" });
    }
  }, [loading]);

  const handleClose = () => {
    setVisible(false);
    setTimeout(onClose, 300);
  };

  const currentProg = programmes.find(p => p.startTime <= now && p.endTime > now);
  const progress = currentProg
    ? ((now - currentProg.startTime) / (currentProg.endTime - currentProg.startTime)) * 100
    : 0;

  return (
    <div className="fixed inset-0 z-50">
      <div
        className={`absolute inset-0 bg-black/60 backdrop-blur-sm transition-opacity duration-300 ${
          visible ? "opacity-100" : "opacity-0"
        }`}
        onClick={handleClose}
      />
      <div
        className={`absolute bottom-0 left-0 right-0 bg-card rounded-t-2xl shadow-2xl flex flex-col transition-transform duration-300 ease-out max-h-[85vh] overflow-hidden ${
          visible ? "translate-y-0" : "translate-y-full"
        }`}
      >
        {/* Handle */}
        <div className="flex justify-center pt-3 pb-1 shrink-0">
          <div className="w-9 h-1 rounded-full bg-border" />
        </div>
        <div className="flex justify-end px-5 pt-1 shrink-0">
          <button onClick={handleClose} aria-label="Close" className="text-muted-foreground hover:text-foreground transition-colors">
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Channel info header */}
        <div className="flex gap-4 px-5 pb-4 shrink-0">
          <div className="w-16 h-16 rounded-xl bg-secondary overflow-hidden shrink-0 flex items-center justify-center">
            {channel.logoUrl ? (
              <img src={channel.logoUrl} alt="" className="h-full w-full object-contain p-1" loading="lazy" />
            ) : (
              <Tv2 className="h-7 w-7 text-muted-foreground/30" />
            )}
          </div>
          <div className="flex flex-col justify-center gap-1 flex-1 min-w-0">
            <span className="flex items-center gap-1 text-[10px] font-medium text-red-400 w-fit">
              <span className="h-1.5 w-1.5 rounded-full bg-red-400 animate-pulse" />
              LIVE
            </span>
            <p className="text-base font-semibold leading-tight line-clamp-1">{channel.name}</p>
            {channel.groupTitle && <p className="text-xs text-muted-foreground">{channel.groupTitle}</p>}
          </div>
        </div>

        <div className="border-t border-border mx-5 shrink-0" />

        {/* EPG content */}
        <div className="flex-1 overflow-y-auto px-5 py-4">
          <div className="flex items-center gap-2 mb-3">
            <CalendarClock className="h-4 w-4 text-muted-foreground" />
            <p className="text-sm font-medium">Today's Schedule</p>
          </div>

          {loading && (
            <div className="flex justify-center py-8">
              <div className="h-5 w-5 animate-spin rounded-full border-2 border-primary border-t-transparent" />
            </div>
          )}

          {!loading && noEpgUrl && (
            <div className="flex flex-col items-center gap-3 py-6 text-center">
              <CalendarClock className="h-8 w-8 text-muted-foreground/30" />
              <p className="text-sm text-muted-foreground">No EPG source configured</p>
              <p className="text-xs text-muted-foreground/60">Add an EPG URL in playlist settings to see the schedule.</p>
              <button
                onClick={() => { handleClose(); navigate("/playlists"); }}
                className="flex items-center gap-1.5 text-xs text-primary hover:underline mt-1"
              >
                <Settings className="h-3 w-3" /> Go to Playlist Settings
              </button>
            </div>
          )}

          {!loading && !noEpgUrl && programmes.length === 0 && (
            <div className="flex flex-col items-center gap-2 py-6 text-center">
              <p className="text-sm text-muted-foreground">No schedule available for this channel</p>
              <p className="text-xs text-muted-foreground/60">This channel may not have a tvg-id match in the EPG data.</p>
            </div>
          )}

          {/* Current programme hero */}
          {!loading && currentProg && (
            <div className="mb-4 p-3 rounded-xl bg-primary/10 border border-primary/20">
              <div className="flex items-center gap-2 mb-2">
                <span className="text-[10px] font-semibold text-primary bg-primary/15 px-2 py-0.5 rounded-full">▶ NOW</span>
                <span className="text-xs text-muted-foreground">
                  {formatTime(currentProg.startTime)} – {formatTime(currentProg.endTime)} · {formatDuration(currentProg.startTime, currentProg.endTime)}
                </span>
              </div>
              <p className="text-sm font-semibold mb-1">{currentProg.title}</p>
              {currentProg.description && (
                <p className="text-xs text-muted-foreground line-clamp-2">{currentProg.description}</p>
              )}
              {/* Progress bar */}
              <div className="mt-2 h-1 rounded-full bg-primary/20 overflow-hidden">
                <div
                  className="h-full rounded-full bg-primary transition-all duration-1000"
                  style={{ width: `${Math.min(100, progress)}%` }}
                />
              </div>
            </div>
          )}

          {/* Full schedule list */}
          {!loading && programmes.length > 0 && (
            <div className="space-y-0.5">
              {programmes.map((prog, i) => {
                const isCurrent = prog.startTime <= now && prog.endTime > now;
                const isPast = prog.endTime <= now;
                return (
                  <div
                    key={i}
                    ref={isCurrent ? currentRef : undefined}
                    className={cn(
                      "flex items-start gap-3 px-2 py-2 rounded-lg",
                      isCurrent && "bg-primary/5",
                      isPast && "opacity-40"
                    )}
                  >
                    <span className="text-xs text-muted-foreground w-12 shrink-0 mt-0.5 tabular-nums">
                      {formatTime(prog.startTime)}
                    </span>
                    <div className="flex-1 min-w-0">
                      <p className={cn("text-sm leading-snug", isCurrent && "font-semibold")}>{prog.title}</p>
                      {prog.category && (
                        <p className="text-[10px] text-muted-foreground mt-0.5">{prog.category}</p>
                      )}
                    </div>
                    {isCurrent && (
                      <span className="text-[9px] font-semibold text-primary bg-primary/15 px-1.5 py-0.5 rounded-full shrink-0 self-center">
                        NOW
                      </span>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>

        <div className="shrink-0 pb-2" />
      </div>
    </div>
  );
}
```

Note: `providers` must be exposed from `useChannels`. Check if it's already in the context; if not, add `providers: providers` to the context value.

### Step 2: Build check + commit

```bash
cd apps/desktop && npx tsc --noEmit
git add apps/desktop/src/components/channels/LiveInfoDrawer.tsx
git commit -m "feat(epg): full EPG schedule in LiveInfoDrawer with progress bar"
```

---

## Task 11: OMDB enrichment in MovieInfoDrawer + SeriesDetailModal

**Files:**
- Modify: `apps/desktop/src/components/channels/MovieInfoDrawer.tsx`
- Modify: `apps/desktop/src/components/channels/SeriesDetailModal.tsx`

### Step 1: Enhance `MovieInfoDrawer` with OMDB data

At the top of the drawer, after the channel/provider data loads, trigger an OMDB fetch:

Add to `MovieInfoDrawer.tsx`:

```tsx
import { useState, useEffect } from "react";
import { fetchOmdbData } from "@/lib/tauri";
import type { OmdbData } from "@/lib/types";
import { Star, ExternalLink } from "lucide-react";
```

Inside the component, add:

```tsx
const [omdb, setOmdb] = useState<OmdbData | null>(null);
const [omdbLoading, setOmdbLoading] = useState(true);

useEffect(() => {
  fetchOmdbData(movie.id, movie.name, "movie")
    .then(data => setOmdb(data))
    .catch(() => {})
    .finally(() => setOmdbLoading(false));
}, [movie.id, movie.name]);
```

Update the poster section to use OMDB poster if available:

```tsx
const posterSrc = omdb?.posterUrl || movie.logoUrl;
```

Add OMDB data display below the title, before the source/play controls:

```tsx
{/* Ratings row */}
{omdb && (omdb.imdbRating || omdb.rottenTomatoes) && (
  <div className="flex items-center gap-3 mt-1.5">
    {omdb.imdbRating && (
      <span className="flex items-center gap-1 text-xs font-medium text-yellow-400">
        <Star className="h-3 w-3 fill-yellow-400" />
        {omdb.imdbRating}
      </span>
    )}
    {omdb.rottenTomatoes && (
      <span className="text-xs font-medium text-orange-400">🍅 {omdb.rottenTomatoes}</span>
    )}
    {omdb.rated && (
      <span className="text-[10px] px-1.5 py-0.5 rounded bg-secondary text-secondary-foreground font-medium">
        {omdb.rated}
      </span>
    )}
  </div>
)}

{/* Metadata row */}
{omdb && (
  <div className="flex flex-wrap gap-x-3 gap-y-0.5 mt-1.5 text-xs text-muted-foreground">
    {omdb.genre && <span>{omdb.genre}</span>}
    {omdb.runtime && <span>· {omdb.runtime}</span>}
    {omdb.year && <span>· {omdb.year}</span>}
  </div>
)}

{/* Director / Cast */}
{omdb?.director && (
  <p className="text-xs text-muted-foreground mt-1">
    <span className="font-medium text-foreground/70">Dir:</span> {omdb.director}
  </p>
)}
{omdb?.actors && (
  <p className="text-xs text-muted-foreground mt-0.5">
    <span className="font-medium text-foreground/70">Cast:</span>{" "}
    {omdb.actors.split(", ").slice(0, 3).join(", ")}
  </p>
)}

{/* Plot */}
{omdb?.plot && (
  <p className="text-sm text-muted-foreground mt-2 leading-relaxed">{omdb.plot}</p>
)}

{/* No API key nudge */}
{!omdbLoading && !omdb && (
  <p className="text-xs text-muted-foreground/60 mt-2">
    Add an OMDB API key in Settings → Integrations for ratings & plot.
  </p>
)}
```

### Step 2: Enhance `SeriesDetailModal` info card with OMDB

In `SeriesDetailModal.tsx`, on the info step, trigger OMDB fetch using `showTitle`:

```tsx
const [omdb, setOmdb] = useState<OmdbData | null>(null);

useEffect(() => {
  if (!showTitle) return;
  // Use the first episode's channel id as cache key
  const cacheId = episodes[0]?.id ?? showTitle;
  fetchOmdbData(cacheId, showTitle, "series")
    .then(data => setOmdb(data))
    .catch(() => {});
}, [showTitle, episodes]);
```

Display in the info card's right panel (stats panel), add ratings above the season/episode stats:

```tsx
{omdb && (omdb.imdbRating || omdb.rottenTomatoes) && (
  <div className="flex gap-2 mb-3">
    {omdb.imdbRating && (
      <div className="flex-1 rounded-lg bg-yellow-400/10 p-2 text-center">
        <p className="text-xs text-muted-foreground">IMDB</p>
        <p className="text-sm font-bold text-yellow-400">★ {omdb.imdbRating}</p>
      </div>
    )}
    {omdb.rottenTomatoes && (
      <div className="flex-1 rounded-lg bg-orange-400/10 p-2 text-center">
        <p className="text-xs text-muted-foreground">RT</p>
        <p className="text-sm font-bold text-orange-400">{omdb.rottenTomatoes}</p>
      </div>
    )}
  </div>
)}
```

And below the existing season/episode count stats, add plot:

```tsx
{omdb?.plot && (
  <p className="text-xs text-muted-foreground mt-3 leading-relaxed line-clamp-4">{omdb.plot}</p>
)}
```

### Step 3: Build check + commit

```bash
cd apps/desktop && npx tsc --noEmit
git add apps/desktop/src/components/channels/MovieInfoDrawer.tsx \
  apps/desktop/src/components/channels/SeriesDetailModal.tsx
git commit -m "feat(omdb): enrich MovieInfoDrawer and SeriesDetailModal with OMDB ratings/plot"
```

---

## Task 12: Settings page — Integrations + History sections

**Files:**
- Modify: `apps/desktop/src/components/settings/Settings.tsx`

### Step 1: Rewrite Settings page with new sections

Replace `Settings.tsx` with a tabbed layout: General | Integrations | History | About.

```tsx
import { useState, useEffect } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Slider } from "@/components/ui/slider";
import { usePlatform } from "@/hooks/usePlatform";
import { Settings as SettingsIcon, Monitor, Smartphone, Tv, Eye, EyeOff } from "lucide-react";
import {
  getOmdbApiKey,
  setOmdbApiKey,
  fetchOmdbData,
  clearWatchHistory,
} from "@/lib/tauri";
import { cn } from "@/lib/utils";

type SettingsTab = "general" | "integrations" | "history" | "about";

function TabButton({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "px-4 py-2 text-sm font-medium border-b-2 transition-colors",
        active
          ? "border-primary text-primary"
          : "border-transparent text-muted-foreground hover:text-foreground"
      )}
    >
      {children}
    </button>
  );
}

function GeneralTab() {
  const { platform, layoutMode } = usePlatform();
  const [hwAccel, setHwAccel] = useState(true);
  const [defaultVolume, setDefaultVolume] = useState(100);
  const platformIcon = { desktop: Monitor, mobile: Smartphone, tv: Tv }[layoutMode];
  const PlatformIcon = platformIcon;

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader><CardTitle className="text-base">Platform</CardTitle></CardHeader>
        <CardContent>
          <div className="flex items-center gap-3">
            <PlatformIcon className="h-5 w-5 text-muted-foreground" />
            <div>
              <p className="text-sm font-medium capitalize">{platform}</p>
              <p className="text-xs text-muted-foreground">Layout: {layoutMode}</p>
            </div>
          </div>
        </CardContent>
      </Card>
      <Card>
        <CardHeader><CardTitle className="text-base">Playback</CardTitle></CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm font-medium">Hardware Acceleration</p>
              <p className="text-xs text-muted-foreground">Use GPU decoding when available</p>
            </div>
            <Button variant={hwAccel ? "default" : "secondary"} size="sm" onClick={() => setHwAccel(!hwAccel)}>
              {hwAccel ? "On" : "Off"}
            </Button>
          </div>
          <div>
            <div className="flex items-center justify-between mb-2">
              <p className="text-sm font-medium">Default Volume</p>
              <span className="text-sm text-muted-foreground">{defaultVolume}%</span>
            </div>
            <Slider value={defaultVolume} min={0} max={150} step={5} onValueChange={setDefaultVolume} />
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function IntegrationsTab() {
  const [apiKey, setApiKey] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<"idle" | "ok" | "error">("idle");

  useEffect(() => {
    getOmdbApiKey().then(key => { if (key) setApiKey(key); }).catch(() => {});
  }, []);

  const handleSave = async () => {
    setSaving(true);
    try {
      await setOmdbApiKey(apiKey.trim());
      setTestResult("idle");
    } finally {
      setSaving(false);
    }
  };

  const handleTest = async () => {
    setTesting(true);
    setTestResult("idle");
    try {
      // Save key first, then test with a known movie
      await setOmdbApiKey(apiKey.trim());
      const result = await fetchOmdbData("__test__", "The Dark Knight", "movie");
      setTestResult(result ? "ok" : "error");
    } catch {
      setTestResult("error");
    } finally {
      setTesting(false);
    }
  };

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader><CardTitle className="text-base">OMDB API</CardTitle></CardHeader>
        <CardContent className="space-y-3">
          <p className="text-xs text-muted-foreground">
            OMDB provides ratings, plot summaries, and cast info for movies and TV shows.
            Get a free key at{" "}
            <span className="text-primary">omdbapi.com</span> (1000 requests/day).
          </p>
          <div>
            <label className="text-xs text-muted-foreground mb-1 block">API Key</label>
            <div className="flex gap-2">
              <div className="relative flex-1">
                <Input
                  type={showKey ? "text" : "password"}
                  value={apiKey}
                  onChange={e => setApiKey(e.target.value)}
                  placeholder="Enter your OMDB API key"
                  className="h-8 text-sm pr-8"
                />
                <button
                  type="button"
                  onClick={() => setShowKey(v => !v)}
                  className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                >
                  {showKey ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
                </button>
              </div>
              <Button size="sm" variant="secondary" onClick={handleTest} disabled={testing || !apiKey}>
                {testing ? "Testing…" : "Test"}
              </Button>
              <Button size="sm" onClick={handleSave} disabled={saving || !apiKey}>
                {saving ? "Saving…" : "Save"}
              </Button>
            </div>
          </div>
          {testResult === "ok" && (
            <p className="text-xs text-green-500">✓ API key is valid</p>
          )}
          {testResult === "error" && (
            <p className="text-xs text-destructive">✗ Invalid key or network error</p>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

function HistorySettingsTab() {
  const [confirmClear, setConfirmClear] = useState(false);

  const handleClearAll = async () => {
    if (!confirmClear) { setConfirmClear(true); return; }
    await clearWatchHistory();
    setConfirmClear(false);
  };

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader><CardTitle className="text-base">Watch History</CardTitle></CardHeader>
        <CardContent className="space-y-3">
          <p className="text-xs text-muted-foreground">
            Your watch history is stored locally. It tracks play count and total watch time per channel.
          </p>
          <div className="flex items-center justify-between pt-1">
            <div>
              <p className="text-sm font-medium">Clear All History</p>
              <p className="text-xs text-muted-foreground">Permanently delete all watch history</p>
            </div>
            <div className="flex gap-2">
              {confirmClear && (
                <Button variant="ghost" size="sm" onClick={() => setConfirmClear(false)}>
                  Cancel
                </Button>
              )}
              <Button
                variant={confirmClear ? "destructive" : "secondary"}
                size="sm"
                onClick={handleClearAll}
              >
                {confirmClear ? "Confirm" : "Clear All"}
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

export function Settings() {
  const [tab, setTab] = useState<SettingsTab>("general");

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center gap-3 px-4 pt-4 pb-2 shrink-0">
        <SettingsIcon className="h-5 w-5 text-primary" />
        <h1 className="text-lg font-bold">Settings</h1>
      </div>

      {/* Tab bar */}
      <div className="flex border-b border-border px-4 shrink-0">
        <TabButton active={tab === "general"} onClick={() => setTab("general")}>General</TabButton>
        <TabButton active={tab === "integrations"} onClick={() => setTab("integrations")}>Integrations</TabButton>
        <TabButton active={tab === "history"} onClick={() => setTab("history")}>History</TabButton>
        <TabButton active={tab === "about"} onClick={() => setTab("about")}>About</TabButton>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-4 max-w-2xl">
        {tab === "general" && <GeneralTab />}
        {tab === "integrations" && <IntegrationsTab />}
        {tab === "history" && <HistorySettingsTab />}
        {tab === "about" && (
          <Card>
            <CardHeader><CardTitle className="text-base">About</CardTitle></CardHeader>
            <CardContent>
              <p className="text-sm text-muted-foreground">MaxVideoPlayer v0.1.0</p>
              <p className="text-xs text-muted-foreground mt-1">Built with Tauri v2, React, and libmpv</p>
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  );
}
```

### Step 2: Build check + commit

```bash
cd apps/desktop && npx tsc --noEmit
git add apps/desktop/src/components/settings/Settings.tsx
git commit -m "feat(settings): add Integrations (OMDB key) and History tabs to Settings page"
```

---

## Task 13: ProviderSettingsModal — EPG section

**Files:**
- Modify: `apps/desktop/src/components/playlist/ProviderSettingsModal.tsx`
- Modify: `apps/desktop/src/hooks/useChannels.ts`

### Step 1: Add EPG settings to `ProviderSettings` type

In `useChannels.ts`, update `ProviderSettings`:

```ts
export interface ProviderSettings {
  autoRefresh: boolean;
  refreshIntervalHours: number;
  epgAutoRefresh: boolean;        // NEW (default: true)
  epgRefreshIntervalHours: number; // NEW (default: 24)
}
```

Update `loadProviderSettings` to include defaults for new fields:

```ts
return {
  autoRefresh: parsed.autoRefresh ?? false,
  refreshIntervalHours: parsed.refreshIntervalHours ?? 24,
  epgAutoRefresh: parsed.epgAutoRefresh ?? true,
  epgRefreshIntervalHours: parsed.epgRefreshIntervalHours ?? 24,
};
```

In the startup interval check, also trigger `refreshEpg` if overdue:

```ts
// After the existing playlist auto-refresh check loop, add:
for (const p of providerList) {
  const { epgAutoRefresh, epgRefreshIntervalHours } = loadProviderSettings(p.id);
  if (!epgAutoRefresh || !p.epgUrl) continue;
  const intervalMs = epgRefreshIntervalHours * 60 * 60 * 1000;
  if (!p.lastUpdated || Date.now() - new Date(p.lastUpdated).getTime() > intervalMs) {
    refreshEpgApi(p.id).catch(() => {});
  }
}
```

Import: `import { refreshEpg as refreshEpgApi } from "@/lib/tauri";`

### Step 2: Add EPG section to `ProviderSettingsModal`

Add EPG state:

```tsx
const [epgUrl, setEpgUrl] = useState(provider.epgUrl ?? "");
const [epgAutoRefresh, setEpgAutoRefresh] = useState(initial.epgAutoRefresh);
const [epgIntervalHours, setEpgIntervalHours] = useState(initial.epgRefreshIntervalHours);
const [detectingEpg, setDetectingEpg] = useState(false);
```

Add detect handler:

```tsx
const handleDetectEpg = async () => {
  setDetectingEpg(true);
  try {
    // For Xtream: construct xmltv URL from credentials
    if (isXtream) {
      const detected = `${url.trim().replace(/\/$/, "")}/xmltv.php?username=${username}&password=${password}`;
      setEpgUrl(detected);
    } else {
      // For M3U: EPG URL is saved on provider from initial load
      setEpgUrl(provider.epgUrl ?? "");
    }
  } finally {
    setDetectingEpg(false);
  }
};
```

In `handleSave`, also save EPG settings:

```tsx
await setEpgUrlApi(provider.id, epgUrl.trim() || null);
saveProviderSettings(provider.id, {
  autoRefresh,
  refreshIntervalHours: intervalHours,
  epgAutoRefresh,
  epgRefreshIntervalHours: epgIntervalHours,
});
```

Import: `import { setEpgUrl as setEpgUrlApi } from "@/lib/tauri";`

Add EPG section to the modal body (after the Auto-refresh section):

```tsx
{/* EPG Schedule section */}
{!isFile && (
  <div>
    <SectionLabel>EPG Schedule</SectionLabel>
    <div className="space-y-3">
      <div>
        <label className="text-xs text-muted-foreground mb-1 block">EPG URL</label>
        <div className="flex gap-2">
          <Input
            value={epgUrl}
            onChange={(e) => setEpgUrl(e.target.value)}
            placeholder="http://example.com/epg.xml"
            className="h-8 text-sm flex-1"
          />
          <Button
            variant="secondary"
            size="sm"
            onClick={handleDetectEpg}
            disabled={detectingEpg}
            className="shrink-0"
          >
            {detectingEpg ? "…" : "Auto-detect"}
          </Button>
        </div>
      </div>

      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm font-medium">Auto-refresh EPG</p>
          <p className="text-xs text-muted-foreground">Refresh schedule data automatically</p>
        </div>
        <div className="flex gap-0.5 p-0.5 bg-secondary rounded-lg ml-3 shrink-0">
          <button
            onClick={() => setEpgAutoRefresh(false)}
            className={cn("px-3 py-1 rounded-md text-xs font-medium transition-colors",
              !epgAutoRefresh ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"
            )}
          >Off</button>
          <button
            onClick={() => setEpgAutoRefresh(true)}
            className={cn("px-3 py-1 rounded-md text-xs font-medium transition-colors",
              epgAutoRefresh ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"
            )}
          >On</button>
        </div>
      </div>

      {epgAutoRefresh && (
        <div>
          <label className="text-xs text-muted-foreground mb-1 block">Refresh interval</label>
          <select
            value={epgIntervalHours}
            onChange={(e) => setEpgIntervalHours(Number(e.target.value))}
            className="w-full bg-secondary text-sm rounded-lg px-3 py-2 border border-transparent focus:outline-none focus:border-primary appearance-none cursor-pointer"
          >
            {INTERVAL_OPTIONS.map((opt) => (
              <option key={opt.hours} value={opt.hours}>{opt.label}</option>
            ))}
          </select>
        </div>
      )}
    </div>
  </div>
)}
```

### Step 3: Build check + run all tests

```bash
cargo test -p mvp-core
cd apps/desktop && npx tsc --noEmit && npm test -- --run
```
Expected: all tests pass.

### Step 4: Commit

```bash
git add apps/desktop/src/components/playlist/ProviderSettingsModal.tsx \
  apps/desktop/src/hooks/useChannels.ts
git commit -m "feat(epg): EPG section in ProviderSettingsModal with auto-refresh settings"
```

---

## Final Verification

```bash
# All Rust tests
cargo test

# TypeScript type check
cd apps/desktop && npx tsc --noEmit

# Frontend unit tests
cd apps/desktop && npm test -- --run
```

Expected: 49+ Rust tests pass, 0 TypeScript errors, 34+ frontend tests pass.
