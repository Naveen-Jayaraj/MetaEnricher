use crate::services::DatabaseService;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UptimeLogEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub timestamp: String,
    pub service: String,
    pub render_status: String,
    pub render_uptime_seconds: u64,
    pub neon_status: String,
    pub neon_latency_ms: Option<u32>,
    pub neon_metadata_count: u64,
    pub neon_lyrics_count: u64,
    pub event_type: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UptimeSummary {
    pub render_status: String,
    pub render_uptime_seconds: u64,
    pub render_uptime_formatted: String,
    pub neon_status: String,
    pub neon_latency_ms: u32,
    pub neon_metadata_cached: u64,
    pub neon_lyrics_cached: u64,
    pub local_logs_count: usize,
    pub database_logs_count: u64,
    pub total_cache_purges: u64,
    pub data_policy: &'static str,
}

pub struct UptimeManager {
    local_logs: Arc<RwLock<VecDeque<UptimeLogEntry>>>,
    start_time: Instant,
    last_neon_latency_ms: Arc<AtomicU64>,
    total_purges: Arc<AtomicU64>,
    db: Option<Arc<DatabaseService>>,
}

impl UptimeManager {
    pub fn new(start_time: Instant, db: Option<Arc<DatabaseService>>) -> Self {
        Self {
            local_logs: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            start_time,
            last_neon_latency_ms: Arc::new(AtomicU64::new(0)),
            total_purges: Arc::new(AtomicU64::new(0)),
            db,
        }
    }

    /// Load recent history from Neon DB into local buffer so Render has past records even after a restart.
    pub async fn init_from_database(&self) {
        if let Some(db) = &self.db {
            if let Ok(db_logs) = db.get_recent_uptime_logs(100).await {
                if !db_logs.is_empty() {
                    let mut lock = self.local_logs.write().await;
                    for entry in db_logs.into_iter().rev() {
                        if lock.len() >= 1000 {
                            lock.pop_front();
                        }
                        lock.push_back(entry);
                    }
                    info!("Loaded historical uptime logs from Neon DB into Render local memory.");
                }
            }
        }
    }

    /// Record a log entry in BOTH Render local memory and Neon DB.
    pub async fn record(&self, mut entry: UptimeLogEntry) {
        if entry.timestamp.is_empty() {
            entry.timestamp = Utc::now().to_rfc3339();
        }

        // 1. Store in Render local circular buffer (survives Neon outages)
        {
            let mut lock = self.local_logs.write().await;
            if lock.len() >= 1000 {
                lock.pop_front();
            }
            lock.push_back(entry.clone());
        }

        // 2. Store in Neon DB (survives Render restarts)
        if let Some(db) = &self.db {
            let db = db.clone();
            tokio::spawn(async move {
                if let Err(e) = db.save_uptime_log(&entry).await {
                    warn!("Failed to persist uptime log to Neon DB: {e}");
                }
            });
        }
    }

    /// Perform a heartbeat check on both Render and Neon DB.
    pub async fn heartbeat(&self) {
        let uptime = self.start_time.elapsed().as_secs();
        let (neon_status, latency_ms, meta_count, lyrics_count) = if let Some(db) = &self.db {
            match db.ping_latency().await {
                Ok(ms) => {
                    self.last_neon_latency_ms.store(ms as u64, Ordering::Relaxed);
                    let (m, l) = db.cached_counts();
                    ("ONLINE".to_string(), Some(ms), m, l)
                }
                Err(e) => {
                    warn!("Neon DB heartbeat ping failed: {e}");
                    ("OFFLINE".to_string(), None, 0, 0)
                }
            }
        } else {
            ("OFFLINE (No DB Configured)".to_string(), None, 0, 0)
        };

        let message = if neon_status == "ONLINE" {
            format!("Heartbeat OK. Render Uptime: {}s, Neon Latency: {}ms", uptime, latency_ms.unwrap_or(0))
        } else {
            format!("Heartbeat Warning: Neon DB is currently unreachable from Render.")
        };

        let entry = UptimeLogEntry {
            id: None,
            timestamp: Utc::now().to_rfc3339(),
            service: "system".to_string(),
            render_status: "ONLINE".to_string(),
            render_uptime_seconds: uptime,
            neon_status,
            neon_latency_ms: latency_ms,
            neon_metadata_count: meta_count,
            neon_lyrics_count: lyrics_count,
            event_type: "HEARTBEAT".to_string(),
            message,
        };

        self.record(entry).await;
    }

