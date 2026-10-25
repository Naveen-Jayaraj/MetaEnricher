use crate::models::{BatchSongRequest, BatchSongResponse};
use crate::AppState;
use axum::{extract::State, Json};
use futures::future::join_all;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::Semaphore;

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

    // Limit concurrency to 8 parallel requests to be respectful to upstream APIs
    let semaphore = Arc::new(Semaphore::new(8));
    let mut tasks = Vec::with_capacity(total);

    for track in tracks {
        let enricher = state.enricher.clone();
        let sem = semaphore.clone();

        tasks.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.ok();
            enricher.enrich_song(track).await
        }));
    }

    let results = join_all(tasks).await;
    let mut enriched_tracks = Vec::with_capacity(total);
    let mut enriched_count = 0;

    for res in results {
        if let Ok(song_resp) = res {
            if song_resp.audit.matched {
                enriched_count += 1;
            }
            enriched_tracks.push(song_resp);
        }
    }

    Json(BatchSongResponse {
        success: true,
        total,
        enriched: enriched_count,
        tracks: enriched_tracks,
    })
}
