mod lrclib;
mod netease;

use crate::models::{LyricQuery, LyricResponse};
use crate::services::matching::{Candidate, SearchQuery};
use lrclib::LrclibService;
use moka::future::Cache;
use netease::NetEaseLyricsService;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub(super) struct LyricMatch {
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
}

pub struct LyricsAggregator {
    lrclib: LrclibService,
    netease: NetEaseLyricsService,
    cache: Cache<SearchQuery, Arc<LyricMatch>>,
    by_id: Cache<String, Arc<LyricMatch>>,
}

impl LyricsAggregator {
    pub fn new() -> Self {
        let client = super::http_client();
        Self {
            lrclib: LrclibService::new(client.clone()),
            netease: NetEaseLyricsService::new(client),
            cache: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(Duration::from_secs(86_400))
                .build(),
            by_id: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(Duration::from_secs(86_400))
                .build(),
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
            self.cache.insert(query.key(), hit).await;
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
            self.cache
                .try_get_with_by_ref(&signature.key(), async {
                    if let Some(hit) = self.find_lrclib(&signature).await {
                        if hit.has_lyrics() || hit.instrumental {
                            return Ok(Arc::new(hit));
                        }
                    }
                    self.find_netease(&signature).await.map(Arc::new).ok_or(())
                })
                .await
                .ok()
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
}
