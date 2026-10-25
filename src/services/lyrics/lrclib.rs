#![allow(dead_code)]

use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct LrclibService {
    client: Client,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LrclibTrack {
    pub id: Option<i64>,
    pub track_name: Option<String>,
    pub artist_name: Option<String>,
    pub album_name: Option<String>,
    pub duration: Option<f64>,
    pub instrumental: Option<bool>,
    pub plain_lyrics: Option<String>,
    pub synced_lyrics: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LrclibResult {
    pub synced_lyrics: Option<String>,
    pub plain_lyrics: Option<String>,
    pub is_instrumental: bool,
    pub source: String,
}

impl LrclibService {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(6))
            .user_agent("MetaEnricher/1.0 (Lyrica/OfflineMusic; +https://github.com/Wilooper/Lyrica)")
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client }
    }

    pub async fn fetch_lyrics(
        &self,
        title: &str,
        artist: &str,
        album: Option<&str>,
        duration_ms: Option<i64>,
    ) -> Option<LrclibResult> {
        // 1. Try exact match endpoint: /api/get
        let mut url = format!(
            "https://lrclib.net/api/get?track_name={}&artist_name={}",
            urlencoding::encode(title),
            urlencoding::encode(artist)
        );

        if let Some(alb) = album {
            if !alb.trim().is_empty() {
                url.push_str(&format!("&album_name={}", urlencoding::encode(alb)));
            }
        }

        if let Some(d) = duration_ms {
            if d > 0 {
                let dur_secs = d / 1000;
                url.push_str(&format!("&duration={}", dur_secs));
            }
        }

        if let Ok(resp) = self.client.get(&url).send().await {
            if resp.status().is_success() {
                if let Ok(track) = resp.json::<LrclibTrack>().await {
                    if track.synced_lyrics.is_some() || track.plain_lyrics.is_some() || track.instrumental == Some(true) {
                        return Some(LrclibResult {
                            synced_lyrics: track.synced_lyrics,
                            plain_lyrics: track.plain_lyrics,
                            is_instrumental: track.instrumental.unwrap_or(false),
                            source: "LRCLIB".to_string(),
                        });
                    }
                }
            }
        }

        // 2. Fallback to /api/search
        let search_query = format!("{} {}", title, artist);
        let search_url = format!(
            "https://lrclib.net/api/search?q={}",
            urlencoding::encode(&search_query)
        );

        if let Ok(resp) = self.client.get(&search_url).send().await {
            if resp.status().is_success() {
                if let Ok(tracks) = resp.json::<Vec<LrclibTrack>>().await {
                    for track in tracks {
                        if track.synced_lyrics.is_some() || track.plain_lyrics.is_some() {
                            return Some(LrclibResult {
                                synced_lyrics: track.synced_lyrics,
                                plain_lyrics: track.plain_lyrics,
                                is_instrumental: track.instrumental.unwrap_or(false),
                                source: "LRCLIB-Search".to_string(),
                            });
                        }
                    }
                }
            }
        }

        None
    }
}
