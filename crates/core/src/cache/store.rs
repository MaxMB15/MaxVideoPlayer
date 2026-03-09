use crate::iptv::m3u::parse_series_name;
use crate::iptv::omdb::OmdbData;
use crate::models::channel::Channel;
use crate::models::playlist::{Provider, ProviderType};
use rusqlite::{params, Connection, Result as SqlResult};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct StoredEpgProgram {
    pub channel_id: String,
    pub title: String,
    pub description: Option<String>,
    pub start_time: i64,
    pub end_time: i64,
    pub category: Option<String>,
    pub provider_id: String,
    pub fetched_at: i64,
}

pub struct CacheStore {
    conn: Connection,
}

impl CacheStore {
    pub fn open(db_path: &Path) -> Result<Self, CacheError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        let store = Self { conn };
        store.init_tables()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self, CacheError> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.init_tables()?;
        Ok(store)
    }

    fn init_tables(&self) -> Result<(), CacheError> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS providers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                provider_type TEXT NOT NULL,
                url TEXT NOT NULL,
                username TEXT,
                password TEXT,
                last_updated TEXT,
                channel_count INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS channels (
                id TEXT PRIMARY KEY,
                provider_id TEXT NOT NULL,
                name TEXT NOT NULL,
                url TEXT NOT NULL,
                logo_url TEXT,
                group_title TEXT NOT NULL DEFAULT '',
                tvg_id TEXT,
                tvg_name TEXT,
                is_favorite INTEGER NOT NULL DEFAULT 0,
                content_type TEXT NOT NULL DEFAULT 'live',
                sources TEXT NOT NULL DEFAULT '[]',
                FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS epg_cache (
                channel_id TEXT NOT NULL,
                data_json TEXT NOT NULL,
                fetched_at TEXT NOT NULL,
                PRIMARY KEY (channel_id, fetched_at)
            );

            CREATE INDEX IF NOT EXISTS idx_channels_provider ON channels(provider_id);
            CREATE INDEX IF NOT EXISTS idx_channels_group ON channels(group_title);
            CREATE INDEX IF NOT EXISTS idx_channels_favorite ON channels(is_favorite);",
        )?;
        // Migrate: add content_type if it doesn't exist yet (SQLite ignores duplicate columns error)
        let _ = self.conn.execute_batch(
            "ALTER TABLE channels ADD COLUMN content_type TEXT NOT NULL DEFAULT 'live';"
        );
        let _ = self.conn.execute_batch(
            "ALTER TABLE channels ADD COLUMN sources TEXT NOT NULL DEFAULT '[]';"
        );
        let _ = self.conn.execute_batch(
            "ALTER TABLE channels ADD COLUMN series_title TEXT;"
        );
        let _ = self.conn.execute_batch(
            "ALTER TABLE channels ADD COLUMN season INTEGER;"
        );
        let _ = self.conn.execute_batch(
            "ALTER TABLE channels ADD COLUMN episode INTEGER;"
        );
        let _ = self.conn.execute_batch("ALTER TABLE providers ADD COLUMN epg_url TEXT;");
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
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS omdb_cache (
                channel_id  TEXT PRIMARY KEY,
                data_json   TEXT NOT NULL,
                fetched_at  INTEGER NOT NULL
            );"
        )?;
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
        Ok(())
    }

    // --- Providers ---

    pub fn upsert_provider(&self, provider: &Provider) -> Result<(), CacheError> {
        let ptype = match provider.provider_type {
            ProviderType::M3u => "m3u",
            ProviderType::Xtream => "xtream",
        };
        // Use INSERT ... ON CONFLICT DO UPDATE (not INSERT OR REPLACE) so the existing
        // row is updated in-place rather than deleted + re-inserted.  DELETE + INSERT
        // would trigger ON DELETE CASCADE and wipe all channels for the provider.
        self.conn.execute(
            "INSERT INTO providers (id, name, provider_type, url, username, password, last_updated, channel_count, epg_url)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
               name          = excluded.name,
               provider_type = excluded.provider_type,
               url           = excluded.url,
               username      = excluded.username,
               password      = excluded.password,
               last_updated  = excluded.last_updated,
               channel_count = excluded.channel_count,
               epg_url       = excluded.epg_url",
            params![
                provider.id,
                provider.name,
                ptype,
                provider.url,
                provider.username,
                provider.password,
                provider.last_updated,
                provider.channel_count as i64,
                provider.epg_url,
            ],
        )?;
        Ok(())
    }

    pub fn update_provider_credentials(
        &self,
        id: &str,
        name: &str,
        url: &str,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Result<(), CacheError> {
        self.conn.execute(
            "UPDATE providers SET name = ?1, url = ?2, username = ?3, password = ?4 WHERE id = ?5",
            params![name, url, username, password, id],
        )?;
        Ok(())
    }

    pub fn get_providers(&self) -> Result<Vec<Provider>, CacheError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, provider_type, url, username, password, last_updated, channel_count, epg_url FROM providers"
        )?;
        let providers = stmt.query_map([], |row| {
            let ptype: String = row.get(2)?;
            Ok(Provider {
                id: row.get(0)?,
                name: row.get(1)?,
                provider_type: if ptype == "xtream" {
                    ProviderType::Xtream
                } else {
                    ProviderType::M3u
                },
                url: row.get(3)?,
                username: row.get(4)?,
                password: row.get(5)?,
                last_updated: row.get(6)?,
                channel_count: row.get::<_, i64>(7)? as usize,
                epg_url: row.get(8)?,
            })
        })?.collect::<SqlResult<Vec<_>>>()?;
        Ok(providers)
    }

    pub fn get_provider(&self, id: &str) -> Result<Option<Provider>, CacheError> {
        let result = self.conn.query_row(
            "SELECT id, name, provider_type, url, username, password, last_updated, channel_count, epg_url FROM providers WHERE id = ?1",
            params![id],
            |row| {
                let ptype: String = row.get(2)?;
                Ok(Provider {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    provider_type: if ptype == "xtream" { ProviderType::Xtream } else { ProviderType::M3u },
                    url: row.get(3)?,
                    username: row.get(4)?,
                    password: row.get(5)?,
                    last_updated: row.get(6)?,
                    channel_count: row.get::<_, i64>(7)? as usize,
                    epg_url: row.get(8)?,
                })
            },
        );
        match result {
            Ok(p) => Ok(Some(p)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CacheError::Db(e)),
        }
    }

    pub fn remove_provider(&self, id: &str) -> Result<(), CacheError> {
        self.conn.execute("DELETE FROM channels WHERE provider_id = ?1", params![id])?;
        self.conn.execute("DELETE FROM providers WHERE id = ?1", params![id])?;
        Ok(())
    }

    // --- Channels ---

    pub fn save_channels(&self, provider_id: &str, channels: &[Channel]) -> Result<(), CacheError> {
        self.conn.execute_batch("BEGIN")?;

        if let Err(e) = self.save_channels_inner(provider_id, channels) {
            let _ = self.conn.execute_batch("ROLLBACK");
            return Err(e);
        }

        self.conn.execute_batch("COMMIT")?;
        Ok(())
    }

    fn save_channels_inner(&self, provider_id: &str, channels: &[Channel]) -> Result<(), CacheError> {
        self.conn.execute("DELETE FROM channels WHERE provider_id = ?1", params![provider_id])?;

        let mut stmt = self.conn.prepare(
            "INSERT INTO channels (id, provider_id, name, url, logo_url, group_title, tvg_id, tvg_name, is_favorite, content_type, sources, series_title, season, episode)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"
        )?;

        for ch in channels {
            let sources_json = serde_json::to_string(&ch.sources)
                .unwrap_or_else(|_| "[]".to_string());
            stmt.execute(params![
                ch.id,
                provider_id,
                ch.name,
                ch.url,
                ch.logo_url,
                ch.group_title,
                ch.tvg_id,
                ch.tvg_name,
                ch.is_favorite as i32,
                &ch.content_type,
                sources_json,
                ch.series_title,
                ch.season.map(|s| s as i64),
                ch.episode.map(|e| e as i64),
            ])?;
        }
        Ok(())
    }

    pub fn get_channels(&self, provider_id: &str) -> Result<Vec<Channel>, CacheError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, url, logo_url, group_title, tvg_id, tvg_name, is_favorite, content_type, sources, series_title, season, episode
             FROM channels WHERE provider_id = ?1 ORDER BY name"
        )?;
        let mut channels = stmt.query_map(params![provider_id], |row| {
            let sources_json: String = row.get(9).unwrap_or_else(|_| "[]".to_string());
            let sources: Vec<String> = serde_json::from_str(&sources_json).unwrap_or_default();
            Ok(Channel {
                id: row.get(0)?,
                name: row.get(1)?,
                url: row.get(2)?,
                logo_url: row.get(3)?,
                group_title: row.get(4)?,
                tvg_id: row.get(5)?,
                tvg_name: row.get(6)?,
                is_favorite: row.get::<_, i32>(7)? != 0,
                content_type: row.get(8)?,
                sources,
                series_title: row.get(10)?,
                season: row.get::<_, Option<i64>>(11)?.map(|s| s as u32),
                episode: row.get::<_, Option<i64>>(12)?.map(|e| e as u32),
            })
        })?.collect::<SqlResult<Vec<_>>>()?;
        enrich_stale_series(&mut channels);
        Ok(channels)
    }

    pub fn get_all_channels(&self) -> Result<Vec<Channel>, CacheError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, url, logo_url, group_title, tvg_id, tvg_name, is_favorite, content_type, sources, series_title, season, episode
             FROM channels ORDER BY name"
        )?;
        let mut channels = stmt.query_map([], |row| {
            let sources_json: String = row.get(9).unwrap_or_else(|_| "[]".to_string());
            let sources: Vec<String> = serde_json::from_str(&sources_json).unwrap_or_default();
            Ok(Channel {
                id: row.get(0)?,
                name: row.get(1)?,
                url: row.get(2)?,
                logo_url: row.get(3)?,
                group_title: row.get(4)?,
                tvg_id: row.get(5)?,
                tvg_name: row.get(6)?,
                is_favorite: row.get::<_, i32>(7)? != 0,
                content_type: row.get(8)?,
                sources,
                series_title: row.get(10)?,
                season: row.get::<_, Option<i64>>(11)?.map(|s| s as u32),
                episode: row.get::<_, Option<i64>>(12)?.map(|e| e as u32),
            })
        })?.collect::<SqlResult<Vec<_>>>()?;
        enrich_stale_series(&mut channels);
        Ok(channels)
    }

    /// Look up a single channel by its ID. Used to resolve Xtream series metadata.
    pub fn get_channel_by_id(&self, channel_id: &str) -> Result<Option<Channel>, CacheError> {
        let result = self.conn.query_row(
            "SELECT id, name, url, logo_url, group_title, tvg_id, tvg_name, is_favorite, content_type, sources, series_title, season, episode
             FROM channels WHERE id = ?1",
            params![channel_id],
            |row| {
                let sources_json: String = row.get(9).unwrap_or_else(|_| "[]".to_string());
                let sources: Vec<String> = serde_json::from_str(&sources_json).unwrap_or_default();
                Ok(Channel {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    url: row.get(2)?,
                    logo_url: row.get(3)?,
                    group_title: row.get(4)?,
                    tvg_id: row.get(5)?,
                    tvg_name: row.get(6)?,
                    is_favorite: row.get::<_, i32>(7)? != 0,
                    content_type: row.get(8)?,
                    sources,
                    series_title: row.get(10)?,
                    season: row.get::<_, Option<i64>>(11)?.map(|s| s as u32),
                    episode: row.get::<_, Option<i64>>(12)?.map(|e| e as u32),
                })
            },
        );
        match result {
            Ok(ch) => Ok(Some(ch)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CacheError::Db(e)),
        }
    }

    /// Find the provider that owns a given channel (JOIN query).
    pub fn get_provider_for_channel(&self, channel_id: &str) -> Result<Option<Provider>, CacheError> {
        let result = self.conn.query_row(
            "SELECT p.id, p.name, p.provider_type, p.url, p.username, p.password, p.last_updated, p.channel_count, p.epg_url
             FROM providers p JOIN channels c ON c.provider_id = p.id WHERE c.id = ?1",
            params![channel_id],
            |row| {
                let ptype: String = row.get(2)?;
                Ok(Provider {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    provider_type: if ptype == "xtream" { ProviderType::Xtream } else { ProviderType::M3u },
                    url: row.get(3)?,
                    username: row.get(4)?,
                    password: row.get(5)?,
                    last_updated: row.get(6)?,
                    channel_count: row.get::<_, i64>(7)? as usize,
                    epg_url: row.get(8)?,
                })
            },
        );
        match result {
            Ok(p) => Ok(Some(p)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CacheError::Db(e)),
        }
    }

    pub fn toggle_favorite(&self, channel_id: &str) -> Result<bool, CacheError> {
        let current: i32 = self.conn.query_row(
            "SELECT is_favorite FROM channels WHERE id = ?1",
            params![channel_id],
            |row| row.get(0),
        )?;
        let new_val = if current == 0 { 1 } else { 0 };
        self.conn.execute(
            "UPDATE channels SET is_favorite = ?1 WHERE id = ?2",
            params![new_val, channel_id],
        )?;
        Ok(new_val == 1)
    }

    // --- EPG Cache ---

    pub fn save_epg_data(&self, channel_id: &str, json: &str) -> Result<(), CacheError> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT OR REPLACE INTO epg_cache (channel_id, data_json, fetched_at) VALUES (?1, ?2, ?3)",
            params![channel_id, json, now],
        )?;
        Ok(())
    }

    pub fn get_epg_data(&self, channel_id: &str) -> Result<Option<String>, CacheError> {
        let result = self.conn.query_row(
            "SELECT data_json FROM epg_cache WHERE channel_id = ?1 ORDER BY fetched_at DESC LIMIT 1",
            params![channel_id],
            |row| row.get(0),
        );
        match result {
            Ok(json) => Ok(Some(json)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CacheError::Db(e)),
        }
    }

    // --- EPG Programmes ---

    pub fn save_epg_programmes(
        &self,
        provider_id: &str,
        programmes: &[StoredEpgProgram],
    ) -> Result<(), CacheError> {
        self.conn.execute_batch("BEGIN")?;
        match self.save_epg_programmes_inner(provider_id, programmes) {
            Ok(()) => { self.conn.execute_batch("COMMIT")?; Ok(()) }
            Err(e) => { let _ = self.conn.execute_batch("ROLLBACK"); Err(e) }
        }
    }

    fn save_epg_programmes_inner(
        &self,
        provider_id: &str,
        programmes: &[StoredEpgProgram],
    ) -> Result<(), CacheError> {
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
                    prog.channel_id, prog.title, prog.description,
                    prog.start_time, prog.end_time, prog.category,
                    prog.provider_id, prog.fetched_at,
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

    // --- OMDB Cache ---

    /// Store OMDB data for a channel. Overwrites any existing cached entry.
    pub fn save_omdb_cache(&self, channel_id: &str, data: &OmdbData) -> Result<(), CacheError> {
        let data_json = serde_json::to_string(data)?;
        let now = chrono::Utc::now().timestamp();
        self.conn.execute(
            "INSERT OR REPLACE INTO omdb_cache (channel_id, data_json, fetched_at) VALUES (?1, ?2, ?3)",
            params![channel_id, data_json, now],
        )?;
        Ok(())
    }

    /// Retrieve cached OMDB data for a channel. Returns `None` if not cached or if the
    /// cached entry is older than `ttl_seconds`.
    pub fn get_omdb_cache(&self, channel_id: &str, ttl_seconds: i64) -> Result<Option<OmdbData>, CacheError> {
        let result = self.conn.query_row(
            "SELECT data_json, fetched_at FROM omdb_cache WHERE channel_id = ?1",
            params![channel_id],
            |row| {
                let data_json: String = row.get(0)?;
                let fetched_at: i64 = row.get(1)?;
                Ok((data_json, fetched_at))
            },
        );
        match result {
            Ok((data_json, fetched_at)) => {
                let now = chrono::Utc::now().timestamp();
                if now - fetched_at > ttl_seconds {
                    return Ok(None); // stale
                }
                let data: OmdbData = serde_json::from_str(&data_json)?;
                Ok(Some(data))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CacheError::Db(e)),
        }
    }

    // --- Watch History ---

    /// Upsert a watch history entry. If the channel has been watched before, increments
    /// `play_count` and updates `last_watched_at`. On first watch, inserts with `play_count=1`.
    pub fn record_play_start(
        &self,
        channel_id: &str,
        channel_name: &str,
        channel_logo: Option<&str>,
        content_type: &str,
    ) -> Result<(), CacheError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.conn.execute(
            "INSERT INTO watch_history
                (channel_id, channel_name, channel_logo, content_type, first_watched_at, last_watched_at, total_duration_seconds, play_count)
             VALUES
                (?1, ?2, ?3, ?4, ?5, ?5, 0, 1)
             ON CONFLICT(channel_id) DO UPDATE SET
                channel_name    = excluded.channel_name,
                channel_logo    = excluded.channel_logo,
                last_watched_at = excluded.last_watched_at,
                play_count      = play_count + 1",
            params![channel_id, channel_name, channel_logo, content_type, now],
        )?;
        Ok(())
    }

    /// Add elapsed seconds to `total_duration_seconds` for the given channel.
    /// Noop if the channel is not found in history.
    pub fn record_play_end(&self, channel_id: &str, duration_seconds: i64) -> Result<(), CacheError> {
        self.conn.execute(
            "UPDATE watch_history
             SET total_duration_seconds = total_duration_seconds + ?1
             WHERE channel_id = ?2",
            params![duration_seconds, channel_id],
        )?;
        Ok(())
    }

    /// Return watch history entries ordered by `last_watched_at DESC`, limited to `limit`.
    pub fn get_watch_history(&self, limit: usize) -> Result<Vec<WatchHistoryEntry>, CacheError> {
        let mut stmt = self.conn.prepare(
            "SELECT channel_id, channel_name, channel_logo, content_type,
                    first_watched_at, last_watched_at, total_duration_seconds, play_count
             FROM watch_history
             ORDER BY last_watched_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
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

    /// Delete a single watch history entry by channel ID.
    pub fn delete_history_entry(&self, channel_id: &str) -> Result<(), CacheError> {
        self.conn.execute(
            "DELETE FROM watch_history WHERE channel_id = ?1",
            params![channel_id],
        )?;
        Ok(())
    }

    /// Delete all watch history entries.
    pub fn clear_watch_history(&self) -> Result<(), CacheError> {
        self.conn.execute("DELETE FROM watch_history", [])?;
        Ok(())
    }
}

/// For series channels loaded from an older cache that has NULL series_title/season/episode,
/// re-parse the channel name in memory so the frontend always gets enriched metadata.
/// Does not modify the database — the next playlist refresh will persist the correct values.
fn enrich_stale_series(channels: &mut Vec<Channel>) {
    for ch in channels.iter_mut() {
        if ch.content_type != "series" {
            continue;
        }
        if ch.series_title.is_none() || ch.season.is_none() || ch.episode.is_none() {
            if let Some((title, season, episode)) = parse_series_name(&ch.name) {
                ch.series_title = Some(title);
                ch.season = Some(season);
                ch.episode = Some(episode);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_crud() {
        let store = CacheStore::open_in_memory().unwrap();
        let provider = Provider {
            id: "p1".into(),
            name: "Test".into(),
            provider_type: ProviderType::M3u,
            url: "http://example.com/playlist.m3u".into(),
            username: None,
            password: None,
            last_updated: None,
            channel_count: 0,
            epg_url: None,
        };
        store.upsert_provider(&provider).unwrap();
        let providers = store.get_providers().unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name, "Test");

        store.remove_provider("p1").unwrap();
        let providers = store.get_providers().unwrap();
        assert!(providers.is_empty());
    }

    fn make_provider(id: &str, name: &str) -> Provider {
        Provider {
            id: id.into(),
            name: name.into(),
            provider_type: ProviderType::M3u,
            url: format!("http://example.com/{id}.m3u"),
            username: None,
            password: None,
            last_updated: None,
            channel_count: 0,
            epg_url: None,
        }
    }

    fn make_channel(id: &str, name: &str, group: &str) -> Channel {
        Channel {
            id: id.into(),
            name: name.into(),
            url: format!("http://stream.example.com/{id}"),
            logo_url: None,
            group_title: group.into(),
            tvg_id: None,
            tvg_name: None,
            is_favorite: false,
            content_type: "live".into(),
            sources: Vec::new(),
            series_title: None,
            season: None,
            episode: None,
        }
    }

    #[test]
    fn test_channel_storage() {
        let store = CacheStore::open_in_memory().unwrap();
        store.upsert_provider(&make_provider("p1", "Test")).unwrap();

        let channels = vec![
            make_channel("ch1", "News", "News"),
            make_channel("ch2", "Sports", "Sports"),
        ];
        store.save_channels("p1", &channels).unwrap();
        let loaded = store.get_channels("p1").unwrap();
        assert_eq!(loaded.len(), 2);
    }

    // --- Edge cases ---

    #[test]
    fn test_upsert_provider_updates_existing() {
        let store = CacheStore::open_in_memory().unwrap();
        store.upsert_provider(&make_provider("p1", "Original")).unwrap();

        let updated = Provider {
            name: "Updated".into(),
            channel_count: 42,
            ..make_provider("p1", "Updated")
        };
        store.upsert_provider(&updated).unwrap();

        let providers = store.get_providers().unwrap();
        assert_eq!(providers.len(), 1, "upsert must not duplicate");
        assert_eq!(providers[0].name, "Updated");
        assert_eq!(providers[0].channel_count, 42);
    }

    #[test]
    fn test_save_channels_replaces_previous_batch() {
        let store = CacheStore::open_in_memory().unwrap();
        store.upsert_provider(&make_provider("p1", "P")).unwrap();

        store.save_channels("p1", &[make_channel("ch1", "Old", "G")]).unwrap();
        // Second save should wipe ch1 and only keep the new channels
        store
            .save_channels("p1", &[make_channel("ch2", "New A", "G"), make_channel("ch3", "New B", "G")])
            .unwrap();

        let loaded = store.get_channels("p1").unwrap();
        assert_eq!(loaded.len(), 2);
        assert!(!loaded.iter().any(|c| c.id == "ch1"), "old channel must be gone");
        assert!(loaded.iter().any(|c| c.id == "ch2"));
        assert!(loaded.iter().any(|c| c.id == "ch3"));
    }

    #[test]
    fn test_remove_provider_cascades_channels() {
        let store = CacheStore::open_in_memory().unwrap();
        store.upsert_provider(&make_provider("p1", "P")).unwrap();
        store
            .save_channels("p1", &[make_channel("ch1", "Ch", "G"), make_channel("ch2", "Ch2", "G")])
            .unwrap();

        store.remove_provider("p1").unwrap();

        assert!(store.get_providers().unwrap().is_empty());
        assert!(store.get_all_channels().unwrap().is_empty(), "cascade delete must remove channels");
    }

    #[test]
    fn test_toggle_favorite_roundtrip() {
        let store = CacheStore::open_in_memory().unwrap();
        store.upsert_provider(&make_provider("p1", "P")).unwrap();
        store.save_channels("p1", &[make_channel("ch1", "Ch", "G")]).unwrap();

        let was_fav = store.toggle_favorite("ch1").unwrap();
        assert!(was_fav, "first toggle → favorite");

        let back = store.toggle_favorite("ch1").unwrap();
        assert!(!back, "second toggle → not favorite");

        let ch = &store.get_channels("p1").unwrap()[0];
        assert!(!ch.is_favorite, "persisted state must match");
    }

    #[test]
    fn test_get_all_channels_across_providers() {
        let store = CacheStore::open_in_memory().unwrap();
        store.upsert_provider(&make_provider("p1", "Provider 1")).unwrap();
        store.upsert_provider(&make_provider("p2", "Provider 2")).unwrap();
        store.save_channels("p1", &[make_channel("ch1", "A", "G"), make_channel("ch2", "B", "G")]).unwrap();
        store.save_channels("p2", &[make_channel("ch3", "C", "G")]).unwrap();

        let all = store.get_all_channels().unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_get_channels_only_returns_own_provider() {
        let store = CacheStore::open_in_memory().unwrap();
        store.upsert_provider(&make_provider("p1", "P1")).unwrap();
        store.upsert_provider(&make_provider("p2", "P2")).unwrap();
        store.save_channels("p1", &[make_channel("ch1", "P1 Ch", "G")]).unwrap();
        store.save_channels("p2", &[make_channel("ch2", "P2 Ch", "G")]).unwrap();

        let p1_channels = store.get_channels("p1").unwrap();
        assert_eq!(p1_channels.len(), 1);
        assert_eq!(p1_channels[0].id, "ch1");
    }

    #[test]
    fn test_epg_save_and_retrieve() {
        let store = CacheStore::open_in_memory().unwrap();
        let json = r#"{"title":"Morning News"}"#;
        store.save_epg_data("ch1", json).unwrap();
        let result = store.get_epg_data("ch1").unwrap();
        assert_eq!(result.as_deref(), Some(json));
    }

    #[test]
    fn test_epg_returns_none_for_unknown_channel() {
        let store = CacheStore::open_in_memory().unwrap();
        let result = store.get_epg_data("nonexistent").unwrap();
        assert!(result.is_none());
    }

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
        let provider = Provider {
            id: "p1".into(),
            name: "Test".into(),
            provider_type: crate::models::playlist::ProviderType::M3u,
            url: "http://example.com/playlist.m3u".into(),
            username: None,
            password: None,
            last_updated: None,
            channel_count: 0,
            epg_url: Some("http://example.com/epg.xml".into()),
        };
        store.upsert_provider(&provider).unwrap();
        let providers = store.get_providers().unwrap();
        assert_eq!(providers[0].epg_url.as_deref(), Some("http://example.com/epg.xml"));
    }

    #[test]
    fn test_xtream_provider_roundtrip() {
        let store = CacheStore::open_in_memory().unwrap();
        let provider = Provider {
            id: "x1".into(),
            name: "Xtream Test".into(),
            provider_type: ProviderType::Xtream,
            url: "http://xtream.example.com".into(),
            username: Some("user".into()),
            password: Some("pass".into()),
            last_updated: Some("2026-01-01".into()),
            channel_count: 500,
            epg_url: None,
        };
        store.upsert_provider(&provider).unwrap();
        let loaded = store.get_providers().unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(matches!(loaded[0].provider_type, ProviderType::Xtream));
        assert_eq!(loaded[0].username.as_deref(), Some("user"));
        assert_eq!(loaded[0].password.as_deref(), Some("pass"));
        assert_eq!(loaded[0].channel_count, 500);
    }

    #[test]
    fn test_stale_series_enriched_on_read() {
        // Simulate a channel stored with content_type="series" but NULL series_title/season/episode
        // (as would happen with data cached before those columns were added).
        let store = CacheStore::open_in_memory().unwrap();
        store.upsert_provider(&make_provider("p1", "P")).unwrap();

        let stale = Channel {
            id: "ch1".into(),
            name: "Suits LA S01E10".into(),
            url: "http://example.com/series/x/y/ep.mp4".into(),
            logo_url: None,
            group_title: "Server 2".into(),
            tvg_id: None,
            tvg_name: None,
            is_favorite: false,
            content_type: "series".into(),
            sources: Vec::new(),
            series_title: None,   // stale — not yet parsed
            season: None,
            episode: None,
        };
        store.save_channels("p1", &[stale]).unwrap();

        let channels = store.get_channels("p1").unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].series_title.as_deref(), Some("Suits LA"));
        assert_eq!(channels[0].season, Some(1));
        assert_eq!(channels[0].episode, Some(10));
    }

    #[test]
    fn test_stale_series_enriched_in_get_all() {
        let store = CacheStore::open_in_memory().unwrap();
        store.upsert_provider(&make_provider("p1", "P")).unwrap();

        let stale = Channel {
            id: "ch2".into(),
            name: "Breaking Bad S03E07".into(),
            url: "http://example.com/series/bb/S03E07.ts".into(),
            logo_url: None,
            group_title: "Drama".into(),
            tvg_id: None,
            tvg_name: None,
            is_favorite: false,
            content_type: "series".into(),
            sources: Vec::new(),
            series_title: None,
            season: None,
            episode: None,
        };
        store.save_channels("p1", &[stale]).unwrap();

        let channels = store.get_all_channels().unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].series_title.as_deref(), Some("Breaking Bad"));
        assert_eq!(channels[0].season, Some(3));
        assert_eq!(channels[0].episode, Some(7));
    }

    fn make_omdb_data(title: &str) -> OmdbData {
        OmdbData {
            title: title.into(),
            year: Some("2008".into()),
            rated: Some("PG-13".into()),
            runtime: Some("152 min".into()),
            genre: Some("Action".into()),
            director: Some("Christopher Nolan".into()),
            actors: Some("Christian Bale".into()),
            plot: Some("A movie plot.".into()),
            poster_url: Some("https://example.com/poster.jpg".into()),
            imdb_rating: Some("9.0".into()),
            rotten_tomatoes: Some("94%".into()),
        }
    }

    #[test]
    fn test_save_and_retrieve_omdb_cache() {
        let store = CacheStore::open_in_memory().unwrap();
        let data = make_omdb_data("The Dark Knight");

        store.save_omdb_cache("ch1", &data).unwrap();

        // TTL of 30 days in seconds; fresh data should be returned
        let ttl = 30 * 24 * 60 * 60;
        let result = store.get_omdb_cache("ch1", ttl).unwrap();
        assert!(result.is_some());
        let cached = result.unwrap();
        assert_eq!(cached.title, "The Dark Knight");
        assert_eq!(cached.imdb_rating.as_deref(), Some("9.0"));
        assert_eq!(cached.rotten_tomatoes.as_deref(), Some("94%"));
    }

    #[test]
    fn test_get_omdb_cache_returns_none_for_unknown_channel() {
        let store = CacheStore::open_in_memory().unwrap();
        let result = store.get_omdb_cache("nonexistent", 30 * 24 * 60 * 60).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_omdb_cache_respects_ttl() {
        let store = CacheStore::open_in_memory().unwrap();
        let data = make_omdb_data("Old Movie");

        store.save_omdb_cache("ch1", &data).unwrap();

        // Use TTL of -1 so the entry is always considered stale
        let result = store.get_omdb_cache("ch1", -1).unwrap();
        assert!(result.is_none(), "stale cache entry should return None");
    }

    #[test]
    fn test_save_omdb_cache_overwrites_existing() {
        let store = CacheStore::open_in_memory().unwrap();

        let first = make_omdb_data("First Movie");
        store.save_omdb_cache("ch1", &first).unwrap();

        let second = OmdbData {
            title: "Second Movie".into(),
            year: None,
            rated: None,
            runtime: None,
            genre: None,
            director: None,
            actors: None,
            plot: None,
            poster_url: None,
            imdb_rating: None,
            rotten_tomatoes: None,
        };
        store.save_omdb_cache("ch1", &second).unwrap();

        let ttl = 30 * 24 * 60 * 60;
        let result = store.get_omdb_cache("ch1", ttl).unwrap().unwrap();
        assert_eq!(result.title, "Second Movie", "second save must overwrite first");
    }

    #[test]
    fn test_omdb_cache_preserves_none_fields() {
        let store = CacheStore::open_in_memory().unwrap();
        let data = OmdbData {
            title: "Minimal Movie".into(),
            year: None,
            rated: None,
            runtime: None,
            genre: None,
            director: None,
            actors: None,
            plot: None,
            poster_url: None,
            imdb_rating: None,
            rotten_tomatoes: None,
        };
        store.save_omdb_cache("ch2", &data).unwrap();

        let ttl = 30 * 24 * 60 * 60;
        let result = store.get_omdb_cache("ch2", ttl).unwrap().unwrap();
        assert_eq!(result.title, "Minimal Movie");
        assert!(result.year.is_none());
        assert!(result.rotten_tomatoes.is_none());
    }

    // --- Watch History Tests ---

    #[test]
    fn test_record_play_start_creates_entry() {
        let store = CacheStore::open_in_memory().unwrap();
        store
            .record_play_start("ch1", "BBC News", Some("http://logo.example.com/bbc.png"), "live")
            .unwrap();

        let history = store.get_watch_history(10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].channel_id, "ch1");
        assert_eq!(history[0].channel_name, "BBC News");
        assert_eq!(history[0].play_count, 1);
        assert_eq!(history[0].total_duration_seconds, 0);
    }

    #[test]
    fn test_record_play_start_increments_play_count() {
        let store = CacheStore::open_in_memory().unwrap();
        store
            .record_play_start("ch1", "BBC News", None, "live")
            .unwrap();
        let first_entry = store.get_watch_history(10).unwrap();
        let first_watched_at = first_entry[0].first_watched_at;

        // Second call should increment play_count but not change first_watched_at
        store
            .record_play_start("ch1", "BBC News", None, "live")
            .unwrap();

        let history = store.get_watch_history(10).unwrap();
        assert_eq!(history.len(), 1, "must not create duplicate row");
        assert_eq!(history[0].play_count, 2);
        assert_eq!(
            history[0].first_watched_at, first_watched_at,
            "first_watched_at must not change on upsert"
        );
    }

    #[test]
    fn test_record_play_end_accumulates_duration() {
        let store = CacheStore::open_in_memory().unwrap();
        store
            .record_play_start("ch1", "Movie Channel", None, "movie")
            .unwrap();
        store.record_play_end("ch1", 30).unwrap();
        store.record_play_end("ch1", 45).unwrap();

        let history = store.get_watch_history(10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].total_duration_seconds, 75);
    }

    #[test]
    fn test_delete_history_entry() {
        let store = CacheStore::open_in_memory().unwrap();
        store
            .record_play_start("ch1", "Channel 1", None, "live")
            .unwrap();
        store
            .record_play_start("ch2", "Channel 2", None, "live")
            .unwrap();

        store.delete_history_entry("ch1").unwrap();

        let history = store.get_watch_history(10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].channel_id, "ch2");
    }

    #[test]
    fn test_clear_watch_history() {
        let store = CacheStore::open_in_memory().unwrap();
        store
            .record_play_start("ch1", "Channel 1", None, "live")
            .unwrap();
        store
            .record_play_start("ch2", "Channel 2", None, "live")
            .unwrap();
        store
            .record_play_start("ch3", "Channel 3", None, "movie")
            .unwrap();

        store.clear_watch_history().unwrap();

        let history = store.get_watch_history(10).unwrap();
        assert!(history.is_empty(), "history must be empty after clear");
    }
}
