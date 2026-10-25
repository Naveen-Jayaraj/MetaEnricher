pub mod lrclib;
pub mod netease;

use crate::models::{LyricQuery, LyricResponse};
use lrclib::LrclibService;
use moka::future::Cache;
use netease::NetEaseLyricsService;
use std::time::Duration;

#[derive(Clone)]
pub struct LyricsAggregator {
    lrclib: LrclibService,
    netease: NetEaseLyricsService,
    cache: Cache<String, LyricResponse>,
}

impl LyricsAggregator {
    pub fn new() -> Self {
        let cache = Cache::builder()
            .max_capacity(10_000)
            .time_to_live(Duration::from_secs(86400)) // 24-hour cache
            .build();

        Self {
            lrclib: LrclibService::new(),
            netease: NetEaseLyricsService::new(),
            cache,
        }
    }

    pub async fn get_lyrics(&self, query: &LyricQuery) -> LyricResponse {
        let cache_key = format!(
            "{}:{}:{}",
            query.title.to_lowercase().trim(),
            query.artist.to_lowercase().trim(),
            query.duration_ms.unwrap_or(0) / 1000
        );

        if let Some(cached) = self.cache.get(&cache_key).await {
            let mut result = cached;
            if let Some(ref id) = query.id {
                result.id = id.clone();
            }
            return result;
        }

        let track_id = query
            .id
            .clone()
            .unwrap_or_else(|| format!("{:x}", chrono::Utc::now().timestamp_micros()));

        // 1. Try LRCLIB (best source for line-synced lyrics)
        if let Some(lrc_res) = self
            .lrclib
            .fetch_lyrics(
                &query.title,
                &query.artist,
                query.album.as_deref(),
                query.duration_ms,
            )
            .await
        {
            let has_synced = lrc_res.synced_lyrics.is_some();
            let raw_text = lrc_res
                .synced_lyrics
                .as_deref()
                .or(lrc_res.plain_lyrics.as_deref())
                .unwrap_or("");
            let lines_count = raw_text.lines().count();

            let resp = LyricResponse {
                id: track_id.clone(),
                title: query.title.clone(),
                artist: query.artist.clone(),
                lyrics_available: has_synced || lrc_res.plain_lyrics.is_some(),
                has_synced,
                synced_lyrics: lrc_res.synced_lyrics,
                plain_lyrics: lrc_res.plain_lyrics,
                source: lrc_res.source,
                lines_count,
                lyrics_uri: Some(format!("/lyric/file?id={}", track_id)),
            };

            self.cache.insert(cache_key, resp.clone()).await;
            return resp;
        }

        // 2. Try NetEase
        if let Some(ne_res) = self.netease.fetch_lyrics(&query.title, &query.artist).await {
            let has_synced = ne_res.synced_lyrics.is_some();
            let raw_text = ne_res
                .synced_lyrics
                .as_deref()
                .or(ne_res.plain_lyrics.as_deref())
                .unwrap_or("");
            let lines_count = raw_text.lines().count();

            let resp = LyricResponse {
                id: track_id.clone(),
                title: query.title.clone(),
                artist: query.artist.clone(),
                lyrics_available: has_synced || ne_res.plain_lyrics.is_some(),
                has_synced,
                synced_lyrics: ne_res.synced_lyrics,
                plain_lyrics: ne_res.plain_lyrics,
                source: ne_res.source,
                lines_count,
                lyrics_uri: Some(format!("/lyric/file?id={}", track_id)),
            };

            self.cache.insert(cache_key, resp.clone()).await;
            return resp;
        }

        // Not found
        LyricResponse {
            id: track_id,
            title: query.title.clone(),
            artist: query.artist.clone(),
            lyrics_available: false,
            has_synced: false,
            synced_lyrics: None,
            plain_lyrics: None,
            source: "None".to_string(),
            lines_count: 0,
            lyrics_uri: None,
        }
    }

    /// Retrieve raw LRC text for a given cached or queried track
    pub async fn get_raw_lrc(&self, id: &str, title: &str, artist: &str) -> Option<String> {
        let query = LyricQuery {
            id: Some(id.to_string()),
            title: title.to_string(),
            artist: artist.to_string(),
            album: None,
            duration_ms: None,
        };
        let res = self.get_lyrics(&query).await;
        res.synced_lyrics.or(res.plain_lyrics)
    }

    pub fn get_cache(&self) -> &Cache<String, LyricResponse> {
        &self.cache
    }
}
