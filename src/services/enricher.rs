use crate::models::{EnrichmentAudit, SingleSongResponse, TrackEntity};
use crate::services::cleaner::clean_catalog_title;
use crate::services::deezer::DeezerService;
use crate::services::itunes::ITunesService;
use crate::services::lyrics::{LyricMatch, LyricsAggregator};
use crate::services::matching::{
    add_names, album_key, duration_delta, has_exact_title, normalize, rank_candidates, same_album,
    same_recording, score, Candidate, Recording, SearchQuery, Source,
};
use crate::services::musicbrainz::MusicBrainzService;
use crate::services::web_search::WebSearchService;
use crate::services::DatabaseService;
use moka::future::Cache;
use moka::Expiry;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct MetadataEnricher {
    deezer: DeezerService,
    itunes: ITunesService,
    musicbrainz: MusicBrainzService,
    web_search: WebSearchService,
    lyrics: Arc<LyricsAggregator>,
    cache: Cache<SearchQuery, Arc<Discovery>>,
    db: Option<Arc<DatabaseService>>,
}

#[derive(Default, Clone)]
pub(crate) struct Discovery {
    pub recording: Option<Recording>,
    pub sources: Vec<String>,
    pub confidence: f32,
    pub lyrics: Option<Arc<LyricMatch>>,
}

struct DiscoveryExpiry;

impl Expiry<SearchQuery, Arc<Discovery>> for DiscoveryExpiry {
    fn expire_after_create(
        &self,
        _key: &SearchQuery,
        value: &Arc<Discovery>,
        _created_at: Instant,
    ) -> Option<Duration> {
        let seconds = if value.recording.is_none() {
            15
        } else {
            120 // Ephemeral in-memory TTL on Render; Neon DB is the permanent storage
        };
        Some(Duration::from_secs(seconds))
    }
}

impl MetadataEnricher {
    pub fn new(lyrics: Arc<LyricsAggregator>, db: Option<Arc<DatabaseService>>) -> Self {
        let client = super::http_client();
        Self {
            deezer: DeezerService::new(client.clone()),
            itunes: ITunesService::new(client.clone()),
            musicbrainz: MusicBrainzService::new(client),
            web_search: WebSearchService::new(),
            lyrics,
            cache: Cache::builder()
                .max_capacity(10_000)
                .expire_after(DiscoveryExpiry)
                .build(),
            db,
        }
    }

    pub async fn enrich_song(&self, track: TrackEntity) -> SingleSongResponse {
        let mut query = SearchQuery::new(
            &track.title,
            &track.artist,
            track.album.as_deref(),
            track.year,
            track.duration_ms,
        );
        query.set_youtube_id(track.youtube_id.as_deref());

        let discovery = if query.title.is_empty() {
            Arc::new(Discovery::default())
        } else {
            // 1. Check in-memory Moka cache (L1)
            if let Some(cached) = self.cache.get(&query.key()).await {
                cached
            } else {
                // 2. Check Neon PostgreSQL database cache (L2)
                let db_cached = if let Some(db) = &self.db {
                    db.get_metadata(&query).await.map(Arc::new)
                } else {
                    None
                };

                if let Some(cached) = db_cached {
                    self.cache.insert(query.key(), cached.clone()).await;
                    cached
                } else {
                    // 3. Search and aggregate across external providers
                    let discovered = self.discover(&query).await;
                    let arc_disc = Arc::new(discovered);

                    // 4. When any metadata is found, cache it to Neon
                    if arc_disc.recording.is_some() {
                        if let Some(db) = &self.db {
                            let db = db.clone();
                            let q = query.clone();
                            let d = arc_disc.clone();
                            tokio::spawn(async move {
                                if let Err(e) = db.save_metadata(&q, &d).await {
                                    tracing::warn!("Failed to cache metadata to Neon: {e}");
                                }
                            });
                        }
                    }

                    self.cache.insert(query.key(), arc_disc.clone()).await;
                    arc_disc
                }
            }
        };
        let response = apply_discovery(track, &discovery);
        if let Some(lyrics) = &discovery.lyrics {
            let song = &response.song;
            let canonical = SearchQuery::new(
                &song.title,
                &song.artist,
                song.album.as_deref(),
                None,
                song.duration_ms,
            );
            self.lyrics.remember(&canonical, Arc::clone(lyrics)).await;
        }
        response
    }

