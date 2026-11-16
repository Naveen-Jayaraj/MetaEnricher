mod lrclib;
mod netease;

use crate::models::{LyricQuery, LyricResponse};
use crate::services::matching::{Candidate, Recording, SearchQuery, Source};
use crate::services::DatabaseService;
use lrclib::LrclibService;
use moka::future::Cache;
use netease::NetEaseLyricsService;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub(crate) struct LyricMatch {
    pub candidate: Candidate,
    pub synced: Option<String>,
    pub plain: Option<String>,
    pub instrumental: bool,
}

impl LyricMatch {
    pub fn has_lyrics(&self) -> bool {
        self.synced.is_some() || self.plain.is_some()
    }

    pub fn id(&self) -> String {
        let source = self.candidate.source.name().to_ascii_lowercase();
        let kind = if self.synced.is_some() { "lrc" } else { "text" };
        format!("{source}-{}-{kind}", self.candidate.id)
    }

    pub fn uri(&self) -> Option<String> {
        self.has_lyrics()
            .then(|| format!("/lyric/file?id={}", self.id()))
    }

    pub fn new_cached(
        source_name: &str,
        id: &str,
        title: &str,
        artist: &str,
        synced: Option<String>,
        plain: Option<String>,
        instrumental: bool,
    ) -> Self {
        let source = match source_name.to_lowercase().as_str() {
            "netease" => Source::NetEase,
            _ => Source::Lrclib,
        };
        let recording = Recording {
            title: title.to_string(),
            artists: vec![artist.to_string()],
            ..Default::default()
        };
        let candidate = Candidate {
            source,
            id: id.to_string(),
            release_id: None,
            recording,
        };
        Self {
            candidate,
            synced,
            plain,
            instrumental,
        }
    }
}

pub struct LyricsAggregator {
    lrclib: LrclibService,
    netease: NetEaseLyricsService,
    cache: Cache<SearchQuery, Arc<LyricMatch>>,
    by_id: Cache<String, Arc<LyricMatch>>,
    db: Option<Arc<DatabaseService>>,
}

impl LyricsAggregator {
    pub fn new(db: Option<Arc<DatabaseService>>) -> Self {
        let client = super::http_client();
        Self {
            lrclib: LrclibService::new(client.clone()),
            netease: NetEaseLyricsService::new(client),
            cache: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(Duration::from_secs(120))
                .build(),
            by_id: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(Duration::from_secs(120))
                .build(),
            db,
        }
    }

    pub(super) async fn find_lrclib(&self, query: &SearchQuery) -> Option<LyricMatch> {
        self.lrclib.find(query).await
    }

    pub(super) async fn find_netease(&self, query: &SearchQuery) -> Option<LyricMatch> {
        self.netease.find(query).await
    }

    pub(super) async fn remember(&self, query: &SearchQuery, hit: Arc<LyricMatch>) {
        if hit.has_lyrics() {
            self.by_id.insert(hit.id(), Arc::clone(&hit)).await;
            self.cache.insert(query.key(), hit.clone()).await;

            if let Some(db) = &self.db {
                let db = db.clone();
                let query = query.clone();
                let hit = hit.clone();
                tokio::spawn(async move {
                    let _ = db.save_lyrics(&query, &hit).await;
                });
            }
        }
    }

    pub async fn get_lyrics(&self, query: &LyricQuery) -> LyricResponse {
        let signature = SearchQuery::new(
            &query.title,
            &query.artist,
            query.album.as_deref(),
            None,
            query.duration_ms.unwrap_or_default(),
        );
        let found = if signature.title.is_empty() {
            None
        } else {
            // 1. Check in-memory cache
            if let Some(hit) = self.cache.get(&signature.key()).await {
                Some(hit)
            } else {
                // 2. Check Neon PostgreSQL persistent cache
                let db_hit = if let Some(db) = &self.db {
                    db.get_lyrics(&signature).await.map(Arc::new)
                } else {
                    None
                };

                if let Some(hit) = db_hit {
                    self.cache.insert(signature.key(), hit.clone()).await;
                    Some(hit)
                } else {
                    // 3. Query LRCLIB and NetEase
                    let api_hit = if let Some(hit) = self.find_lrclib(&signature).await {
                        if hit.has_lyrics() || hit.instrumental {
                            Some(Arc::new(hit))
                        } else {
                            self.find_netease(&signature).await.map(Arc::new)
                        }
                    } else {
                        self.find_netease(&signature).await.map(Arc::new)
                    };

                    if let Some(hit) = &api_hit {
                        if hit.has_lyrics() || hit.instrumental {
                            self.cache.insert(signature.key(), hit.clone()).await;
                            if let Some(db) = &self.db {
                                let db = db.clone();
                                let sig = signature.clone();
                                let h = hit.clone();
                                tokio::spawn(async move {
                                    let _ = db.save_lyrics(&sig, &h).await;
                                });
                            }
                        }
                    }
                    api_hit
                }
            }
        };
        let mut response = LyricResponse {
            id: query.id.clone().unwrap_or_else(|| {
                found
                    .as_ref()
                    .map(|hit| hit.id())
                    .unwrap_or_else(|| format!("{:x}", chrono::Utc::now().timestamp_micros()))
            }),
            title: query.title.clone(),
            artist: query.artist.clone(),
            lyrics_available: false,
            has_synced: false,
            synced_lyrics: None,
            plain_lyrics: None,
            source: "None".to_string(),
            lines_count: 0,
            lyrics_uri: None,
        };

        if let Some(hit) = found {
            response.lyrics_available = hit.has_lyrics();
            response.has_synced = hit.synced.is_some();
            response.synced_lyrics = hit.synced.clone();
            response.plain_lyrics = hit.plain.clone();
            response.source = hit.candidate.source.name().to_string();
            response.lines_count = hit
                .synced
                .as_deref()
                .or(hit.plain.as_deref())
                .map_or(0, |s| s.lines().count());
            response.lyrics_uri = hit.uri();
            if hit.has_lyrics() {
                self.by_id.insert(hit.id(), hit).await;
            }
        }
        response
    }

    pub async fn get_raw_lrc(&self, id: &str, title: &str, artist: &str) -> Option<String> {
        if let Some(hit) = self.by_id.get(id).await {
            return hit.synced.clone().or_else(|| hit.plain.clone());
        }
        if title.trim().is_empty() {
            return None;
        }
        let res = self
            .get_lyrics(&LyricQuery {
                id: Some(id.to_string()),
                title: title.to_string(),
                artist: artist.to_string(),
                album: None,
                duration_ms: None,
            })
            .await;
        res.synced_lyrics.or(res.plain_lyrics)
    }

    pub fn cache_size(&self) -> u64 {
        self.cache.entry_count()
    }

    /// Purges all in-memory lyrics entries from Render. Persistent lyrics remain in Neon DB.
    pub async fn purge_memory_cache(&self) -> (u64, u64) {
        let c1 = self.cache.entry_count();
        let c2 = self.by_id.entry_count();
        self.cache.invalidate_all();
        self.by_id.invalidate_all();
        (c1, c2)
    }
}
