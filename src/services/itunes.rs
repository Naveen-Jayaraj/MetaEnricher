use super::matching::{catalog_artists, year_from_date, Candidate, Recording, SearchQuery, Source};
use reqwest::Client;
use serde::Deserialize;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub(super) struct ITunesService {
    client: Client,
    requests: Mutex<VecDeque<Instant>>,
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    results: Vec<Track>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Track {
    track_id: Option<i64>,
    track_name: Option<String>,
    artist_name: Option<String>,
    collection_name: Option<String>,
    primary_genre_name: Option<String>,
    release_date: Option<String>,
    track_number: Option<i32>,
    track_time_millis: Option<i64>,
    artwork_url100: Option<String>,
}

impl Track {
    fn into_candidate(self, query: &SearchQuery) -> Option<Candidate> {
        let title = self.track_name?;
        let artist = self.artist_name?;
        let artwork = self
            .artwork_url100
            .map(|url| url.replace("100x100bb.", "1200x1200bb."));
        Some(Candidate::new(
            Source::ITunes,
            self.track_id?.to_string(),
            Recording {
                title,
                artists: catalog_artists(&artist, query),
                album: self.collection_name,
                year: self.release_date.as_deref().and_then(year_from_date),
                track_number: self.track_number,
                duration_ms: self.track_time_millis,
                genre: self.primary_genre_name,
                artwork,
                ..Recording::default()
            },
        ))
    }
}

impl ITunesService {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            requests: Mutex::new(VecDeque::new()),
        }
    }

    pub async fn search(&self, query: &SearchQuery, alternate: bool) -> Vec<Candidate> {
        let country = preferred_storefront(query);
        let (term, country) = if alternate {
            (
                query.alternate_term(),
                if country == "US" { "IN" } else { "US" },
            )
        } else {
            (query.term(), country)
        };
        self.search_storefront(query, &term, country)
            .await
            .unwrap_or_default()
    }

    async fn search_storefront(
        &self,
        query: &SearchQuery,
        term: &str,
        country: &str,
    ) -> Option<Vec<Candidate>> {
        {
            let mut requests = self.requests.lock().expect("iTunes request limiter");
            let now = Instant::now();
            while requests
                .front()
                .is_some_and(|start| now.duration_since(*start) >= Duration::from_secs(60))
            {
                requests.pop_front();
            }
            // The Search API allows approximately 20 calls per minute. Other providers can
            // still answer immediately when this storefront's request budget is exhausted.
            if requests.len() >= 20 {
                return None;
            }
            requests.push_back(now);
        }
        let response: SearchResponse = self
            .client
            .get("https://itunes.apple.com/search")
            .query(&[
                ("term", term),
                ("country", country),
                ("media", "music"),
                ("entity", "song"),
                ("limit", "16"),
            ])
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .await
            .ok()?;
        Some(
            response
                .results
                .into_iter()
                .filter_map(|item| item.into_candidate(query))
                .collect(),
        )
    }
}

fn preferred_storefront(query: &SearchQuery) -> &'static str {
    let names = std::iter::once(&query.title).chain(&query.artists);
    for c in names.flat_map(|s| s.chars()) {
        match c {
            '\u{0900}'..='\u{0d7f}' => return "IN",
            '\u{3040}'..='\u{30ff}' => return "JP",
            '\u{ac00}'..='\u{d7af}' => return "KR",
            _ => {}
        }
    }
    "US"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::matching::rank_candidates;

    #[test]
    fn ranks_all_apple_results_and_uses_the_complete_credit() {
        let response: SearchResponse = serde_json::from_str(r#"{"results":[
            {"trackId":1,"trackName":"Happier","artistName":"Olivia Rodrigo","trackTimeMillis":175000},
            {"trackId":2,"trackName":"Happier (Stripped)","artistName":"Marshmello & Bastille","trackTimeMillis":155000},
            {"trackId":3,"trackName":"Happier","artistName":"Marshmello & Bastille","collectionName":"Happier - Single","releaseDate":"2018-08-16T12:00:00Z","trackNumber":1,"trackTimeMillis":214290}
        ]}"#).unwrap();
        let query = SearchQuery::new("happier", "marshmello", None, None, 214_000);
        let options: Vec<_> = response
            .results
            .into_iter()
            .filter_map(|r| r.into_candidate(&query))
            .collect();
        let selected = rank_candidates(&query, &options);
        assert_eq!(selected[0].id, "3");
        assert_eq!(selected[0].recording.artists, ["Marshmello", "Bastille"]);
    }
}