    async fn discover(&self, input: &SearchQuery) -> Discovery {
        let (musicbrainz, deezer, itunes, lrclib) = tokio::join!(
            self.musicbrainz.search(input, false),
            self.deezer.search(input, false),
            self.itunes.search(input, false),
            self.lyrics.find_lrclib(input),
        );
        let mut candidates: Vec<_> = musicbrainz
            .into_iter()
            .chain(deezer)
            .chain(itunes)
            .collect();
        if let Some(hit) = &lrclib {
            candidates.push(hit.candidate.clone());
        }
        let mut matched = rank_candidates(input, &candidates);
        let mut netease = None;
        let mut netease_searched = false;
        let mut query = input.clone();
        let mut known_video = false;

        // Deeper searches run only when the initial catalogs fail to identify the recording.
        if !matched.iter().any(|c| {
            matches!(
                c.source,
                Source::MusicBrainz | Source::Deezer | Source::ITunes
            ) && has_exact_title(input, &c.recording)
        }) {
            netease_searched = !lrclib
                .as_ref()
                .is_some_and(|hit| hit.has_lyrics() || hit.instrumental);
            let (mb, dz, apple, videos, net) = tokio::join!(
                self.musicbrainz.search(input, true),
                self.deezer.search(input, true),
                self.itunes.search(input, true),
                self.web_search.search(input),
                async {
                    if netease_searched {
                        self.lyrics.find_netease(input).await
                    } else {
                        None
                    }
                },
            );
            candidates.extend(mb.into_iter().chain(dz).chain(apple).chain(videos));
            netease = net;
            if let Some(hit) = &netease {
                candidates.push(hit.candidate.clone());
            }
            matched = rank_candidates(input, &candidates);

            if matched.is_empty() {
                if let Some(swapped) = input.swapped() {
                    let reversed = rank_candidates(&swapped, &candidates);
                    if !reversed.is_empty() {
                        matched = reversed;
                        query = swapped;
                    }
                }
            }
            if let Some(video) = candidates.iter().find(|c| {
                c.source == Source::YouTube
                    && input.youtube_id.as_deref() == Some(c.id.as_str())
                    && duration_delta(input.duration_ms, c.recording.duration_ms)
                        .is_none_or(|d| d <= 10_000)
            }) {
                if !matched
                    .iter()
                    .any(|c| same_recording(&video.recording, &c.recording))
                {
                    // The client supplied the exact upload. Its structured music data also works
                    // when the original title or artist is just an uploader's label.
                    matched = vec![video.clone()];
                    known_video = true;
                }
            }
        }

        let Some(winner) = matched.first() else {
            return Discovery::default();
        };
        let recording = winner.recording.clone();
        let score = score(&query, &recording).unwrap_or_default();
        let confidence = if known_video {
            0.95
        } else {
            (score as f32 / 130.0 + (matched.len() - 1).min(3) as f32 * 0.055).min(0.99)
        };

        let mut best_lyrics = [lrclib, netease]
            .into_iter()
            .flatten()
            .filter(|hit| same_recording(&recording, &hit.candidate.recording))
            .max_by_key(|hit| (hit.synced.is_some(), hit.instrumental, hit.has_lyrics()));

        let canonical = SearchQuery::new(
            &recording.title,
            &recording.artists.join(" / "),
            recording.album.as_deref().or(query.album.as_deref()),
            None,
            query
                .duration_ms
                .or(recording.duration_ms)
                .unwrap_or_default(),
        );

        let (_, extra_lyrics) = tokio::join!(self.enrich_details(&query, &mut matched), async {
            if best_lyrics
                .as_ref()
                .is_some_and(|hit| hit.has_lyrics() || hit.instrumental)
            {
                return None;
            }
            let title_or_artist_changed = normalize(&canonical.title) != normalize(&input.title)
                || normalize(canonical.artist()) != normalize(input.artist());
            let (retry, net) = tokio::join!(
                async {
                    if title_or_artist_changed {
                        self.lyrics.find_lrclib(&canonical).await
                    } else {
                        None
                    }
                },
                async {
                    if !netease_searched || title_or_artist_changed {
                        self.lyrics.find_netease(&canonical).await
                    } else {
                        None
                    }
                },
            );
            [retry, net]
                .into_iter()
                .flatten()
                .filter(|hit| same_recording(&recording, &hit.candidate.recording))
                .max_by_key(|hit| (hit.synced.is_some(), hit.instrumental, hit.has_lyrics()))
        },);
        if let Some(hit) = extra_lyrics {
            if !matched.iter().any(|c| c.source == hit.candidate.source) {
                matched.push(hit.candidate.clone());
            }
            best_lyrics = Some(hit);
        }
        let recording = merge_recordings(&query, &matched);
        Discovery {
            recording: Some(recording),
            sources: matched.iter().map(|c| c.source.name().to_string()).collect(),
            confidence,
            lyrics: best_lyrics.map(Arc::new),
        }
    }

