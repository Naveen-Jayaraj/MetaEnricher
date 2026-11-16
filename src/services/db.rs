use crate::services::enricher::Discovery;
use crate::services::lyrics::LyricMatch;
use crate::services::matching::{Recording, SearchQuery};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

pub struct DatabaseService {
    pool: PgPool,
}

impl DatabaseService {
    /// Initialize connection pool to Neon PostgreSQL and run database migrations.
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        info!("Connecting to Neon PostgreSQL...");

        let mut last_err = None;
        for attempt in 1..=3 {
            match PgPoolOptions::new()
                .max_connections(10)
                .min_connections(1)
                .acquire_timeout(Duration::from_secs(10))
                .idle_timeout(Duration::from_secs(60))
                .max_lifetime(Duration::from_secs(300))
                .connect(url)
                .await
            {
                Ok(pool) => {
                    let service = Self { pool };
                    service.run_migrations().await?;
                    return Ok(service);
                }
                Err(e) => {
                    warn!("Neon connection attempt {attempt} failed: {e}");
                    last_err = Some(e);
                    tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await;
                }
            }
        }
        Err(last_err.unwrap())
    }

    /// Automatically create cache tables and indices if they do not exist.
    pub async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS metadata_cache (
                cache_key VARCHAR(512) PRIMARY KEY,
                title VARCHAR(512) NOT NULL,
                artist VARCHAR(512) NOT NULL,
                album VARCHAR(512),
                year INT,
                duration_ms BIGINT,
                track_number INT,
                genre VARCHAR(255),
                composer TEXT,
                producer TEXT,
                language VARCHAR(100),
                mood VARCHAR(100),
                artwork_uri TEXT,
                lyrics_uri TEXT,
                lyrics_available BOOLEAN DEFAULT FALSE,
                synced_lyrics TEXT,
                plain_lyrics TEXT,
                sources JSONB DEFAULT '[]'::jsonb,
                confidence REAL DEFAULT 0.0,
                recording_json JSONB NOT NULL,
                hit_count BIGINT DEFAULT 1,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW()
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_metadata_cache_title_artist 
            ON metadata_cache (LOWER(title), LOWER(artist))
            "#
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_metadata_cache_updated_at 
            ON metadata_cache (updated_at DESC)
            "#
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS lyrics_cache (
                cache_key VARCHAR(512) PRIMARY KEY,
                title VARCHAR(512) NOT NULL,
                artist VARCHAR(512) NOT NULL,
                synced_lyrics TEXT,
                plain_lyrics TEXT,
                instrumental BOOLEAN DEFAULT FALSE,
                source VARCHAR(64),
                source_id VARCHAR(255),
                hit_count BIGINT DEFAULT 1,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW()
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_lyrics_cache_title_artist 
            ON lyrics_cache (LOWER(title), LOWER(artist))
            "#
        )
        .execute(&self.pool)
        .await?;

        info!("Neon PostgreSQL schema verified & migrations complete.");
        Ok(())
    }

    /// Retrieve cached metadata discovery for a given search query.
    pub(crate) async fn get_metadata(&self, query: &SearchQuery) -> Option<Discovery> {
        let cache_key = query.cache_key();
        let query_title = query.title.trim().to_lowercase();
        let query_artist = query.artist().trim().to_lowercase();

        let row = sqlx::query(
            r#"
            SELECT 
                cache_key, title, artist, album, year, duration_ms, track_number,
                genre, composer, producer, language, mood, artwork_uri,
                lyrics_uri, lyrics_available, synced_lyrics, plain_lyrics,
                sources, confidence, recording_json
            FROM metadata_cache
            WHERE cache_key = $1 
               OR (LOWER(title) = $2 AND LOWER(artist) = $3)
            ORDER BY (cache_key = $1) DESC, hit_count DESC
            LIMIT 1
            "#
        )
        .bind(&cache_key)
        .bind(&query_title)
        .bind(&query_artist)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| warn!("Neon get_metadata query error: {e}"))
        .ok()??;

        let recording_json: serde_json::Value = row.get("recording_json");
        let recording: Recording = serde_json::from_value(recording_json)
            .map_err(|e| warn!("Failed to deserialize cached recording: {e}"))
            .ok()?;

        let sources_val: serde_json::Value = row.get("sources");
        let sources: Vec<String> = serde_json::from_value(sources_val).unwrap_or_default();
        let confidence: f32 = row.get("confidence");

        let lyrics_available: bool = row.get("lyrics_available");
        let synced_lyrics: Option<String> = row.get("synced_lyrics");
        let plain_lyrics: Option<String> = row.get("plain_lyrics");

        let lyrics = if lyrics_available || synced_lyrics.is_some() || plain_lyrics.is_some() {
            let row_title: String = row.get("title");
            let row_artist: String = row.get("artist");
            Some(Arc::new(LyricMatch::new_cached(
                "lrclib",
                &format!("neon-{}", row_title),
                &row_title,
                &row_artist,
                synced_lyrics,
                plain_lyrics,
                false,
            )))
        } else {
            None
        };

        // Increment hit_count in background
        let pool = self.pool.clone();
        let found_key: String = row.get("cache_key");
        tokio::spawn(async move {
            let _ = sqlx::query("UPDATE metadata_cache SET hit_count = hit_count + 1 WHERE cache_key = $1")
                .bind(found_key)
                .execute(&pool)
                .await;
        });

        Some(Discovery {
            recording: Some(recording),
            sources,
            confidence,
            lyrics,
        })
    }

    /// Cache discovered metadata to Neon PostgreSQL.
    pub(crate) async fn save_metadata(&self, query: &SearchQuery, discovery: &Discovery) -> Result<(), sqlx::Error> {
        let recording = match &discovery.recording {
            Some(r) => r,
            None => return Ok(()), // Only cache when metadata is found
        };

        let cache_key = query.cache_key();
        let title = &recording.title;
        let artist = recording.artists.join(" / ");
        let album = recording.album.as_deref();
        let year = recording.year;
        let duration_ms = recording.duration_ms;
        let track_number = recording.track_number;
        let genre = recording.genre.as_deref();
        let composer = if recording.composers.is_empty() {
            None
        } else {
            Some(recording.composers.join(" / "))
        };
        let producer = if recording.producers.is_empty() {
            None
        } else {
            Some(recording.producers.join(" / "))
        };
        let language = recording.language.as_deref();
        let mood = None::<String>;
        let artwork_uri = recording.artwork.as_deref();

        let (lyrics_available, lyrics_uri, synced_lyrics, plain_lyrics, instrumental, lyrics_source, lyrics_id) = 
            if let Some(l) = &discovery.lyrics {
                (
                    l.has_lyrics(),
                    l.uri(),
                    l.synced.clone(),
                    l.plain.clone(),
                    l.instrumental,
                    Some(l.candidate.source.name().to_string()),
                    Some(l.candidate.id.clone()),
                )
            } else {
                (false, None, None, None, false, None, None)
            };

        let sources_json = serde_json::to_value(&discovery.sources).unwrap_or(serde_json::json!([]));
        let recording_json = serde_json::to_value(recording).unwrap_or(serde_json::json!({}));

        sqlx::query(
            r#"
            INSERT INTO metadata_cache (
                cache_key, title, artist, album, year, duration_ms, track_number,
                genre, composer, producer, language, mood, artwork_uri, lyrics_uri,
                lyrics_available, synced_lyrics, plain_lyrics, sources, confidence,
                recording_json, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7,
                $8, $9, $10, $11, $12, $13, $14,
                $15, $16, $17, $18, $19,
                $20, NOW()
            )
            ON CONFLICT (cache_key) DO UPDATE SET
                title = EXCLUDED.title,
                artist = EXCLUDED.artist,
                album = COALESCE(EXCLUDED.album, metadata_cache.album),
                year = COALESCE(EXCLUDED.year, metadata_cache.year),
                duration_ms = COALESCE(EXCLUDED.duration_ms, metadata_cache.duration_ms),
                track_number = COALESCE(EXCLUDED.track_number, metadata_cache.track_number),
                genre = COALESCE(EXCLUDED.genre, metadata_cache.genre),
                composer = COALESCE(EXCLUDED.composer, metadata_cache.composer),
                producer = COALESCE(EXCLUDED.producer, metadata_cache.producer),
                language = COALESCE(EXCLUDED.language, metadata_cache.language),
                mood = COALESCE(EXCLUDED.mood, metadata_cache.mood),
                artwork_uri = COALESCE(EXCLUDED.artwork_uri, metadata_cache.artwork_uri),
                lyrics_uri = COALESCE(EXCLUDED.lyrics_uri, metadata_cache.lyrics_uri),
                lyrics_available = EXCLUDED.lyrics_available OR metadata_cache.lyrics_available,
                synced_lyrics = COALESCE(EXCLUDED.synced_lyrics, metadata_cache.synced_lyrics),
                plain_lyrics = COALESCE(EXCLUDED.plain_lyrics, metadata_cache.plain_lyrics),
                sources = EXCLUDED.sources,
                confidence = GREATEST(EXCLUDED.confidence, metadata_cache.confidence),
                recording_json = EXCLUDED.recording_json,
                hit_count = metadata_cache.hit_count + 1,
                updated_at = NOW()
            "#
        )
        .bind(&cache_key)
        .bind(title)
        .bind(&artist)
        .bind(album)
        .bind(year)
        .bind(duration_ms)
        .bind(track_number)
        .bind(genre)
        .bind(composer)
        .bind(producer)
        .bind(language)
        .bind(mood)
        .bind(artwork_uri)
        .bind(lyrics_uri)
        .bind(lyrics_available)
        .bind(synced_lyrics.as_deref())
        .bind(plain_lyrics.as_deref())
        .bind(sources_json)
        .bind(discovery.confidence)
        .bind(recording_json)
        .execute(&self.pool)
        .await?;

        // If lyrics are available, also cache into lyrics_cache
        if lyrics_available || instrumental {
            let lyric_key = format!("t:{}|a:{}", title.to_lowercase(), artist.to_lowercase());
            let _ = sqlx::query(
                r#"
                INSERT INTO lyrics_cache (
                    cache_key, title, artist, synced_lyrics, plain_lyrics, instrumental,
                    source, source_id, updated_at
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, NOW()
                )
                ON CONFLICT (cache_key) DO UPDATE SET
                    synced_lyrics = COALESCE(EXCLUDED.synced_lyrics, lyrics_cache.synced_lyrics),
                    plain_lyrics = COALESCE(EXCLUDED.plain_lyrics, lyrics_cache.plain_lyrics),
                    instrumental = EXCLUDED.instrumental,
                    source = EXCLUDED.source,
                    source_id = EXCLUDED.source_id,
                    hit_count = lyrics_cache.hit_count + 1,
                    updated_at = NOW()
                "#
            )
            .bind(&lyric_key)
            .bind(title)
            .bind(&artist)
            .bind(synced_lyrics.as_deref())
            .bind(plain_lyrics.as_deref())
            .bind(instrumental)
            .bind(lyrics_source)
            .bind(lyrics_id)
            .execute(&self.pool)
            .await;
        }

        Ok(())
    }

    /// Retrieve cached lyrics for a given query.
    pub(crate) async fn get_lyrics(&self, query: &SearchQuery) -> Option<LyricMatch> {
        let cache_key = format!("t:{}|a:{}", query.title.trim().to_lowercase(), query.artist().trim().to_lowercase());
        let query_title = query.title.trim().to_lowercase();
        let query_artist = query.artist().trim().to_lowercase();

        let row = sqlx::query(
            r#"
            SELECT 
                cache_key, title, artist, synced_lyrics, plain_lyrics, instrumental, source, source_id
            FROM lyrics_cache
            WHERE cache_key = $1 
               OR (LOWER(title) = $2 AND LOWER(artist) = $3)
            ORDER BY (cache_key = $1) DESC, hit_count DESC
            LIMIT 1
            "#
        )
        .bind(&cache_key)
        .bind(&query_title)
        .bind(&query_artist)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| warn!("Neon get_lyrics query error: {e}"))
        .ok()??;

        let title: String = row.get("title");
        let artist: String = row.get("artist");
        let synced_lyrics: Option<String> = row.get("synced_lyrics");
        let plain_lyrics: Option<String> = row.get("plain_lyrics");
        let instrumental: bool = row.get("instrumental");
        let source: Option<String> = row.get("source");
        let source_id: Option<String> = row.get("source_id");

        let source_name = source.as_deref().unwrap_or("LRCLIB");
        let id_val = source_id.unwrap_or_else(|| format!("{:x}", chrono::Utc::now().timestamp_micros()));

        Some(LyricMatch::new_cached(
            source_name,
            &id_val,
            &title,
            &artist,
            synced_lyrics,
            plain_lyrics,
            instrumental,
        ))
    }

    /// Save lyrics directly to Neon lyrics cache.
    pub(crate) async fn save_lyrics(&self, query: &SearchQuery, hit: &LyricMatch) -> Result<(), sqlx::Error> {
        let cache_key = format!("t:{}|a:{}", query.title.trim().to_lowercase(), query.artist().trim().to_lowercase());
        let title = &hit.candidate.recording.title;
        let artist = hit.candidate.recording.artists.first().map(String::as_str).unwrap_or(query.artist());

        sqlx::query(
            r#"
            INSERT INTO lyrics_cache (
                cache_key, title, artist, synced_lyrics, plain_lyrics, instrumental,
                source, source_id, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, NOW()
            )
            ON CONFLICT (cache_key) DO UPDATE SET
                synced_lyrics = COALESCE(EXCLUDED.synced_lyrics, lyrics_cache.synced_lyrics),
                plain_lyrics = COALESCE(EXCLUDED.plain_lyrics, lyrics_cache.plain_lyrics),
                instrumental = EXCLUDED.instrumental,
                source = EXCLUDED.source,
                source_id = EXCLUDED.source_id,
                hit_count = lyrics_cache.hit_count + 1,
                updated_at = NOW()
            "#
        )
        .bind(&cache_key)
        .bind(title)
        .bind(artist)
        .bind(hit.synced.as_deref())
        .bind(hit.plain.as_deref())
        .bind(hit.instrumental)
        .bind(hit.candidate.source.name())
        .bind(&hit.candidate.id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get counts of cached entries in Neon (metadata_count, lyrics_count).
    pub async fn counts(&self) -> (u64, u64) {
        let meta_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM metadata_cache")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        let lyrics_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM lyrics_cache")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        (meta_count.max(0) as u64, lyrics_count.max(0) as u64)
    }

    /// Quick ping to check if database connection is alive.
    pub async fn is_alive(&self) -> bool {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .is_ok()
    }
}
