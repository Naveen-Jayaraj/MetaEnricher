use super::LyricMatch;
use crate::services::cleaner::split_artists;
use crate::services::matching::{
    album_key, catalog_artists, duration_delta, score, Candidate, Recording, SearchQuery, Source,
};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::{sleep_until, Instant};

pub(super) struct LrclibService {
    client: Client,
    next_request: Mutex<Instant>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibTrack {
    id: i64,
    track_name: String,
    artist_name: String,
    album_name: Option<String>,
    duration: Option<f64>,
    instrumental: Option<bool>,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

impl LrclibTrack {
    fn into_match(self, query: &SearchQuery) -> LyricMatch {
        let mut artists = catalog_artists(&self.artist_name, query);
        let mut split = split_artists(&self.artist_name);
        let original_count = split.len();
        if original_count > 1 {
            let album = self.album_name.as_deref().map(album_key);
            split.retain(|name| {
                let year = name.len() == 4
                    && name
                        .parse::<u16>()
                        .is_ok_and(|y| (1900..=2100).contains(&y));
                let film = album
                    .as_ref()
                    .is_some_and(|a| !a.is_empty() && *a == album_key(name));
                !year && !film
            });
            if !split.is_empty() && split.len() < original_count {
                artists = split;
            }
        }
        let recording = Recording {
            title: self.track_name,
            artists,
            album: self.album_name,
            duration_ms: self.duration.map(|s| (s * 1000.0).round() as i64),
            ..Recording::default()
        };
        // A radio edit and an album cut may have the same lyrics and different timings.
        let synced = self
            .synced_lyrics
            .filter(|s| !s.trim().is_empty())
            .filter(|_| {
                query.duration_ms.is_none()
                    || duration_delta(query.duration_ms, recording.duration_ms)
                        .is_some_and(|d| d <= 3_000)
            });
        LyricMatch {
            candidate: Candidate::new(Source::Lrclib, self.id.to_string(), recording),
            synced,
            plain: self.plain_lyrics.filter(|s| !s.trim().is_empty()),
            instrumental: self.instrumental.unwrap_or(false),
        }
    }
}

impl LrclibService {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            next_request: Mutex::new(Instant::now()),
        }
    }

    pub async fn find(&self, query: &SearchQuery) -> Option<LyricMatch> {
        let mut next = self.next_request.lock().await;
        let now = Instant::now();
        if *next > now + Duration::from_millis(900) {
            return None;
        }
        let start = (*next).max(now);
        *next = start + Duration::from_millis(250);
        sleep_until(start).await;
        drop(next);

        let mut params = vec![("track_name", query.title.as_str())];
        if !query.artist().is_empty() {
            params.push(("artist_name", query.artist()));
        }
        let result = self
            .client
            .get("https://lrclib.net/api/search")
            .query(&params)
            .send()
            .await
            .ok()?;
        if result.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry = result
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|h| h.to_str().ok())
                .and_then(|n| n.parse().ok())
                .unwrap_or(60);
            let mut next = self.next_request.lock().await;
            *next = (*next).max(Instant::now() + Duration::from_secs(retry.min(86_400)));
            return None;
        }
        let tracks: Vec<LrclibTrack> = result.error_for_status().ok()?.json().await.ok()?;

        best_match(tracks, query)
    }
}

fn best_match(tracks: Vec<LrclibTrack>, query: &SearchQuery) -> Option<LyricMatch> {
    if query.artists.is_empty() && query.album.is_none() {
        return None;
    }
    tracks
        .into_iter()
        .map(|track| track.into_match(query))
        .filter_map(|hit| score(query, &hit.candidate.recording).map(|points| (points, hit)))
        .max_by_key(|(points, hit)| (*points, hit.synced.is_some(), hit.has_lyrics()))
        .map(|(_, hit)| hit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_artist_and_timing_before_using_lyrics_or_credits() {
        let tracks: Vec<LrclibTrack> = serde_json::from_str(r#"[
            {"id":1, "trackName":"Happier", "artistName":"Olivia Rodrigo", "duration":214, "syncedLyrics":"Wrong song"},
            {"id":2, "trackName":"Happier", "artistName":"Marshmello, Bastille", "duration":222, "syncedLyrics":"Out of sync", "plainLyrics":"Correct words"},
            {"id":3, "trackName":"Happier (Stripped)", "artistName":"Marshmello, Bastille", "duration":214, "syncedLyrics":"Wrong version"}
        ]"#).unwrap();
        let query = SearchQuery::new("Happier", "Marshmello", None, None, 214_000);
        let hit = best_match(tracks, &query).unwrap();
        assert_eq!(hit.candidate.id, "2");
        assert_eq!(hit.candidate.recording.artists, ["Marshmello", "Bastille"]);
        assert!(hit.synced.is_none());
        assert_eq!(hit.plain.as_deref(), Some("Correct words"));
    }

    #[test]
    fn removes_a_soundtrack_and_year_from_crowdsourced_artist_credits() {
        let track: LrclibTrack = serde_json::from_str(
            r#"{
          "id": 1, "trackName": "Podimeesha",
          "artistName": "P. Jayachandran / Pa Va / 2016",
          "albumName": "Pa Va", "duration": 195
        }"#,
        )
        .unwrap();
        let query = SearchQuery::new(
            "Podimeesha",
            "P. Jayachandran",
            Some("Pa Va"),
            None,
            195_000,
        );
        assert_eq!(
            track.into_match(&query).candidate.recording.artists,
            ["P. Jayachandran"]
        );
    }
}