    async fn enrich_details(&self, query: &SearchQuery, matches: &mut [Candidate]) {
        let have_genre = matches.iter().any(|c| usable(c.recording.genre.as_deref()));
        let preferred_album = select_release(query, matches).and_then(|r| r.album.as_deref());
        let have_art = matches.iter().any(|c| {
            same_album(preferred_album, c.recording.album.as_deref())
                && usable(c.recording.artwork.as_deref())
        });
        futures::future::join_all(matches.iter_mut().map(|m| async move {
            match m.source {
                Source::MusicBrainz => self.musicbrainz.add_details(m, !have_art).await,
                Source::Deezer => self.deezer.add_details(m, !have_genre).await,
                _ => {}
            }
        }))
        .await;
    }

    pub fn cache_size(&self) -> u64 {
        self.cache.entry_count()
    }

    /// Purges all in-memory metadata entries from Render. Persistent metadata remains in Neon DB.
    pub async fn purge_memory_cache(&self) -> u64 {
        let count = self.cache.entry_count();
        self.cache.invalidate_all();
        count
    }
}

fn select_release<'a>(query: &SearchQuery, matches: &'a [Candidate]) -> Option<&'a Recording> {
    let releases = matches.iter().map(|c| &c.recording).filter(|r| {
        r.album
            .as_ref()
            .is_some_and(|album| !album_key(album).is_empty())
    });
    if query.album.is_some() {
        releases
            .filter(|r| same_album(query.album.as_deref(), r.album.as_deref()))
            .min_by_key(|r| {
                (
                    query.year.is_some_and(|year| Some(year) != r.year),
                    r.year.unwrap_or(i32::MAX),
                )
            })
    } else {
        releases.min_by_key(|r| r.year.unwrap_or(i32::MAX))
    }
}

fn merge_recordings(query: &SearchQuery, matches: &[Candidate]) -> Recording {
    let best = &matches[0].recording;
    let mut merged = Recording {
        title: clean_catalog_title(&best.title),
        duration_ms: matches
            .iter()
            .find_map(|c| c.recording.duration_ms.filter(|d| *d > 0)),
        ..Recording::default()
    };

    // Start with individually credited names, then supplement partial credits from the
    // same recording. An unstructured catalog can represent a band as a list of names.
    let credit_sources = matches
        .iter()
        .filter(|m| m.source.has_structured_credits())
        .chain(
            matches
                .iter()
                .filter(|m| !m.source.has_structured_credits()),
        );
    let mut role_credits = Vec::new();
    for source in matches
        .iter()
        .filter(|source| source.source.has_structured_credits())
    {
        add_names(&mut role_credits, &source.recording.composers);
        add_names(&mut role_credits, &source.recording.producers);
    }
    for candidate in credit_sources {
        let credits = &candidate.recording.artists;
        let existing_credit = normalize(&merged.artists.join(" "));
        let new_credit = normalize(&credits.join(" "));
        if existing_credit != new_credit {
            let names: Vec<_> = credits
                .iter()
                .filter(|name| {
                    candidate.source.has_structured_credits()
                        || !role_credits
                            .iter()
                            .any(|credit| normalize(credit) == normalize(name))
                })
                .cloned()
                .collect();
            add_names(&mut merged.artists, &names);
        }
    }

    if let Some(release) = select_release(query, matches) {
        merged.album = release.album.clone();
        let same_edition = |c: &&Candidate| {
            same_album(merged.album.as_deref(), c.recording.album.as_deref())
                && !release
                    .year
                    .zip(c.recording.year)
                    .is_some_and(|(a, b)| a.abs_diff(b) > 1)
        };
        merged.year = release.year;
        merged.track_number = release.track_number.filter(|n| *n > 0).or_else(|| {
            matches
                .iter()
                .filter(same_edition)
                .find_map(|c| c.recording.track_number.filter(|n| *n > 0))
        });
        merged.artwork = matches.iter().filter(same_edition).find_map(|c| {
            c.recording
                .artwork
                .as_ref()
                .filter(|a| usable(Some(a)))
                .cloned()
        });
    } else if query.album.is_none() {
        merged.year = matches.iter().filter_map(|c| c.recording.year).min();
    }

    merged.genre = matches
        .iter()
        .filter(|c| usable(c.recording.genre.as_deref()))
        .min_by_key(|c| {
            let on_release = same_album(merged.album.as_deref(), c.recording.album.as_deref());
            (
                !on_release,
                !matches!(c.source, Source::ITunes | Source::Deezer),
            )
        })
        .and_then(|c| c.recording.genre.clone());
    merged.language = matches.iter().find_map(|c| c.recording.language.clone());
    for candidate in matches {
        add_names(&mut merged.composers, &candidate.recording.composers);
        add_names(&mut merged.producers, &candidate.recording.producers);
        add_names(&mut merged.tags, &candidate.recording.tags);
    }
    merged
}

