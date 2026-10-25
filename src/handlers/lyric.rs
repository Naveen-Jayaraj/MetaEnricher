use crate::models::{LyricQuery, LyricResponse};
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::sync::atomic::Ordering;

#[derive(Debug, Deserialize)]
pub struct LyricFileQueryParams {
    pub id: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LyricGetQueryParams {
    pub id: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
}

/// PUT or POST /lyric or /api/enrich-lyrics
pub async fn lyric_put_handler(
    State(state): State<AppState>,
    Json(query): Json<LyricQuery>,
) -> Json<LyricResponse> {
    state.total_lyric_requests.fetch_add(1, Ordering::Relaxed);
    let result = state.lyrics.get_lyrics(&query).await;
    Json(result)
}

/// GET /lyric?title=...&artist=...
pub async fn lyric_get_handler(
    State(state): State<AppState>,
    Query(params): Query<LyricGetQueryParams>,
) -> Json<LyricResponse> {
    state.total_lyric_requests.fetch_add(1, Ordering::Relaxed);
    let query = LyricQuery {
        id: params.id,
        title: params.title,
        artist: params.artist,
        album: params.album,
        duration_ms: params.duration_ms,
    };
    let result = state.lyrics.get_lyrics(&query).await;
    Json(result)
}

/// GET /lyric/file?id=...&title=...&artist=...
/// Returns the raw LRC file text directly for the OfflineMusic app to store into lyricsUri.
pub async fn lyric_file_download_handler(
    State(state): State<AppState>,
    Query(params): Query<LyricFileQueryParams>,
) -> Response {
    let id = params.id.unwrap_or_else(|| "track".to_string());
    let title = params.title.unwrap_or_default();
    let artist = params.artist.unwrap_or_default();

    if title.is_empty() && artist.is_empty() {
        return (StatusCode::BAD_REQUEST, "Missing title and artist query parameters").into_response();
    }

    if let Some(lrc_content) = state.lyrics.get_raw_lrc(&id, &title, &artist).await {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "text/plain; charset=utf-8".parse().unwrap());
        headers.insert(
            header::CONTENT_DISPOSITION,
            format!(r#"attachment; filename="{}.lrc""#, sanitize_filename(&title)).parse().unwrap(),
        );
        (StatusCode::OK, headers, lrc_content).into_response()
    } else {
        (StatusCode::NOT_FOUND, "No lyrics found for track").into_response()
    }
}

/// GET /api/lyric/:id/file?title=...&artist=...
pub async fn lyric_file_by_id_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<LyricFileQueryParams>,
) -> Response {
    let title = params.title.unwrap_or_default();
    let artist = params.artist.unwrap_or_default();

    if let Some(lrc_content) = state.lyrics.get_raw_lrc(&id, &title, &artist).await {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "text/plain; charset=utf-8".parse().unwrap());
        headers.insert(
            header::CONTENT_DISPOSITION,
            format!(r#"attachment; filename="{}.lrc""#, id).parse().unwrap(),
        );
        (StatusCode::OK, headers, lrc_content).into_response()
    } else {
        (StatusCode::NOT_FOUND, "No lyrics found for track").into_response()
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect()
}
