use crate::models::{SingleSongResponse, TrackEntity};
use crate::AppState;
use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use std::sync::atomic::Ordering;

/// PUT or POST /song or /api/enrich-metadata
/// Deep search enricher for a single song.
pub async fn enrich_song_handler(
    State(state): State<AppState>,
    Query(params): Query<SongLookupParams>,
    Json(mut track): Json<TrackEntity>,
) -> Json<SingleSongResponse> {
    state.total_song_requests.fetch_add(1, Ordering::Relaxed);
    track.youtube_id = params.youtube_id.or(track.youtube_id);
    let enriched = state.enricher.enrich_song(track).await;
    Json(enriched)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongLookupParams {
    youtube_id: Option<String>,
}
