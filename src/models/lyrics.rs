use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricQuery {
    pub id: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricResponse {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub lyrics_available: bool,
    pub has_synced: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synced_lyrics: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plain_lyrics: Option<String>,
    pub source: String,
    pub lines_count: usize,
    pub lyrics_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BatchLyricRequest {
    List(Vec<LyricQuery>),
    Wrapped { tracks: Vec<LyricQuery> },
}

impl BatchLyricRequest {
    pub fn into_vec(self) -> Vec<LyricQuery> {
        match self {
            BatchLyricRequest::List(list) => list,
            BatchLyricRequest::Wrapped { tracks } => tracks,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchLyricResponse {
    pub success: bool,
    pub total: usize,
    pub found: usize,
    pub results: Vec<LyricResponse>,
}
