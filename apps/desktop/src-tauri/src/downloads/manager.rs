use crate::commands::AppState;
use crate::downloads::scheduler::ids_to_start;
use mvp_core::downloads::model::{DownloadKind, DownloadRecord, DownloadStatus};
use mvp_core::downloads::paths::derive_dest_path;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, Runtime};
use tokio::process::Child;

/// Per-item runtime handle to a running ffmpeg child.
pub struct RunningChild {
    pub child: Child,
}

/// Tauri managed state. Holds config + the live child-process map.
/// All persisted state lives in `CacheStore` (via `AppState`); this struct only
/// holds what cannot be serialized (the child handles) plus mutable config.
pub struct DownloadManager {
    pub concurrency: Mutex<usize>,
    pub root: Mutex<PathBuf>,
    pub running: Mutex<HashMap<String, RunningChild>>,
}

impl DownloadManager {
    pub fn new(default_root: PathBuf) -> Self {
        Self {
            concurrency: Mutex::new(3),
            root: Mutex::new(default_root),
            running: Mutex::new(HashMap::new()),
        }
    }

    pub fn root(&self) -> PathBuf {
        self.root.lock().unwrap().clone()
    }

    pub fn set_root(&self, p: PathBuf) {
        *self.root.lock().unwrap() = p;
    }

    pub fn concurrency(&self) -> usize {
        *self.concurrency.lock().unwrap()
    }

    pub fn set_concurrency(&self, n: usize) {
        *self.concurrency.lock().unwrap() = n.max(1);
    }
}

/// Create a queued DownloadRecord for a channel and persist it. Returns the id.
/// `series_channel_id` is set for episodes so aggregate state can be computed.
pub fn enqueue_record<R: Runtime>(
    app: &AppHandle<R>,
    channel_id: &str,
    title: &str,
    url: &str,
    kind: DownloadKind,
    series_channel_id: Option<String>,
    series_title: Option<&str>,
) -> Result<String, String> {
    let app_state = app.state::<AppState>();
    let manager = app.state::<DownloadManager>();
    let root = manager.root();
    let dest = derive_dest_path(&root, kind, title, series_title, url);

    let id = format!("dl-{}", uuid_like());
    let now = now_secs();
    let rec = DownloadRecord {
        id: id.clone(),
        channel_id: channel_id.to_string(),
        title: title.to_string(),
        kind,
        series_channel_id,
        status: DownloadStatus::Queued,
        dest_path: format!("{}.part", dest.to_string_lossy()),
        total_bytes: None,
        downloaded_bytes: 0,
        avg_rate_bps: None,
        error: None,
        created_at: now,
        finished_at: None,
    };
    {
        let cache = app_state.cache.lock().map_err(|e| e.to_string())?;
        cache.upsert_download(&rec).map_err(|e| e.to_string())?;
    }
    Ok(id)
}

/// Inspect the active set and start ffmpeg for any items the scheduler selects.
pub fn pump<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let app_state = app.state::<AppState>();
    let manager = app.state::<DownloadManager>();
    let limit = manager.concurrency();

    let active = {
        let cache = app_state.cache.lock().map_err(|e| e.to_string())?;
        cache.list_active_downloads().map_err(|e| e.to_string())?
    };
    let pairs: Vec<(String, DownloadStatus)> =
        active.iter().map(|r| (r.id.clone(), r.status)).collect();
    let start_ids = ids_to_start(&pairs, limit);

    for id in start_ids {
        if let Some(rec) = active.iter().find(|r| r.id == id) {
            start_process(app, rec.clone())?;
        }
    }
    Ok(())
}

pub(crate) fn now_secs() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Lightweight unique id without adding a uuid dependency.
fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}")
}

// TEMPORARY stub — replaced by the real implementation in Task 10.
pub fn start_process<R: Runtime>(_app: &AppHandle<R>, _rec: DownloadRecord) -> Result<(), String> {
    Ok(())
}
