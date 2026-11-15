use super::cleaner::{clean_track, split_artists};
use super::matching::{add_names, normalize, score, Candidate, Recording, SearchQuery, Source};
use serde::Deserialize;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::time::timeout;

pub(super) struct WebSearchService {
    workers: Semaphore,
}

#[derive(Deserialize)]
struct YouTubeEntry {
    id: String,
    title: Option<String>,
    track: Option<String>,
    #[serde(default)]
    artists: Vec<String>,
    artist: Option<String>,
    album: Option<String>,
    release_year: Option<i32>,
    track_number: Option<i32>,
    duration: Option<f64>,
    genre: Option<String>,
    #[serde(default)]
    composers: Vec<String>,
    composer: Option<String>,
    description: Option<String>,
    channel_is_verified: Option<bool>,
}

impl YouTubeEntry {
    fn into_candidate(self) -> Option<Candidate> {
        let description = self.description.as_deref().unwrap_or("");
        let structured =
            self.track.is_some() && (!self.artists.is_empty() || self.artist.is_some());
        let verified = self.channel_is_verified == Some(true);
        if !structured && !verified {
            return None;
        }

        let mut artists = self.artists;
        if artists.is_empty() {
            if let Some(artist) = self.artist {
                artists = split_artists(&artist);
            }
        }
        let mut album = self.album;
        let mut composers = self.composers;
        if let Some(composer) = self.composer {
            add_names(&mut composers, &split_artists(&composer));
        }
        let mut producers = Vec::new();
        let mut labelled_title = None;
        for line in description.lines().take(80) {
            let label_context = line.trim_start().to_ascii_lowercase();
            let film_credit = label_context.starts_with("movie director")
                || label_context.starts_with("film director");
            for segment in line.split('|') {
                let Some((label, value)) = segment.split_once(':') else {
                    continue;
                };
                let value = value.trim();
                if value.is_empty() || value.chars().count() > 120 || value.contains("http") {
                    continue;
                }
                match label.trim().to_ascii_lowercase().as_str() {
                    "song" | "song name" | "song title" | "track" if verified => {
                        labelled_title.get_or_insert_with(|| value.to_owned());
                    }
                    "film" | "movie" | "album" | "soundtrack" if verified => {
                        album.get_or_insert_with(|| short_film_name(value, self.title.as_deref()));
                    }
                    "singer" | "singers" | "sung by" | "playback singer" | "vocals" | "artist"
                    | "artists"
                        if !structured && verified =>
                    {
                        add_names(&mut artists, &split_artists(value))
                    }
                    "composer" | "composed by" | "music director" | "music by" | "lyricist"
                    | "lyrics by" | "writer" | "composer lyricist" => {
                        add_names(&mut composers, &split_artists(value))
                    }
                    "music" | "lyrics" if verified => {
                        add_names(&mut composers, &split_artists(value))
                    }
                    "music producer" => add_names(&mut producers, &split_artists(value)),
                    "producer" | "produced by" if !film_credit => {
                        add_names(&mut producers, &split_artists(value))
                    }
                    _ => {}
                }
            }
        }

        let title = self.track.or(labelled_title).or(self.title)?;
        let clean = clean_track(&title, &artists.join(" / "));
        if artists.is_empty() {
            // A verified upload can embed the credits as "Artist - Track".
            artists = clean.extracted_artists;
        }
        if artists.is_empty() {
            return None;
        }

        Some(Candidate::new(
            Source::YouTube,
            self.id,
            Recording {
                title: clean.cleaned_title,
                artists,
                album,
                year: self.release_year.filter(|y| (1000..=2100).contains(y)),
                track_number: self.track_number,
                duration_ms: self.duration.map(|s| (s * 1000.0).round() as i64),
                genre: self.genre,
                composers,
                producers,
                ..Recording::default()
            },
        ))
    }
}

fn short_film_name(value: &str, video_title: Option<&str>) -> String {
    if let Some((name, _)) = value.split_once('(') {
        let name = name.trim();
        if video_title.is_some_and(|title| {
            title
                .split('|')
                .skip(1)
                .any(|part| normalize(part) == normalize(name))
        }) {
            return name.to_owned();
        }
    }
    value.to_owned()
}

impl WebSearchService {
    pub fn new() -> Self {
        Self {
            workers: Semaphore::new(2),
        }
    }

    pub async fn search(&self, query: &SearchQuery) -> Vec<Candidate> {
        let Ok(Ok(_permit)) = timeout(Duration::from_millis(700), self.workers.acquire()).await
        else {
            return Vec::new();
        };
        let target = match &query.youtube_id {
            Some(id) => format!("https://www.youtube.com/watch?v={id}"),
            None => format!("ytsearch3:{}", query.term()),
        };
        let Ok(mut child) = Command::new("yt-dlp")
            .args([
                "--ignore-config",
                "--skip-download",
                "--dump-json",
                "--no-warnings",
                "--no-playlist",
                "--ignore-no-formats-error",
                "--socket-timeout",
                "2",
                "--extractor-retries",
                "0",
                "--retries",
                "0",
                "--",
            ])
            .arg(target)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
        else {
            return Vec::new();
        };

        let mut lines = BufReader::new(child.stdout.take().expect("piped yt-dlp output")).lines();
        let mut candidates = Vec::new();
        // Keep any completed results if the extractor stalls on another video.
        let _ = timeout(Duration::from_secs(5), async {
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(candidate) = serde_json::from_str::<YouTubeEntry>(&line)
                    .ok()
                    .and_then(YouTubeEntry::into_candidate)
                {
                    let confirmed = query.youtube_id.as_deref() == Some(candidate.id.as_str())
                        || (score(query, &candidate.recording).is_some()
                            && candidate.recording.album.is_some()
                            && !candidate.recording.composers.is_empty());
                    candidates.push(candidate);
                    if confirmed {
                        return;
                    }
                }
            }
            let _ = child.wait().await;
        })
        .await;
        candidates
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verified_soundtrack_video_can_confirm_regional_credits() {
        let entry: YouTubeEntry = serde_json::from_value(serde_json::json!({
            "id": "DjZPK9yv5B4", "title": "Podimeesha Mulakkana Kaalam | Pa Va | P Jayachandran",
            "duration": 195, "channel_is_verified": true,
            "description": "Song: Podimeesha\nMovie: Pa Va (Pappanekkurichum Varkeyekkurichum)\nSinger: P. Jayachandran\nLyrics : Santhosh Varma | Music : Anand Madhusoodanan\nMovie Director : Sooraj Tom | Producer : Siyad Muhammed"
        })).unwrap();
        let track = entry.into_candidate().unwrap().recording;
        let query = SearchQuery::new(
            "Podimeesha Mulakkana Kaalam",
            "Pa Va, P Jayachandran",
            None,
            None,
            195_000,
        );
        assert!(crate::services::matching::score(&query, &track).is_some());
        assert_eq!(track.artists, ["P. Jayachandran"]);
        assert_eq!(track.composers, ["Santhosh Varma", "Anand Madhusoodanan"]);
        assert_eq!(track.album.as_deref(), Some("Pa Va"));
        assert!(track.producers.is_empty());
    }

    #[test]
    fn ignores_credit_lines_on_unverified_reuploads() {
        let entry: YouTubeEntry = serde_json::from_str(
            r#"{
            "id":"example", "title":"Happier", "description":"Singer: Another Artist"
        }"#,
        )
        .unwrap();
        assert!(entry.into_candidate().is_none());
    }
}
