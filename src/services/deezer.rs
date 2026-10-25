#![allow(dead_code)]

use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct DeezerService {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct DeezerSearchResponse {
    data: Option<Vec<DeezerTrackItem>>,
}

#[derive(Debug, Deserialize)]
pub struct DeezerTrackItem {
    pub id: i64,
    pub title: String,
    pub duration: Option<i64>,
    pub track_position: Option<i32>,
    pub artist: Option<DeezerArtistItem>,
    pub album: Option<DeezerAlbumItem>,
    pub contributors: Option<Vec<DeezerContributor>>,
}

#[derive(Debug, Deserialize)]
pub struct DeezerArtistItem {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct DeezerAlbumItem {
    pub id: i64,
    pub title: String,
    pub cover_xl: Option<String>,
    pub cover_big: Option<String>,
    pub cover_medium: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeezerContributor {
    pub name: String,
    pub role: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeezerTrackDetails {
    pub release_date: Option<String>,
    pub bpm: Option<f32>,
    pub contributors: Option<Vec<DeezerContributor>>,
}

#[derive(Debug, Deserialize)]
struct DeezerAlbumDetails {
    pub genres: Option<DeezerGenres>,
}

#[derive(Debug, Deserialize)]
struct DeezerGenres {
    pub data: Option<Vec<DeezerGenreItem>>,
}

#[derive(Debug, Deserialize)]
struct DeezerGenreItem {
    pub name: String,
}

#[derive(Debug, Clone, Default)]
pub struct DeezerEnrichedData {
    pub canonical_title: Option<String>,
    pub artist_name: Option<String>,
    pub album_title: Option<String>,
    pub release_year: Option<i32>,
    pub track_number: Option<i32>,
    pub genre: Option<String>,
    pub artwork_url: Option<String>,
    pub contributors: Vec<String>,
    pub duration_ms: Option<i64>,
}

impl DeezerService {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .user_agent("MetaEnricher/1.0 (OfflineMusic)")
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client }
    }

    pub async fn search_track(&self, title: &str, artist: &str) -> Option<DeezerEnrichedData> {
        // First try specific track + artist query
        let query = format!(r#"track:"{}" artist:"{}""#, title, artist);
        let mut results = self.execute_search(&query).await;

        // If not found, try simple fuzzy combined search
        if results.is_none() {
            let fallback_query = format!("{} {}", title, artist);
            results = self.execute_search(&fallback_query).await;
        }

        let track = results?.into_iter().next()?;
        let track_id = track.id;
        let album_id = track.album.as_ref().map(|a| a.id);

        let mut data = DeezerEnrichedData {
            canonical_title: Some(track.title),
            artist_name: track.artist.as_ref().map(|a| a.name.clone()),
            album_title: track.album.as_ref().map(|a| a.title.clone()),
            track_number: track.track_position,
            duration_ms: track.duration.map(|d| d * 1000),
            artwork_url: track.album.as_ref().and_then(|a| {
                a.cover_xl
                    .clone()
                    .or_else(|| a.cover_big.clone())
                    .or_else(|| a.cover_medium.clone())
            }),
            ..Default::default()
        };

        if let Some(contribs) = track.contributors {
            for c in contribs {
                data.contributors.push(c.name);
            }
        }

        // Fetch release date from track details in parallel if possible
        if let Ok(resp) = self
            .client
            .get(format!("https://api.deezer.com/track/{}", track_id))
            .send()
            .await
        {
            if let Ok(details) = resp.json::<DeezerTrackDetails>().await {
                if let Some(date_str) = details.release_date {
                    if let Some(year_str) = date_str.split('-').next() {
                        if let Ok(yr) = year_str.parse::<i32>() {
                            if yr > 1900 && yr < 2100 {
                                data.release_year = Some(yr);
                            }
                        }
                    }
                }
                if let Some(extra_contribs) = details.contributors {
                    for c in extra_contribs {
                        if !data.contributors.contains(&c.name) {
                            data.contributors.push(c.name);
                        }
                    }
                }
            }
        }

        // Fetch genre from album details if available
        if let Some(aid) = album_id {
            if let Ok(resp) = self
                .client
                .get(format!("https://api.deezer.com/album/{}", aid))
                .send()
                .await
            {
                if let Ok(album_details) = resp.json::<DeezerAlbumDetails>().await {
                    if let Some(genres) = album_details.genres {
                        if let Some(list) = genres.data {
                            if let Some(first) = list.into_iter().next() {
                                data.genre = Some(first.name);
                            }
                        }
                    }
                }
            }
        }

        Some(data)
    }

    async fn execute_search(&self, query: &str) -> Option<Vec<DeezerTrackItem>> {
        let url = format!(
            "https://api.deezer.com/search?q={}&limit=3",
            urlencoding::encode(query)
        );
        let resp = self.client.get(&url).send().await.ok()?;
        let search_res: DeezerSearchResponse = resp.json().await.ok()?;
        search_res.data
    }
}
