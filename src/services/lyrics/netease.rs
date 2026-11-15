use super::LyricMatch;
use crate::services::matching::{
    duration_delta, rank_candidates, Candidate, Recording, SearchQuery, Source,
};
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use std::sync::LazyLock;

static TIMESTAMP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\d{1,3}:\d{2}(?:[.:]\d+)?\]").unwrap());

pub(super) struct NetEaseLyricsService {
    client: Client,
}

#[derive(Deserialize)]
struct SearchResponse {
    result: Option<SearchResult>,
}

#[derive(Deserialize)]
struct SearchResult {
    #[serde(default)]
    songs: Vec<Song>,
}

#[derive(Deserialize)]
struct Song {
    id: i64,
    name: String,
    #[serde(default, alias = "ar")]
    artists: Vec<Name>,
    #[serde(alias = "al")]
    album: Option<Name>,
    #[serde(alias = "dt")]
    duration: Option<i64>,
}

#[derive(Deserialize)]
struct Name {
    name: String,
}

#[derive(Deserialize)]
struct LyricsResponse {
    lrc: Option<Lrc>,
}

#[derive(Deserialize)]
struct Lrc {
    lyric: String,
}

impl NetEaseLyricsService {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn find(&self, query: &SearchQuery) -> Option<LyricMatch> {
        let result: SearchResponse = self
            .client
            .get("https://music.163.com/api/search/get/web")
            .query(&[
                ("s", query.term().as_str()),
                ("type", "1"),
                ("offset", "0"),
                ("limit", "12"),
            ])
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .await
            .ok()?;
        let candidates: Vec<_> = result
            .result?
            .songs
            .into_iter()
            .map(|song| {
                Candidate::new(
                    Source::NetEase,
                    song.id.to_string(),
                    Recording {
                        title: song.name,
                        artists: song.artists.into_iter().map(|a| a.name).collect(),
                        album: song.album.map(|a| a.name),
                        duration_ms: song.duration,
                        ..Recording::default()
                    },
                )
            })
            .collect();
        let candidate = rank_candidates(query, &candidates).into_iter().next()?;
        let text = self.fetch_lyrics(&candidate.id).await;
        let timed = text.as_deref().is_some_and(|lrc| TIMESTAMP.is_match(lrc));
        let same_duration = query.duration_ms.is_none()
            || duration_delta(query.duration_ms, candidate.recording.duration_ms)
                .is_some_and(|d| d <= 3_000);
        let plain = text
            .as_deref()
            .map(|lrc| {
                lrc.lines()
                    .filter_map(|line| {
                        let line = TIMESTAMP.replace_all(line, "");
                        let clean = line.trim();
                        (!clean.is_empty() && !clean.starts_with('[')).then(|| clean.to_owned())
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .filter(|text| !text.is_empty());
        Some(LyricMatch {
            candidate,
            synced: text.filter(|_| timed && same_duration),
            plain,
            instrumental: false,
        })
    }

    async fn fetch_lyrics(&self, id: &str) -> Option<String> {
        let result: LyricsResponse = self
            .client
            .get("https://music.163.com/api/song/lyric")
            .query(&[("id", id), ("lv", "1")])
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .await
            .ok()?;
        Some(result.lrc?.lyric)
    }
}
