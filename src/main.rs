mod handlers;
mod models;
mod services;
mod web;

use axum::{
    extract::DefaultBodyLimit,
    routing::{get, put},
    Router,
};
use handlers::*;
use services::{LyricsAggregator, MetadataEnricher};
use std::net::SocketAddr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Instant;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
pub struct AppState {
    pub enricher: Arc<MetadataEnricher>,
    pub lyrics: Arc<LyricsAggregator>,
    pub start_time: Instant,
    pub total_song_requests: Arc<AtomicU64>,
    pub total_lyric_requests: Arc<AtomicU64>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "meta_enricher=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Initializing MetaEnricher Core Services...");

    let lyrics_aggregator = Arc::new(LyricsAggregator::new());
    let metadata_enricher = Arc::new(MetadataEnricher::new(lyrics_aggregator.clone()));

    let app_state = AppState {
        enricher: metadata_enricher,
        lyrics: lyrics_aggregator,
        start_time: Instant::now(),
        total_song_requests: Arc::new(AtomicU64::new(0)),
        total_lyric_requests: Arc::new(AtomicU64::new(0)),
    };

    // Configure CORS so Android app and browser client can connect without restrictions
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build Axum Router
    let app = Router::new()
        // Interactive Dashboard
        .route("/", get(web::dashboard_handler))
        // Single Song Enricher (Supports PUT & POST on both /song and /api/enrich-metadata)
        .route("/song", put(enrich_song_handler).post(enrich_song_handler))
        .route("/api/enrich-metadata", put(enrich_song_handler).post(enrich_song_handler))
        .route("/api/song", put(enrich_song_handler).post(enrich_song_handler))
        // Batch Songs Enricher
        .route("/songs", put(enrich_songs_batch_handler).post(enrich_songs_batch_handler))
        .route("/api/enrich-metadata/batch", put(enrich_songs_batch_handler).post(enrich_songs_batch_handler))
        .route("/api/songs", put(enrich_songs_batch_handler).post(enrich_songs_batch_handler))
        // Single Lyric (Supports PUT, POST, GET on /lyric and /api/enrich-lyrics)
        .route("/lyric", put(lyric_put_handler).post(lyric_put_handler).get(lyric_get_handler))
        .route("/api/enrich-lyrics", put(lyric_put_handler).post(lyric_put_handler).get(lyric_get_handler))
        .route("/api/lyric", put(lyric_put_handler).post(lyric_put_handler).get(lyric_get_handler))
        // Direct .lrc file download endpoints (For lyricsUri)
        .route("/lyric/file", get(lyric_file_download_handler))
        .route("/api/lyric/{id}/file", get(lyric_file_by_id_handler))
        // Batch Lyrics
        .route("/lyrics", put(lyrics_batch_handler).post(lyrics_batch_handler))
        .route("/api/enrich-lyrics/batch", put(lyrics_batch_handler).post(lyrics_batch_handler))
        .route("/api/lyrics", put(lyrics_batch_handler).post(lyrics_batch_handler))
        // Health & Telemetry
        .route("/health", get(health_handler))
        .route("/api/health", get(health_handler))
        .layer(DefaultBodyLimit::max(15 * 1024 * 1024)) // 15MB request limit
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("MetaEnricher server listening on http://{}", addr);
    tracing::info!("Dashboard ready at http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
