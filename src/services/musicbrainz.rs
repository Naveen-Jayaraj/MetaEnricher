use super::matching::{
    add_names, same_album, year_from_date, Candidate, Recording, SearchQuery, Source,
};
use reqwest::{header, Client, RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::{sleep_until, Instant};

pub(super) struct MusicBrainzService {
    client: Client,
    next_request: Mutex<Instant>,
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    recordings: Vec<RecordingItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RecordingItem {
    id: String,
    title: String,
    length: Option<i64>,
    first_release_date: Option<String>,
    #[serde(default)]
    artist_credit: Vec<Name>,
    #[serde(default)]
    aliases: Vec<Name>,
    #[serde(default)]
    releases: Vec<Release>,
    #[serde(default)]
    tags: Vec<Tag>,
    #[serde(default)]
    genres: Vec<Tag>,
    #[serde(default)]
    relations: Vec<Relation>,
}

#[derive(Deserialize)]
struct Name {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Release {
    id: String,
    title: String,
    date: Option<String>,
    status: Option<String>,
    release_group: Option<ReleaseGroup>,
    #[serde(default)]
    media: Vec<Medium>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ReleaseGroup {
    #[serde(default)]
    secondary_types: Vec<String>,
}

#[derive(Deserialize)]
struct Medium {
    #[serde(default)]
    track: Vec<TrackPosition>,
}

#[derive(Deserialize)]
struct TrackPosition {
    number: Option<String>,
    position: Option<i32>,
}

#[derive(Deserialize)]
struct Tag {
    name: String,
    count: i32,
}

#[derive(Deserialize)]
struct Relation {
    r#type: String,
    artist: Option<Name>,
    work: Option<Work>,
}

#[derive(Deserialize)]
struct Work {
    #[serde(default)]
    languages: Vec<String>,
    #[serde(default)]
    relations: Vec<Relation>,
}

impl RecordingItem {
    fn into_candidate(self, query: &SearchQuery) -> Candidate {
        let mut candidate = Candidate::new(
            Source::MusicBrainz,
            self.id,
            Recording {
                title: self.title,
                artists: self
                    .artist_credit
                    .into_iter()
                    .map(|credit| credit.name)
                    .collect(),
                aliases: self.aliases.into_iter().map(|alias| alias.name).collect(),
                duration_ms: self.length.filter(|ms| *ms >= 1_000),
                year: self.first_release_date.as_deref().and_then(year_from_date),
                ..Recording::default()
            },
        );
        let release = self
            .releases
            .into_iter()
            .filter(|r| {
                r.status
                    .as_deref()
                    .is_none_or(|status| status == "Official")
            })
            .min_by_key(|r| {
                let matches_album = same_album(query.album.as_deref(), Some(&r.title));
                let compilation = r.release_group.as_ref().is_some_and(|group| {
                    group
                        .secondary_types
                        .iter()
                        .any(|kind| kind == "Compilation")
                });
                let year = r.date.as_deref().and_then(year_from_date);
                let same_title = same_album(Some(&candidate.recording.title), Some(&r.title));
                (
                    !matches_album,
                    compilation,
                    year.unwrap_or(i32::MAX),
                    !same_title,
                    r.date.clone(),
                )
            });
        if let Some(release) = release {
            candidate.release_id = Some(release.id);
            candidate.recording.album = Some(release.title);
            candidate.recording.year = release
                .date
                .as_deref()
                .and_then(year_from_date)
                .or(candidate.recording.year);
            candidate.recording.track_number = release
                .media
                .iter()
                .flat_map(|m| &m.track)
                .filter_map(|t| {
                    t.number
                        .as_deref()
                        .and_then(|number| number.parse().ok())
                        .or(t.position)
                })
                .find(|n| *n > 0);
        }
        apply_credits_and_tags(
            &mut candidate.recording,
            self.relations,
            self.genres,
            self.tags,
        );
        candidate
    }
}

fn apply_credits_and_tags(
    recording: &mut Recording,
    relations: Vec<Relation>,
    genres: Vec<Tag>,
    tags: Vec<Tag>,
) {
    if let Some(genre) = genres
        .into_iter()
        .filter(|tag| tag.count > 0)
        .max_by_key(|tag| tag.count)
    {
        recording.genre = Some(genre.name);
    }
    add_names(
        &mut recording.tags,
        &tags
            .into_iter()
            .filter(|t| t.count > 0)
            .map(|t| t.name)
            .collect::<Vec<_>>(),
    );

    let mut languages = Vec::new();
    for relation in relations {
        if relation.r#type == "producer" {
            if let Some(artist) = relation.artist {
                add_names(&mut recording.producers, &[artist.name]);
            }
        }
        if let Some(work) = relation.work {
            languages.extend(
                work.languages
                    .iter()
                    .filter_map(|language| work_language(language))
                    .map(str::to_owned),
            );
            for credit in work.relations {
                if matches!(credit.r#type.as_str(), "composer" | "writer" | "lyricist") {
                    if let Some(artist) = credit.artist {
                        add_names(&mut recording.composers, &[artist.name]);
                    }
                }
            }
        }
    }
    languages.sort_unstable();
    languages.dedup();
    if !languages.is_empty() {
        recording.language = Some(languages.join(" / "));
    }
}

impl MusicBrainzService {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            next_request: Mutex::new(Instant::now()),
        }
    }

    pub async fn search(&self, query: &SearchQuery, alternate: bool) -> Vec<Candidate> {
        self.search_recording(query, alternate)
            .await
            .unwrap_or_default()
    }

    async fn search_recording(
        &self,
        query: &SearchQuery,
        alternate: bool,
    ) -> Option<Vec<Candidate>> {
        let title = phrase(&query.title);
        let mut expression = format!("(recording:{title} OR alias:{title}) AND NOT video:true");
        if alternate {
            if let Some(swapped) = query.swapped() {
                expression = format!(
                    "({expression}) OR (recording:{} AND artist:{} AND NOT video:true)",
                    phrase(&swapped.title),
                    phrase(swapped.artist())
                );
            }
        } else {
            let mut hints: Vec<_> = query
                .artists
                .iter()
                .take(5)
                .map(|artist| format!("artist:{}", phrase(artist)))
                .collect();
            if let Some(album) = &query.album {
                hints.push(format!("release:{}", phrase(album)));
            }
            if !hints.is_empty() {
                expression.push_str(&format!(" AND ({})", hints.join(" OR ")));
            }
        }
        let request = self
            .client
            .get("https://musicbrainz.org/ws/2/recording")
            .query(&[
                ("query", expression.as_str()),
                ("fmt", "json"),
                ("limit", "14"),
            ]);
        let response: SearchResponse = self.request(request).await?;
        Some(
            response
                .recordings
                .into_iter()
                .map(|r| r.into_candidate(query))
                .collect(),
        )
    }

    pub async fn add_details(&self, candidate: &mut Candidate, need_artwork: bool) {
        let release_id = candidate.release_id.as_deref().filter(|_| need_artwork);
        let (detail, cover) = tokio::join!(self.lookup(&candidate.id), self.cover_art(release_id));
        if let Some(detail) = detail {
            let credits: Vec<_> = detail
                .artist_credit
                .into_iter()
                .map(|credit| credit.name)
                .collect();
            add_names(&mut candidate.recording.artists, &credits);
            apply_credits_and_tags(
                &mut candidate.recording,
                detail.relations,
                detail.genres,
                detail.tags,
            );
        }
        candidate.recording.artwork = cover;
    }

    async fn lookup(&self, id: &str) -> Option<RecordingItem> {
        let request = self
            .client
            .get(format!("https://musicbrainz.org/ws/2/recording/{id}"))
            .query(&[
                ("fmt", "json"),
                (
                    "inc",
                    "artist-credits+artist-rels+work-rels+work-level-rels+genres+tags",
                ),
            ]);
        self.request(request).await
    }

    async fn request<T: DeserializeOwned>(&self, request: RequestBuilder) -> Option<T> {
        self.wait_for_slot().await?;
        let response = request
            .header(header::ACCEPT, "application/json")
            .send()
            .await
            .inspect_err(|error| {
                tracing::debug!(
                    timeout = error.is_timeout(),
                    connect = error.is_connect(),
                    "MusicBrainz request failed"
                )
            })
            .ok()?;
        let status = response.status();
        if matches!(
            status,
            StatusCode::TOO_MANY_REQUESTS | StatusCode::SERVICE_UNAVAILABLE
        ) {
            let delay = response
                .headers()
                .get(header::RETRY_AFTER)
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(3)
                .min(86_400);
            let mut next = self.next_request.lock().await;
            *next = (*next).max(Instant::now() + Duration::from_secs(delay));
        }
        if !status.is_success() {
            tracing::debug!(%status, "MusicBrainz returned an error");
            return None;
        }
        response
            .json()
            .await
            .inspect_err(
                |error| tracing::debug!(%error, "MusicBrainz response could not be decoded"),
            )
            .ok()
    }

    async fn cover_art(&self, release_id: Option<&str>) -> Option<String> {
        let url = format!(
            "https://coverartarchive.org/release/{}/front-1200",
            release_id?
        );
        let response = self
            .client
            .head(&url)
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?;
        response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)?
            .to_str()
            .ok()?
            .starts_with("image/")
            .then_some(url)
    }

    async fn wait_for_slot(&self) -> Option<()> {
        let mut next = self.next_request.lock().await;
        if *next > Instant::now() + Duration::from_millis(1_100) {
            return None;
        }
        let reserved = (*next).max(Instant::now());
        *next = reserved + Duration::from_millis(1_050);
        drop(next);
        sleep_until(reserved).await;
        Some(())
    }
}

