#![allow(dead_code)]

use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct NetEaseLyricsService {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct NetEaseSearchResponse {
    result: Option<NetEaseSearchResult>,
}

#[derive(Debug, Deserialize)]
struct NetEaseSearchResult {
    songs: Option<Vec<NetEaseSongItem>>,
}

#[derive(Debug, Deserialize)]
pub struct NetEaseSongItem {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Deserialize)]
struct NetEaseLyricResponse {
    lrc: Option<NetEaseLrcObject>,
    tlyric: Option<NetEaseLrcObject>,
}

#[derive(Debug, Deserialize)]
struct NetEaseLrcObject {
    lyric: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NetEaseResult {
    pub synced_lyrics: Option<String>,
    pub plain_lyrics: Option<String>,
    pub source: String,
}

impl NetEaseLyricsService {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client }
    }

    pub async fn fetch_lyrics(&self, title: &str, artist: &str) -> Option<NetEaseResult> {
        let query = format!("{} {}", title, artist);
        let search_url = format!(
            "https://music.163.com/api/search/get/web?s={}&type=1&offset=0&limit=3",
            urlencoding::encode(&query)
        );

        let search_resp = self.client.get(&search_url).send().await.ok()?;
        let search_json: NetEaseSearchResponse = search_resp.json().await.ok()?;
        let songs = search_json.result?.songs?;
        let song_id = songs.into_iter().next()?.id;

        let lyric_url = format!(
            "https://music.163.com/api/song/lyric?id={}&lv=1&kv=1&tv=-1",
            song_id
        );

        let lyric_resp = self.client.get(&lyric_url).send().await.ok()?;
        let lyric_json: NetEaseLyricResponse = lyric_resp.json().await.ok()?;
        let raw_lrc = lyric_json.lrc?.lyric?;

        if raw_lrc.trim().is_empty() {
            return None;
        }

        // Check if contains timestamp [00:
        let has_timestamps = raw_lrc.contains("[00:") || raw_lrc.contains("[01:") || raw_lrc.contains("[02:");

        let (synced, plain) = if has_timestamps {
            let plain = strip_lrc_timestamps(&raw_lrc);
            (Some(raw_lrc), Some(plain))
        } else {
            (None, Some(raw_lrc))
        };

        Some(NetEaseResult {
            synced_lyrics: synced,
            plain_lyrics: plain,
            source: "NetEase".to_string(),
        })
    }
}

fn strip_lrc_timestamps(lrc: &str) -> String {
    let mut lines = Vec::new();
    for line in lrc.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.contains(']') {
            let idx = trimmed.rfind(']').unwrap();
            let text = trimmed[idx + 1..].trim();
            if !text.is_empty() {
                lines.push(text);
            }
        } else if !trimmed.is_empty() {
            lines.push(trimmed);
        }
    }
    lines.join("\n")
}
