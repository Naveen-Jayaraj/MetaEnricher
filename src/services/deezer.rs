use super::matching::{add_names, year_from_date, Candidate, Recording, SearchQuery, Source};
use reqwest::Client;
use serde::Deserialize;

pub(super) struct DeezerService {
    client: Client,
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    data: Vec<Track>,
}

#[derive(Deserialize)]
struct Track {
    id: i64,
    title: String,
    duration: Option<i64>,
    track_position: Option<i32>,
    artist: Option<Artist>,
    album: Option<Album>,
    contributors: Option<Vec<Contributor>>,
}

#[derive(Deserialize)]
struct Artist {
    name: String,
}

#[derive(Deserialize)]
struct Album {
    id: i64,
    title: String,
    cover_xl: Option<String>,
    cover_big: Option<String>,
    cover_medium: Option<String>,
}

#[derive(Deserialize)]
struct Contributor {
    name: String,
    role: Option<String>,
}

#[derive(Deserialize)]
struct TrackDetails {
    release_date: Option<String>,
    track_position: Option<i32>,
    contributors: Option<Vec<Contributor>>,
}

#[derive(Deserialize)]
struct AlbumDetails {
    genres: Option<Genres>,
}

#[derive(Deserialize)]
struct Genres {
    data: Vec<Genre>,
}

#[derive(Deserialize)]
struct Genre {
    name: String,
}

fn add_contributors(artists: &mut Vec<String>, contributors: Option<Vec<Contributor>>) {
    let performers: Vec<_> = contributors
        .into_iter()
        .flatten()
        .filter(|c| {
            matches!(
                c.role.as_deref(),
                None | Some("Main" | "Featured" | "Artist")
            )
        })
        .map(|c| c.name)
        .collect();
    add_names(artists, &performers);
}

impl Track {
    fn into_candidate(self) -> Candidate {
        let mut artists: Vec<_> = self.artist.into_iter().map(|a| a.name).collect();
        add_contributors(&mut artists, self.contributors);
        let mut candidate = Candidate::new(
            Source::Deezer,
            self.id.to_string(),
            Recording {
                title: self.title,
                artists,
                duration_ms: self.duration.and_then(|d| d.checked_mul(1000)),
                track_number: self.track_position,
                ..Recording::default()
            },
        );
        if let Some(album) = self.album {
            candidate.release_id = Some(album.id.to_string());
            candidate.recording.album = Some(album.title);
            candidate.recording.artwork = album.cover_xl.or(album.cover_big).or(album.cover_medium);
        }
        candidate
    }
}

impl DeezerService {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn search(&self, query: &SearchQuery, alternate: bool) -> Vec<Candidate> {
        let term = if alternate {
            let alt = query.alternate_term();
            if alt == query.term() {
                format!("track:\"{}\"", query.title.replace('"', " "))
            } else {
                alt
            }
        } else {
            query.term()
        };
        self.search_term(&term).await.unwrap_or_default()
    }

    async fn search_term(&self, term: &str) -> Option<Vec<Candidate>> {
        let response: SearchResponse = self
            .client
            .get("https://api.deezer.com/search")
            .query(&[("q", term), ("limit", "16")])
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
                .data
                .into_iter()
                .map(Track::into_candidate)
                .collect(),
        )
    }

    pub async fn add_details(&self, candidate: &mut Candidate, need_genre: bool) {
        let album_id = candidate.release_id.as_deref().filter(|_| need_genre);
        let (details, genre) = tokio::join!(
            self.track_details(&candidate.id),
            self.album_genre(album_id)
        );
        if let Some(details) = details {
            add_contributors(&mut candidate.recording.artists, details.contributors);
            candidate.recording.year = details.release_date.as_deref().and_then(year_from_date);
            candidate.recording.track_number =
                details.track_position.or(candidate.recording.track_number);
        }
        candidate.recording.genre = genre;
    }

    async fn track_details(&self, id: &str) -> Option<TrackDetails> {
        self.client
            .get(format!("https://api.deezer.com/track/{id}"))
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .await
            .ok()
    }

    async fn album_genre(&self, id: Option<&str>) -> Option<String> {
        let id = id?;
        let details: AlbumDetails = self
            .client
            .get(format!("https://api.deezer.com/album/{id}"))
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .await
            .ok()?;
        details.genres?.data.into_iter().next().map(|g| g.name)
    }
}