fn phrase(input: &str) -> String {
    format!("\"{}\"", input.replace('\\', "\\\\").replace('"', "\\\""))
}

fn work_language(iso: &str) -> Option<&'static str> {
    Some(match iso {
        "eng" => "English",
        "spa" => "Spanish",
        "fra" => "French",
        "deu" => "German",
        "ita" => "Italian",
        "jpn" => "Japanese",
        "kor" => "Korean",
        "zho" => "Chinese",
        "cmn" => "Mandarin",
        "yue" => "Cantonese",
        "hin" => "Hindi",
        "tam" => "Tamil",
        "tel" => "Telugu",
        "mal" => "Malayalam",
        "kan" => "Kannada",
        "ben" => "Bengali",
        "pan" => "Punjabi",
        "por" => "Portuguese",
        "rus" => "Russian",
        "ara" => "Arabic",
        "zxx" => "Instrumental",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_requested_release_and_reads_work_credits() {
        let value = serde_json::json!({
            "id": "happier", "title": "Happier", "length": 214_000,
            "artist-credit": [{"name":"Marshmello"}, {"name":"Bastille"}],
            "releases": [
                {"id":"compilation", "title":"Greatest Dance Hits", "status":"Official", "date":"2020"},
                {"id":"single", "title":"Happier", "status":"Official", "date":"2018-08-17",
                 "media":[{"track":[{"number":"1", "position":1}]}]}
            ],
            "relations": [
                {"type":"producer", "artist":{"name":"Steve Mac"}},
                {"type":"mixer", "artist":{"name":"Manny Marroquin"}},
                {"type":"performance", "work":{"languages":["eng"], "relations":[
                    {"type":"writer", "artist":{"name":"Dan Smith"}}
                ]}}
            ]
        });
        let result: RecordingItem = serde_json::from_value(value).unwrap();
        let query = SearchQuery::new(
            "Happier",
            "Marshmello",
            Some("Happier - Single"),
            None,
            214_000,
        );
        let selected = result.into_candidate(&query);
        assert_eq!(selected.recording.artists, ["Marshmello", "Bastille"]);
        assert_eq!(selected.recording.producers, ["Steve Mac"]);
        assert_eq!(selected.recording.composers, ["Dan Smith"]);
        assert_eq!(selected.recording.language.as_deref(), Some("English"));
        assert_eq!(selected.recording.track_number, Some(1));
        assert_eq!(selected.recording.year, Some(2018));
        assert_eq!(selected.release_id.as_deref(), Some("single"));
    }
}
