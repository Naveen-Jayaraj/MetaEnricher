use crate::AppState;
use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::atomic::Ordering;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatus {
    pub status: String,
    pub server: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub total_song_requests: u64,
    pub total_lyric_requests: u64,
    pub metadata_cache_size: u64,
    pub lyrics_cache_size: u64,
}

pub async fn health_handler(State(state): State<AppState>) -> Json<HealthStatus> {
    let uptime = state.start_time.elapsed().as_secs();
    let song_requests = state.total_song_requests.load(Ordering::Relaxed);
    let lyric_requests = state.total_lyric_requests.load(Ordering::Relaxed);
    let meta_cache = state.enricher.get_cache().entry_count();
    let lyric_cache = state.lyrics.get_cache().entry_count();

    Json(HealthStatus {
        status: "ok".to_string(),
        server: "MetaEnricher-Rust".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
        total_song_requests: song_requests,
        total_lyric_requests: lyric_requests,
        metadata_cache_size: meta_cache,
        lyrics_cache_size: lyric_cache,
    })
}
