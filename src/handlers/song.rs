use crate::models::{SingleSongResponse, TrackEntity};
use crate::AppState;
use axum::{extract::State, Json};
use std::sync::atomic::Ordering;

/// PUT or POST /song or /api/enrich-metadata
/// Deep search enricher for a single song.
pub async fn enrich_song_handler(
    State(state): State<AppState>,
    Json(track): Json<TrackEntity>,
) -> Json<SingleSongResponse> {
    state.total_song_requests.fetch_add(1, Ordering::Relaxed);
    let enriched = state.enricher.enrich_song(track).await;
    Json(enriched)
}
