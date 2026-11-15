use crate::models::{BatchSongRequest, BatchSongResponse};
use crate::AppState;
use axum::{extract::State, Json};
use futures::{stream, StreamExt};
use std::sync::atomic::Ordering;

/// PUT or POST /songs or /api/enrich-metadata/batch
/// Parallel batch enricher for multiple tracks with controlled concurrency.
pub async fn enrich_songs_batch_handler(
    State(state): State<AppState>,
    Json(payload): Json<BatchSongRequest>,
) -> Json<BatchSongResponse> {
    let tracks = payload.into_vec();
    let total = tracks.len();
    state
        .total_song_requests
        .fetch_add(total as u64, Ordering::Relaxed);

    let enricher = state.enricher.as_ref();
    let enriched_tracks: Vec<_> = stream::iter(tracks)
        .map(|track| enricher.enrich_song(track))
        .buffered(8)
        .collect()
        .await;
    let enriched_count = enriched_tracks
        .iter()
        .filter(|response| response.audit.matched)
        .count();

    Json(BatchSongResponse {
        success: true,
        total,
        enriched: enriched_count,
        tracks: enriched_tracks,
    })
}
