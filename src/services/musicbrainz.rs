#![allow(dead_code)]

use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct MusicBrainzService {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct MBRecordingResponse {
    recordings: Option<Vec<MBRecordingItem>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MBRecordingItem {
    pub id: String,
    pub title: String,
    pub length: Option<i64>,
    pub artist_credit: Option<Vec<MBArtistCredit>>,
    pub releases: Option<Vec<MBReleaseItem>>,
    pub relations: Option<Vec<MBRelationItem>>,
    pub tags: Option<Vec<MBTagItem>>,
    pub genres: Option<Vec<MBGenreItem>>,
}

#[derive(Debug, Deserialize)]
pub struct MBArtistCredit {
    pub name: String,
    pub joinphrase: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MBReleaseItem {
    pub id: String,
    pub title: String,
    pub date: Option<String>,
    pub country: Option<String>,
    pub text_representation: Option<MBTextRepresentation>,
    pub media: Option<Vec<MBMediaItem>>,
}

#[derive(Debug, Deserialize)]
pub struct MBTextRepresentation {
    pub language: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MBMediaItem {
    pub track_offset: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MBRelationItem {
    pub r#type: Option<String>,
    pub target_type: Option<String>,
    pub artist: Option<MBRelationArtist>,
}

#[derive(Debug, Deserialize)]
pub struct MBRelationArtist {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct MBTagItem {
    pub name: String,
    pub count: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct MBGenreItem {
    pub name: String,
}

#[derive(Debug, Clone, Default)]
pub struct MusicBrainzEnrichedData {
    pub canonical_title: Option<String>,
    pub artists: Vec<String>,
    pub album_title: Option<String>,
    pub release_year: Option<i32>,
    pub track_number: Option<i32>,
    pub composers: Vec<String>,
    pub producers: Vec<String>,
    pub language: Option<String>,
    pub tags: Vec<String>,
    pub duration_ms: Option<i64>,
}

impl MusicBrainzService {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(6))
            .user_agent("MetaEnricher/1.0 (https://github.com/Naveen-Jayaraj/OfflineMusic)")
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client }
    }

    pub async fn search_recording(&self, title: &str, artist: &str) -> Option<MusicBrainzEnrichedData> {
        let lucene_query = format!(
            r#"recording:"{}" AND artist:"{}""#,
            title.replace('"', "\\\""),
            artist.replace('"', "\\\"")
        );

        let url = format!(
            "https://musicbrainz.org/ws/2/recording?query={}&fmt=json&limit=3&inc=artist-credits+releases+work-rels+artist-rels+genres+tags",
            urlencoding::encode(&lucene_query)
        );

        let resp = self.client.get(&url).send().await.ok()?;
        let mb_res: MBRecordingResponse = resp.json().await.ok()?;
        let recording = mb_res.recordings?.into_iter().next()?;

        let mut data = MusicBrainzEnrichedData {
            canonical_title: Some(recording.title),
            duration_ms: recording.length,
            ..Default::default()
        };

        if let Some(credits) = recording.artist_credit {
            for ac in credits {
                let name = ac.name.trim().to_string();
                if !name.is_empty() && !data.artists.contains(&name) {
                    data.artists.push(name);
                }
            }
        }

        if let Some(releases) = recording.releases {
            if let Some(first_release) = releases.into_iter().next() {
                data.album_title = Some(first_release.title);
                if let Some(d) = first_release.date {
                    if let Some(yr_str) = d.split('-').next() {
                        if let Ok(yr) = yr_str.parse::<i32>() {
                            if yr > 1900 && yr < 2100 {
                                data.release_year = Some(yr);
                            }
                        }
                    }
                }
                if let Some(tr) = first_release.text_representation {
                    if let Some(lang) = tr.language {
                        data.language = Some(map_iso_language(&lang));
                    }
                }
            }
        }

        if let Some(rels) = recording.relations {
            for rel in rels {
                let rel_type = rel.r#type.as_deref().unwrap_or("").to_lowercase();
                if let Some(art) = rel.artist {
                    let name = art.name.trim().to_string();
                    if rel_type.contains("composer") || rel_type.contains("writer") || rel_type.contains("lyricist") {
                        if !data.composers.contains(&name) {
                            data.composers.push(name);
                        }
                    } else if rel_type.contains("producer") || rel_type.contains("programming") || rel_type.contains("engineer") {
                        if !data.producers.contains(&name) {
                            data.producers.push(name);
                        }
                    }
                }
            }
        }

        if let Some(tags) = recording.tags {
            for t in tags {
                data.tags.push(t.name.to_lowercase());
            }
        }
        if let Some(genres) = recording.genres {
            for g in genres {
                let gn = g.name.to_lowercase();
                if !data.tags.contains(&gn) {
                    data.tags.push(gn);
                }
            }
        }

        Some(data)
    }
}

fn map_iso_language(iso: &str) -> String {
    match iso.to_lowercase().as_str() {
        "eng" | "en" => "English".to_string(),
        "spa" | "es" => "Spanish".to_string(),
        "fra" | "fr" => "French".to_string(),
        "deu" | "ger" | "de" => "German".to_string(),
        "ita" | "it" => "Italian".to_string(),
        "jpn" | "ja" => "Japanese".to_string(),
        "kor" | "ko" => "Korean".to_string(),
        "zho" | "chi" | "zh" => "Chinese".to_string(),
        "hin" | "hi" => "Hindi".to_string(),
        "tam" | "ta" => "Tamil".to_string(),
        "tel" | "te" => "Telugu".to_string(),
        "por" | "pt" => "Portuguese".to_string(),
        "rus" | "ru" => "Russian".to_string(),
        "ara" | "ar" => "Arabic".to_string(),
        "zxx" => "Instrumental".to_string(),
        other => other.to_string(),
    }
}
