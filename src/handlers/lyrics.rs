use crate::models::{BatchLyricRequest, BatchLyricResponse};
use crate::AppState;
use axum::{extract::State, Json};
use futures::future::join_all;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// PUT or POST /lyrics or /api/enrich-lyrics/batch
/// Parallel batch lyrics retrieval with concurrency limiting.
pub async fn lyrics_batch_handler(
    State(state): State<AppState>,
    Json(payload): Json<BatchLyricRequest>,
) -> Json<BatchLyricResponse> {
    let queries = payload.into_vec();
    let total = queries.len();
    state
        .total_lyric_requests
        .fetch_add(total as u64, Ordering::Relaxed);

    let semaphore = Arc::new(Semaphore::new(8));
    let mut tasks = Vec::with_capacity(total);

    for query in queries {
        let lyrics_service = state.lyrics.clone();
        let sem = semaphore.clone();

        tasks.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.ok();
            lyrics_service.get_lyrics(&query).await
        }));
    }

    let results = join_all(tasks).await;
    let mut lyric_results = Vec::with_capacity(total);
    let mut found_count = 0;

    for res in results {
        if let Ok(lyric_resp) = res {
            if lyric_resp.lyrics_available {
                found_count += 1;
            }
            lyric_results.push(lyric_resp);
        }
    }

    Json(BatchLyricResponse {
        success: true,
        total,
        found: found_count,
        results: lyric_results,
    })
}