fn apply_discovery(mut track: TrackEntity, discovery: &Discovery) -> SingleSongResponse {
    let original = track.clone();
    let mut detected_language = None;
    let mut detected_mood = None;
    if let Some(recording) = &discovery.recording {
        track.title.clone_from(&recording.title);
        if !recording.artists.is_empty() {
            track.artist = recording.artists.join(" / ");
        }
        replace(&mut track.album, &recording.album);
        if track.year.is_none_or(|y| !(1000..=2100).contains(&y)) {
            replace(&mut track.year, &recording.year);
        }
        if track.track_number.is_none_or(|n| n <= 0) {
            replace(&mut track.track_number, &recording.track_number);
        }
        if track.duration_ms <= 0 {
            track.duration_ms = recording.duration_ms.unwrap_or(track.duration_ms);
        }
        fill(&mut track.genre, recording.genre.clone());
        fill(&mut track.composer, joined_names(&recording.composers));
        fill(&mut track.producer, joined_names(&recording.producers));
        detected_language = recording.language.clone().or_else(|| {
            let lyrics = discovery.lyrics.as_ref()?;
            if lyrics.instrumental {
                Some("Instrumental".to_string())
            } else {
                lyrics
                    .plain
                    .as_deref()
                    .or(lyrics.synced.as_deref())
                    .and_then(detect_script_language)
                    .map(str::to_owned)
            }
        });
        fill(&mut track.language, detected_language.clone());
        detected_mood = infer_mood(track.genre.as_deref(), &recording.tags).map(str::to_owned);
        fill(&mut track.mood, detected_mood.clone());

        let placeholder_artwork = track.artwork_uri.as_deref().is_some_and(|a| {
            a.contains("100x100") || a.contains("50x50") || a.contains("i.ytimg.com/")
        });
        if !usable(track.artwork_uri.as_deref()) || placeholder_artwork {
            replace(&mut track.artwork_uri, &recording.artwork);
        }
        track.metachecked = true;
        if track.metadata_status != "MANUAL" {
            track.metadata_status = "CONFIRMED".to_string();
        }
    }

    let lyrics = discovery.lyrics.as_ref().filter(|hit| hit.has_lyrics());
    if let Some(lyrics) = lyrics {
        track.lyrics_available = true;
        if !usable(track.lyrics_uri.as_deref())
            || track
                .lyrics_uri
                .as_deref()
                .is_some_and(|uri| uri.starts_with("/lyric/"))
        {
            track.lyrics_uri = lyrics.uri();
        }
    }
    let title_cleaned = original.title != track.title;
    let artist_reformatted = original.artist != track.artist;
    let artwork_enhanced = original.artwork_uri != track.artwork_uri;
    let mut audit = EnrichmentAudit {
        matched: discovery.recording.is_some(),
        confidence_score: discovery.confidence,
        fields_updated: Vec::new(),
        fields_corrected: Vec::new(),
        sources_consulted: discovery.sources.clone(),
        raw_title_cleaned: title_cleaned,
        artists_reformatted: artist_reformatted,
        artwork_enhanced,
        lyrics_found: lyrics.is_some(),
        detected_language,
        detected_mood,
    };
    for (field, changed, existed) in [
        ("title", title_cleaned, !original.title.trim().is_empty()),
        (
            "artist",
            artist_reformatted,
            !original.artist.trim().is_empty() && original.artist != "Unknown Artist",
        ),
        (
            "album",
            track.album != original.album,
            original
                .album
                .as_deref()
                .is_some_and(|a| !album_key(a).is_empty()),
        ),
        (
            "year",
            track.year != original.year,
            original.year.is_some_and(|y| y > 0),
        ),
        (
            "trackNumber",
            track.track_number != original.track_number,
            original.track_number.is_some_and(|n| n > 0),
        ),
        (
            "durationMs",
            track.duration_ms != original.duration_ms,
            original.duration_ms > 0,
        ),
        (
            "genre",
            track.genre != original.genre,
            usable(original.genre.as_deref()),
        ),
        (
            "composer",
            track.composer != original.composer,
            usable(original.composer.as_deref()),
        ),
        (
            "producer",
            track.producer != original.producer,
            usable(original.producer.as_deref()),
        ),
        (
            "language",
            track.language != original.language,
            usable(original.language.as_deref()),
        ),
        (
            "mood",
            track.mood != original.mood,
            usable(original.mood.as_deref()),
        ),
        (
            "artworkUri",
            artwork_enhanced,
            usable(original.artwork_uri.as_deref()),
        ),
        (
            "lyricsUri",
            track.lyrics_uri != original.lyrics_uri,
            usable(original.lyrics_uri.as_deref()),
        ),
    ] {
        if changed {
            let fields = if existed {
                &mut audit.fields_corrected
            } else {
                &mut audit.fields_updated
            };
            fields.push(field.to_string());
        }
    }
    let lyrics_sample = lyrics
        .and_then(|hit| hit.synced.as_ref())
        .map(|text| text.lines().take(3).collect::<Vec<_>>().join("\n"));
    SingleSongResponse {
        success: true,
        song: track,
        audit,
        lyrics_sample,
    }
}

