use crate::models::channel::Channel;
use crate::models::playlist::{Provider, ProviderType};
use rusqlite::{params, Connection, Result as SqlResult};
use std::path::Path;
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
        Ok(())
    }

    // --- Providers ---

    pub fn upsert_provider(&self, provider: &Provider) -> Result<(), CacheError> {
        let ptype = match provider.provider_type {
            ProviderType::M3u => "m3u",
            ProviderType::Xtream => "xtream",
        };
        self.conn.execute(
            "INSERT OR REPLACE INTO providers (id, name, provider_type, url, username, password, last_updated, channel_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                provider.id,
                provider.name,
                ptype,
                provider.url,
                provider.username,
                provider.password,
                provider.last_updated,
                provider.channel_count,
            ],
        )?;
        Ok(())
    }

    pub fn get_providers(&self) -> Result<Vec<Provider>, CacheError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, provider_type, url, username, password, last_updated, channel_count FROM providers"
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
            })
        })?.collect::<SqlResult<Vec<_>>>()?;
        Ok(providers)
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
            "INSERT INTO channels (id, provider_id, name, url, logo_url, group_title, tvg_id, tvg_name, is_favorite)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
        )?;

        for ch in channels {
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
            ])?;
        }
        Ok(())
    }

    pub fn get_channels(&self, provider_id: &str) -> Result<Vec<Channel>, CacheError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, url, logo_url, group_title, tvg_id, tvg_name, is_favorite
             FROM channels WHERE provider_id = ?1 ORDER BY name"
        )?;
        let channels = stmt.query_map(params![provider_id], |row| {
            Ok(Channel {
                id: row.get(0)?,
                name: row.get(1)?,
                url: row.get(2)?,
                logo_url: row.get(3)?,
                group_title: row.get(4)?,
                tvg_id: row.get(5)?,
                tvg_name: row.get(6)?,
                is_favorite: row.get::<_, i32>(7)? != 0,
            })
        })?.collect::<SqlResult<Vec<_>>>()?;
        Ok(channels)
    }

    pub fn get_all_channels(&self) -> Result<Vec<Channel>, CacheError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, url, logo_url, group_title, tvg_id, tvg_name, is_favorite
             FROM channels ORDER BY name"
        )?;
        let channels = stmt.query_map([], |row| {
            Ok(Channel {
                id: row.get(0)?,
                name: row.get(1)?,
                url: row.get(2)?,
                logo_url: row.get(3)?,
                group_title: row.get(4)?,
                tvg_id: row.get(5)?,
                tvg_name: row.get(6)?,
                is_favorite: row.get::<_, i32>(7)? != 0,
            })
        })?.collect::<SqlResult<Vec<_>>>()?;
        Ok(channels)
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
        };
        store.upsert_provider(&provider).unwrap();
        let providers = store.get_providers().unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name, "Test");

        store.remove_provider("p1").unwrap();
        let providers = store.get_providers().unwrap();
        assert!(providers.is_empty());
    }

    #[test]
    fn test_channel_storage() {
        let store = CacheStore::open_in_memory().unwrap();
        let provider = Provider {
            id: "p1".into(),
            name: "Test".into(),
            provider_type: ProviderType::M3u,
            url: "http://test.m3u".into(),
            username: None,
            password: None,
            last_updated: None,
            channel_count: 2,
        };
        store.upsert_provider(&provider).unwrap();

        let channels = vec![
            Channel {
                id: "ch1".into(),
                name: "News".into(),
                url: "http://news".into(),
                logo_url: None,
                group_title: "News".into(),
                tvg_id: None,
                tvg_name: None,
                is_favorite: false,
            },
            Channel {
                id: "ch2".into(),
                name: "Sports".into(),
                url: "http://sports".into(),
                logo_url: None,
                group_title: "Sports".into(),
                tvg_id: None,
                tvg_name: None,
                is_favorite: false,
            },
        ];
        store.save_channels("p1", &channels).unwrap();
        let loaded = store.get_channels("p1").unwrap();
        assert_eq!(loaded.len(), 2);
    }
}
