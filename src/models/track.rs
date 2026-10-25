use serde::{Deserialize, Serialize};

/// Represents a track in OfflineMusic (`TrackEntity.kt` / `Song.kt`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackEntity {
    /// Unique track hash or ID.
    #[serde(default = "default_id")]
    pub id: String,

    /// Track title (also accepts "name").
    #[serde(alias = "name")]
    pub title: String,

    /// Credited artist(s). Multiple artists separated by " / " (e.g. "The Weeknd / Daft Punk").
    pub artist: String,

    /// Name of album or EP.
    #[serde(default)]
    pub album: Option<String>,

    /// Release year of track or album.
    #[serde(default)]
    pub year: Option<i32>,

    /// Musical genre (e.g. "Electronic", "R&B", "Pop").
    #[serde(default)]
    pub genre: Option<String>,

    /// Track order number on album / CD.
    #[serde(default)]
    pub track_number: Option<i32>,

    /// Music producer(s) / audio engineer.
    #[serde(default)]
    pub producer: Option<String>,

    /// Composer(s) / songwriter(s).
    #[serde(default)]
    pub composer: Option<String>,

    /// Vocal language (e.g. "English", "Spanish", "Instrumental").
    #[serde(default)]
    pub language: Option<String>,

    /// Vibe / mood classification (e.g. "Chill", "Energetic", "Dark", "Focus").
    #[serde(default)]
    pub mood: Option<String>,

    /// Where track originated ("LOCAL", "YOUTUBE", "LUCIDA").
    #[serde(default = "default_source")]
    pub source: String,

    /// Whether synchronized or unsynced lyrics exist.
    #[serde(default)]
    pub lyrics_available: bool,

    /// Server verification flag (set to true after successful enrichment).
    #[serde(default)]
    pub metachecked: bool,

    /// Audio duration in milliseconds.
    #[serde(default)]
    pub duration_ms: i64,

    /// Content URI or physical file path to audio stream.
    #[serde(alias = "uri", default)]
    pub file_uri: Option<String>,

    /// URI pointing to album artwork.
    #[serde(default)]
    pub artwork_uri: Option<String>,

    /// URI pointing directly to .lrc lyrics file or endpoint.
    #[serde(default)]
    pub lyrics_uri: Option<String>,

    /// Audio container format extension (mp3, flac, m4a, ogg, wav).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,

    /// File size in bytes.
    #[serde(default)]
    pub size_bytes: i64,

    /// Audio bitrate in kbps.
    #[serde(default)]
    pub bitrate_kbps: Option<i32>,

    /// Audio sample rate (44100, 48000 Hz).
    #[serde(default)]
    pub sample_rate: Option<i32>,

    /// Unix timestamp when scanned/added.
    #[serde(default)]
    pub date_added: i64,

    /// File system last modified timestamp.
    #[serde(default)]
    pub date_modified: i64,

    /// Internal parsing status ("PARSED", "CONFIRMED", "MANUAL").
    #[serde(default = "default_status")]
    pub metadata_status: String,
}

fn default_id() -> String {
    format!("{:x}", chrono::Utc::now().timestamp_micros())
}

fn default_source() -> String {
    "LOCAL".to_string()
}

fn default_status() -> String {
    "PARSED".to_string()
}

/// Audit report detailing what deep search discovered and fixed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrichmentAudit {
    pub matched: bool,
    pub confidence_score: f32,
    pub fields_updated: Vec<String>,
    pub fields_corrected: Vec<String>,
    pub sources_consulted: Vec<String>,
    pub raw_title_cleaned: bool,
    pub artists_reformatted: bool,
    pub artwork_enhanced: bool,
    pub lyrics_found: bool,
    pub detected_language: Option<String>,
    pub detected_mood: Option<String>,
}

/// Full response for a single song enrichment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SingleSongResponse {
    pub success: bool,
    pub song: TrackEntity,
    pub audit: EnrichmentAudit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lyrics_sample: Option<String>,
}

/// Request for batch song enrichment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BatchSongRequest {
    List(Vec<TrackEntity>),
    Wrapped { tracks: Vec<TrackEntity> },
}

impl BatchSongRequest {
    pub fn into_vec(self) -> Vec<TrackEntity> {
        match self {
            BatchSongRequest::List(list) => list,
            BatchSongRequest::Wrapped { tracks } => tracks,
        }
    }
}

/// Response for batch song enrichment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSongResponse {
    pub success: bool,
    pub total: usize,
    pub enriched: usize,
    pub tracks: Vec<SingleSongResponse>,
}