fn usable(value: Option<&str>) -> bool {
    value.is_some_and(|s| !s.trim().is_empty() && !s.trim().eq_ignore_ascii_case("unknown"))
}

fn replace<T: Clone>(field: &mut Option<T>, value: &Option<T>) {
    if value.is_some() {
        field.clone_from(value);
    }
}

fn fill(field: &mut Option<String>, value: Option<String>) {
    if !usable(field.as_deref()) && usable(value.as_deref()) {
        *field = value;
    }
}

fn joined_names(names: &[String]) -> Option<String> {
    (!names.is_empty()).then(|| names.join(" / "))
}

fn detect_script_language(text: &str) -> Option<&'static str> {
    let mut languages = text.chars().filter_map(|c| match c {
        '\u{0a00}'..='\u{0a7f}' => Some("Punjabi"),
        '\u{0b80}'..='\u{0bff}' => Some("Tamil"),
        '\u{0c00}'..='\u{0c7f}' => Some("Telugu"),
        '\u{0c80}'..='\u{0cff}' => Some("Kannada"),
        '\u{0d00}'..='\u{0d7f}' => Some("Malayalam"),
        '\u{3040}'..='\u{30ff}' => Some("Japanese"),
        '\u{ac00}'..='\u{d7af}' => Some("Korean"),
        _ => None,
    });
    let first = languages.next()?;
    languages.all(|language| language == first).then_some(first)
}

