#![allow(dead_code)]

use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ITunesService {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct ITunesResponse {
    results: Option<Vec<ITunesTrackItem>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ITunesTrackItem {
    pub track_name: Option<String>,
    pub artist_name: Option<String>,
    pub collection_name: Option<String>,
    pub primary_genre_name: Option<String>,
    pub release_date: Option<String>,
    pub track_number: Option<i32>,
    pub track_time_millis: Option<i64>,
    pub artwork_url100: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ITunesEnrichedData {
    pub track_name: Option<String>,
    pub artist_name: Option<String>,
    pub album_name: Option<String>,
    pub primary_genre: Option<String>,
    pub release_year: Option<i32>,
    pub track_number: Option<i32>,
    pub high_res_artwork: Option<String>,
    pub duration_ms: Option<i64>,
}

impl ITunesService {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .user_agent("MetaEnricher/1.0 (OfflineMusic)")
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client }
    }

    pub async fn search_track(
        &self,
        title: &str,
        artist: &str,
        album: Option<&str>,
    ) -> Option<ITunesEnrichedData> {
        // 1. Try title + artist
        let mut term = format!("{} {}", title, artist);
        let mut track = self.execute_query(&term).await;

        // 2. Try title + album if album is known
        if track.is_none() {
            if let Some(alb) = album {
                if !alb.trim().is_empty() {
                    term = format!("{} {}", title, alb);
                    track = self.execute_query(&term).await;
                }
            }
        }

        // 3. Try primary title keyword (e.g. "Podimeesha" from "Podimeesha Mulakkana Kaalam")
        if track.is_none() {
            let first_word = title.split_whitespace().next().unwrap_or(title);
            if let Some(alb) = album {
                term = format!("{} {}", first_word, alb);
                track = self.execute_query(&term).await;
            }
            if track.is_none() {
                track = self.execute_query(first_word).await;
            }
        }

        let item = track?;

        let mut release_year = None;
        if let Some(date_str) = &item.release_date {
            if let Some(year_part) = date_str.split('-').next() {
                if let Ok(yr) = year_part.parse::<i32>() {
                    if yr > 1900 && yr < 2100 {
                        release_year = Some(yr);
                    }
                }
            }
        }

        let high_res_artwork = item.artwork_url100.map(|url| {
            url.replace("100x100bb.jpg", "1200x1200bb.jpg")
                .replace("100x100bb.png", "1200x1200bb.png")
        });

        Some(ITunesEnrichedData {
            track_name: item.track_name,
            artist_name: item.artist_name,
            album_name: item.collection_name,
            primary_genre: item.primary_genre_name,
            release_year,
            track_number: item.track_number,
            high_res_artwork,
            duration_ms: item.track_time_millis,
        })
    }

    async fn execute_query(&self, term: &str) -> Option<ITunesTrackItem> {
        let url = format!(
            "https://itunes.apple.com/search?term={}&entity=song&limit=5",
            urlencoding::encode(term)
        );
        let resp = self.client.get(&url).send().await.ok()?;
        let itunes_res: ITunesResponse = resp.json().await.ok()?;
        itunes_res.results?.into_iter().next()
    }
}
