mod handlers;
mod models;
mod security;
mod services;
mod web;

use axum::{
    extract::DefaultBodyLimit,
    middleware::from_fn_with_state,
    routing::{get, put},
    Router,
};
use handlers::*;
use security::{security_middleware, SecurityConfig};
use services::{DatabaseService, LyricsAggregator, MetadataEnricher, UptimeManager};
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
    pub db: Option<Arc<DatabaseService>>,
    pub uptime: Arc<UptimeManager>,
    pub start_time: Instant,
    pub total_song_requests: Arc<AtomicU64>,
    pub total_lyric_requests: Arc<AtomicU64>,
    pub security: Arc<SecurityConfig>,
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

    let start_time = Instant::now();

    // Connect to Neon PostgreSQL database
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://neondb_owner:npg_VT3XHKRoMil2@ep-flat-heart-bbtb6258-pooler.c-15.us-east-1.aws.neon.tech/neondb?sslmode=require&channel_binding=require".to_string()
    });

    let db_service = match DatabaseService::connect(&database_url).await {
        Ok(db) => {
            tracing::info!("Neon PostgreSQL database connected and initialized.");
            Some(Arc::new(db))
        }
        Err(e) => {
            tracing::warn!("Could not connect to Neon PostgreSQL database: {e}. Falling back to in-memory caching.");
            None
        }
    };

    let lyrics_aggregator = Arc::new(LyricsAggregator::new(db_service.clone()));
    let metadata_enricher = Arc::new(MetadataEnricher::new(lyrics_aggregator.clone(), db_service.clone()));
    let security_config = Arc::new(SecurityConfig::from_env());

    // Initialize Dual-Store Uptime & Telemetry Manager
    let uptime_manager = Arc::new(UptimeManager::new(start_time, db_service.clone()));
    uptime_manager.init_from_database().await;

    // Record system startup event in both Render and Neon DB
    let is_db_connected = db_service.is_some();
    uptime_manager
        .record(services::uptime::UptimeLogEntry {
            id: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            service: "system".to_string(),
            render_status: "ONLINE".to_string(),
            render_uptime_seconds: 0,
            neon_status: if is_db_connected {
                "ONLINE".to_string()
            } else {
                "OFFLINE".to_string()
            },
            neon_latency_ms: None,
            neon_metadata_count: 0,
            neon_lyrics_count: 0,
            event_type: "STARTUP".to_string(),
            message: "MetaEnricher engine initialized with dual-store cross-redundant uptime logging.".to_string(),
        })
        .await;

    // Background Task 1: Periodic Heartbeat & Dual-Uptime Logging (Every 30s)
    let uptime_hb = uptime_manager.clone();
    let db_hb = db_service.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            uptime_hb.heartbeat().await;
            if let Some(db) = &db_hb {
                let _ = db.refresh_counts().await;
            }
        }
    });

    // Background Task 2: Periodic Render Memory Cleanup (Every 2 minutes)
    // "dont store any metadata anything in render delete it periodically, only keep in neon"
    let enricher_purge = metadata_enricher.clone();
    let lyrics_purge = lyrics_aggregator.clone();
    let uptime_purge = uptime_manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(120));
        loop {
            interval.tick().await;
            let m = enricher_purge.purge_memory_cache().await;
            let (l1, l2) = lyrics_purge.purge_memory_cache().await;
            if m > 0 || (l1 + l2) > 0 {
                tracing::info!(
                    "Render ephemeral memory purge: cleared {} metadata and {} lyrics from memory. Authoritative cache preserved in Neon DB.",
                    m, l1 + l2
                );
                uptime_purge.record_purge(m, l1 + l2).await;
            }
        }
    });

    let app_state = AppState {
        enricher: metadata_enricher,
        lyrics: lyrics_aggregator,
        db: db_service,
        uptime: uptime_manager,
        start_time,
        total_song_requests: Arc::new(AtomicU64::new(0)),
        total_lyric_requests: Arc::new(AtomicU64::new(0)),
        security: security_config,
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
        // Dedicated Uptime & System Statistics Page
        .route("/uptime", get(web::uptime_handler))
        .route("/status", get(web::uptime_handler))
        .route("/api/uptime/logs", get(web::uptime_logs_handler))
        .route("/api/uptime/stats", get(web::uptime_stats_handler))
        .route("/api/cache/purge", axum::routing::post(web::manual_purge_handler))
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
        // Security middleware: Rate limiting + API Key / Basic Auth validation
        .layer(from_fn_with_state(app_state.clone(), security_middleware))
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