    /// Record a memory purge event (when Render cleans in-memory metadata to keep only in Neon).
    pub async fn record_purge(&self, purged_meta: u64, purged_lyrics: u64) {
        self.total_purges.fetch_add(1, Ordering::Relaxed);
        let uptime = self.start_time.elapsed().as_secs();
        let (meta_count, lyrics_count) = if let Some(db) = &self.db {
            db.cached_counts()
        } else {
            (0, 0)
        };

        let entry = UptimeLogEntry {
            id: None,
            timestamp: Utc::now().to_rfc3339(),
            service: "render".to_string(),
            render_status: "ONLINE".to_string(),
            render_uptime_seconds: uptime,
            neon_status: "ONLINE".to_string(),
            neon_latency_ms: Some(self.last_neon_latency_ms.load(Ordering::Relaxed) as u32),
            neon_metadata_count: meta_count,
            neon_lyrics_count: lyrics_count,
            event_type: "CACHE_PURGE".to_string(),
            message: format!(
                "Periodic Render memory cleanup: Purged {} metadata & {} lyrics entries from memory. Authoritative metadata stored exclusively in Neon DB.",
                purged_meta, purged_lyrics
            ),
        };

        self.record(entry).await;
    }

    /// Retrieve the most recent uptime logs from local buffer (and Neon DB if buffer is small).
    pub async fn get_logs(&self, limit: usize) -> Vec<UptimeLogEntry> {
        let lock = self.local_logs.read().await;
        if lock.len() >= limit || self.db.is_none() {
            return lock.iter().rev().take(limit).cloned().collect();
        }

        // If local logs has fewer entries than requested, fetch more from Neon
        if let Some(db) = &self.db {
            if let Ok(db_logs) = db.get_recent_uptime_logs(limit as i64).await {
                if !db_logs.is_empty() {
                    return db_logs;
                }
            }
        }

        lock.iter().rev().take(limit).cloned().collect()
    }

    /// Generate an overall summary of system health and dual storage redundancy.
    pub async fn get_summary(&self) -> UptimeSummary {
        let uptime_secs = self.start_time.elapsed().as_secs();
        let (meta_count, lyrics_count, db_logs_count) = if let Some(db) = &self.db {
            let (m, l) = db.cached_counts();
            let logs_cnt = db.uptime_logs_count().await;
            (m, l, logs_cnt)
        } else {
            (0, 0, 0)
        };

        let local_cnt = self.local_logs.read().await.len();
        let latency = self.last_neon_latency_ms.load(Ordering::Relaxed) as u32;
        let neon_status = if self.db.is_some() && (latency > 0 || meta_count > 0) {
            "ONLINE".to_string()
        } else if self.db.is_some() {
            "CONNECTED".to_string()
        } else {
            "OFFLINE".to_string()
        };

        UptimeSummary {
            render_status: "ONLINE".to_string(),
            render_uptime_seconds: uptime_secs,
            render_uptime_formatted: format_uptime(uptime_secs),
            neon_status,
            neon_latency_ms: latency,
            neon_metadata_cached: meta_count,
            neon_lyrics_cached: lyrics_count,
            local_logs_count: local_cnt,
            database_logs_count: db_logs_count,
            total_cache_purges: self.total_purges.load(Ordering::Relaxed),
            data_policy: "Zero persistent metadata on Render (auto-purged). Persistent metadata & lyrics held exclusively in Neon DB.",
        }
    }
}

pub fn format_uptime(seconds: u64) -> String {
    let d = seconds / 86400;
    let h = (seconds % 86400) / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;

    if d > 0 {
        format!("{d}d {h}h {m}m {s}s")
    } else if h > 0 {
        format!("{h}h {m}m {s}s")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}