fn infer_mood(genre: Option<&str>, tags: &[String]) -> Option<&'static str> {
    let evidence = genre
        .into_iter()
        .chain(tags.iter().map(String::as_str))
        .map(str::to_lowercase)
        .collect::<Vec<_>>();
    [
        (
            "Chill",
            &[
                "ambient",
                "acoustic",
                "lo-fi",
                "chill",
                "relax",
                "downtempo",
                "soul",
                "r&b",
            ][..],
        ),
        (
            "Dark",
            &["metal", "goth", "industrial", "dark", "trap", "drill"][..],
        ),
        (
            "Melancholic",
            &["sad", "blues", "melancholic", "ballad"][..],
        ),
        ("Uplifting", &["funk", "disco", "uplifting"][..]),
        (
            "Energetic",
            &[
                "dance",
                "electronic",
                "edm",
                "house",
                "upbeat",
                "energetic",
                "pop",
                "hip-hop",
                "rap",
            ][..],
        ),
        (
            "Focus",
            &["classical", "piano", "focus", "instrumental", "soundtrack"][..],
        ),
    ]
    .into_iter()
    .find_map(|(mood, hints)| {
        evidence
            .iter()
            .any(|tag| hints.iter().any(|hint| tag.contains(hint)))
            .then_some(mood)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn candidate(
        source: Source,
        title: &str,
        artists: &[&str],
        album: &str,
        year: i32,
        ms: i64,
    ) -> Candidate {
        Candidate::new(
            source,
            "1",
            Recording {
                title: title.into(),
                artists: artists.iter().map(|a| (*a).into()).collect(),
                album: Some(album.into()),
                year: Some(year),
                duration_ms: Some(ms),
                ..Recording::default()
            },
        )
    }

    #[test]
    fn merges_all_matching_happier_credits_and_keeps_the_selected_release() {
        let query = SearchQuery::new("happier", "marshmello", None, None, 214_000);
        let mut compilation = candidate(
            Source::Deezer,
            "Happier",
            &["Marshmello"],
            "Dance Party",
            2024,
            214_000,
        );
        compilation.recording.track_number = Some(9);
        compilation.recording.artwork = Some("https://example.test/compilation.jpg".into());
        let mut original = candidate(
            Source::ITunes,
            "Happier",
            &["Marshmello", "Bastille"],
            "Happier - Single",
            2018,
            214_290,
        );
        original.recording.track_number = Some(1);
        let ranked = rank_candidates(
            &query,
            &[
                compilation,
                candidate(
                    Source::ITunes,
                    "Happier",
                    &["Olivia Rodrigo"],
                    "SOUR",
                    2021,
                    175_000,
                ),
                candidate(
                    Source::Deezer,
                    "Happier (Remix)",
                    &["Marshmello", "Bastille", "Remixer"],
                    "Remixes",
                    2018,
                    214_000,
                ),
                original,
            ],
        );
        let metadata = merge_recordings(&query, &ranked);
        assert_eq!(metadata.artists, ["Marshmello", "Bastille"]);
        assert_eq!(metadata.album.as_deref(), Some("Happier - Single"));
        assert_eq!(metadata.year, Some(2018));
        assert_eq!(metadata.track_number, Some(1));
        assert!(metadata.artwork.is_none());
    }

    #[test]
    fn combines_a_verified_soundtrack_upload_with_its_catalog_release() {
        let query = SearchQuery::new(
            "Podimeesha Mulakkana Kaalam",
            "Pa Va, P Jayachandran, Santhosh Varma, Anand Madhusoodanan",
            Some("Pa Va"),
            None,
            195_000,
        );
        let mut upload = candidate(
            Source::YouTube,
            "Podimeesha Mulakkana Kaalam",
            &["P. Jayachandran"],
            "Pa Va",
            2016,
            191_000,
        );
        upload.recording.year = None;
        upload.recording.composers = vec!["Santhosh Varma".into(), "Anand Madhusoodanan".into()];
        let mut release = candidate(
            Source::ITunes,
            "Podimeesha",
            &["P.Jayachandran", "Santhosh Varma"],
            "Pa Va (Original Motion Picture Soundtrack) - EP",
            2016,
            194_961,
        );
        release.recording.track_number = Some(1);
        let ranked = rank_candidates(&query, &[upload, release]);
        let result = merge_recordings(&query, &ranked);
        assert_eq!(ranked.len(), 2);
        assert_eq!(result.artists, ["P. Jayachandran"]);
        assert_eq!(result.composers, ["Santhosh Varma", "Anand Madhusoodanan"]);
        assert_eq!(result.year, Some(2016));
        assert_eq!(result.track_number, Some(1));
    }

    #[test]
    fn cacheable_discovery_keeps_each_files_metadata_and_unknowns() {
        let track: TrackEntity = serde_json::from_value(json!({
            "id": "local-2", "title": "distant skies (live)", "artist": "Unlisted Artist", "durationMs": 191_000,
            "genre": "Jazz", "fileUri": "file:///local-2.mp3", "lyricsAvailable": true, "lyricsUri": "file:///local-2.lrc"
        })).unwrap();
        let unmatched = apply_discovery(track.clone(), &Discovery::default());
        assert!(!unmatched.audit.matched);
        assert!(!unmatched.song.metachecked);
        assert_eq!(unmatched.song.metadata_status, "PARSED");
        assert!(unmatched.song.mood.is_none());
        assert!(unmatched.song.lyrics_available);

        let result = apply_discovery(
            track,
            &Discovery {
                recording: Some(Recording {
                    title: "Distant Skies (Live)".into(),
                    genre: Some("Pop".into()),
                    ..Recording::default()
                }),
                ..Discovery::default()
            },
        );
        assert_eq!(result.song.id, "local-2");
        assert_eq!(result.song.genre.as_deref(), Some("Jazz"));
        assert_eq!(result.song.file_uri.as_deref(), Some("file:///local-2.mp3"));
        assert_eq!(
            result.song.lyrics_uri.as_deref(),
            Some("file:///local-2.lrc")
        );
        assert_eq!(result.song.duration_ms, 191_000);
    }
}
