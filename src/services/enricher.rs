use crate::models::{EnrichmentAudit, LyricQuery, SingleSongResponse, TrackEntity};
use crate::services::cleaner::{clean_track, split_artists};
use crate::services::deezer::DeezerService;
use crate::services::itunes::ITunesService;
use crate::services::lyrics::LyricsAggregator;
use crate::services::musicbrainz::MusicBrainzService;
use crate::services::web_search::WebSearchService;
use moka::future::Cache;
use strsim::jaro_winkler;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct MetadataEnricher {
    deezer: DeezerService,
    itunes: ITunesService,
    musicbrainz: MusicBrainzService,
    web_search: WebSearchService,
    lyrics: Arc<LyricsAggregator>,
    cache: Cache<String, SingleSongResponse>,
}

impl MetadataEnricher {
    pub fn new(lyrics: Arc<LyricsAggregator>) -> Self {
        let cache = Cache::builder()
            .max_capacity(10_000)
            .time_to_live(Duration::from_secs(86400)) // 24 hours
            .build();

        Self {
            deezer: DeezerService::new(),
            itunes: ITunesService::new(),
            musicbrainz: MusicBrainzService::new(),
            web_search: WebSearchService::new(),
            lyrics,
            cache,
        }
    }

    pub async fn enrich_song(&self, mut track: TrackEntity) -> SingleSongResponse {
        // Cache key based on input title + artist
        let cache_key = format!(
            "{}:{}",
            track.title.to_lowercase().trim(),
            track.artist.to_lowercase().trim()
        );

        if let Some(cached) = self.cache.get(&cache_key).await {
            let mut res = cached;
            res.song.id = track.id.clone();
            res.song.file_uri = track.file_uri.clone();
            res.song.source = track.source.clone();
            res.song.format = track.format.clone();
            res.song.size_bytes = track.size_bytes;
            res.song.bitrate_kbps = track.bitrate_kbps;
            res.song.sample_rate = track.sample_rate;
            res.song.date_added = track.date_added;
            res.song.date_modified = track.date_modified;
            return res;
        }

        let mut fields_updated = Vec::new();
        let mut fields_corrected = Vec::new();
        let mut sources_consulted = Vec::new();

        // 1. Clean Title and Format Artists Canonical
        let cleaned = clean_track(&track.title, &track.artist);
        let mut raw_title_cleaned = cleaned.was_title_cleaned;
        let mut artists_reformatted = cleaned.were_artists_reformatted;

        if raw_title_cleaned {
            fields_corrected.push("title".to_string());
            track.title = cleaned.cleaned_title.clone();
        }

        if artists_reformatted || !track.artist.contains(" / ") && cleaned.extracted_artists.len() > 1 {
            fields_corrected.push("artist".to_string());
            track.artist = cleaned.formatted_artist.clone();
        }

        // 2. Deep Web Search & Artist/Movie Disambiguation (handles movie names inside artist, swapped title/artist, complex credits)
        let disambig = self.web_search.disambiguate(&track.title, &track.artist).await;

        if disambig.was_intermixed_or_swapped || disambig.resolved_movie_album.is_some() || !disambig.resolved_singers.is_empty() {
            sources_consulted.push("Google/WebSearch".to_string());
        }

        // If title and artist were intermixed or swapped, reverse them cleanly
        if disambig.was_intermixed_or_swapped && crate::services::web_search::looks_like_song_title(&track.artist) && crate::services::web_search::looks_like_artist_name(&track.title) {
            std::mem::swap(&mut track.title, &mut track.artist);
            fields_corrected.push("title".to_string());
            fields_corrected.push("artist".to_string());
            raw_title_cleaned = true;
            artists_reformatted = true;
        }

        // Handle movie name extracted from artist or title
        if let Some(ref movie) = disambig.resolved_movie_album {
            if track.album.as_deref().unwrap_or("").trim().is_empty() {
                track.album = Some(movie.clone());
                fields_updated.push("album".to_string());
            }

            // Strip movie from title if it was attached like "Song Name / Movie Name"
            let lower_movie = movie.to_lowercase();
            let mut parts = track.title.split(&['/', '|', '-'][..]).map(|s| s.trim()).collect::<Vec<_>>();
            if parts.len() > 1 {
                parts.retain(|p| !p.to_lowercase().contains(&lower_movie) && !lower_movie.contains(&p.to_lowercase()));
                if !parts.is_empty() {
                    let cleaned_title = parts.join(" ");
                    if cleaned_title != track.title {
                        track.title = cleaned_title;
                    }
                }
            }
        }

        // If web search identified actual singer(s), use them as canonical artist and filter out movie/composer names
        if !disambig.resolved_singers.is_empty() {
            let canon_singers = disambig.resolved_singers.join(" / ");
            if track.artist != canon_singers {
                track.artist = canon_singers;
                if !fields_corrected.contains(&"artist".to_string()) {
                    fields_corrected.push("artist".to_string());
                }
                artists_reformatted = true;
            }
        } else if let Some(ref movie) = disambig.resolved_movie_album {
            // Remove movie from artist tokens if it was accidentally placed inside
            let mut remaining_artists = split_artists(&track.artist);
            remaining_artists.retain(|a| !a.eq_ignore_ascii_case(movie) && !movie.to_lowercase().contains(&a.to_lowercase()));
            if !remaining_artists.is_empty() {
                let filtered = remaining_artists.join(" / ");
                if filtered != track.artist {
                    track.artist = filtered;
                    if !fields_corrected.contains(&"artist".to_string()) {
                        fields_corrected.push("artist".to_string());
                    }
                }
            }
        }

        // Handle resolved composers and lyricists
        let mut combined_composers = disambig.resolved_composers.clone();
        for lyr in &disambig.resolved_lyricists {
            if !combined_composers.contains(lyr) {
                combined_composers.push(lyr.clone());
            }
        }
        if !combined_composers.is_empty() {
            let comp_str = combined_composers.join(" / ");
            if track.composer.as_deref().unwrap_or("").trim().is_empty() {
                track.composer = Some(comp_str);
                fields_updated.push("composer".to_string());
            }
        }

        // Handle resolved producers
        if !disambig.resolved_producers.is_empty() {
            let prod_str = disambig.resolved_producers.join(" / ");
            if track.producer.as_deref().unwrap_or("").trim().is_empty() {
                track.producer = Some(prod_str);
                fields_updated.push("producer".to_string());
            }
        }

        // Handle detected language
        if let Some(ref lang) = disambig.detected_language {
            if track.language.as_deref().unwrap_or("").trim().is_empty() {
                track.language = Some(lang.clone());
                fields_updated.push("language".to_string());
            }
        }

        // Primary search artist (first credited artist for external queries)
        let artist_tokens = split_artists(&track.artist);
        let primary_artist = artist_tokens
            .first()
            .cloned()
            .unwrap_or_else(|| track.artist.clone());

        // 3. Query Deep External Services Concurrently
        let deezer_fut = self.deezer.search_track(&track.title, &primary_artist);
        let itunes_fut = self.itunes.search_track(&track.title, &primary_artist, track.album.as_deref());
        let mb_fut = self.musicbrainz.search_recording(&track.title, &primary_artist);
        let lyric_query = LyricQuery {
            id: Some(track.id.clone()),
            title: track.title.clone(),
            artist: primary_artist.clone(),
            album: track.album.clone(),
            duration_ms: if track.duration_ms > 0 { Some(track.duration_ms) } else { None },
        };
        let lyric_fut = self.lyrics.get_lyrics(&lyric_query);

        let (deezer_res, itunes_res, mb_res, lyric_res) =
            tokio::join!(deezer_fut, itunes_fut, mb_fut, lyric_fut);

        if deezer_res.is_some() {
            sources_consulted.push("Deezer".to_string());
        }
        if itunes_res.is_some() {
            sources_consulted.push("iTunes".to_string());
        }
        if mb_res.is_some() {
            sources_consulted.push("MusicBrainz".to_string());
        }
        if lyric_res.lyrics_available {
            sources_consulted.push(lyric_res.source.clone());
        }

        let mut confidence_sum: f32 = 0.0;
        let mut confidence_count = 0;

        // 3. Synthesize & Correct Title
        if let Some(ref dz) = deezer_res {
            if let Some(ref dz_title) = dz.canonical_title {
                let score = jaro_winkler(&track.title.to_lowercase(), &dz_title.to_lowercase()) as f32;
                confidence_sum += score;
                confidence_count += 1;
                if score > 0.85 && track.title != *dz_title {
                    track.title = dz_title.clone();
                    if !fields_corrected.contains(&"title".to_string()) {
                        fields_corrected.push("title".to_string());
                    }
                }
            }
        }

        // 4. Synthesize Artists (ensure all credited artists are present and formatted with " / ")
        let mut final_artists = split_artists(&track.artist);
        if let Some(ref dz) = deezer_res {
            for c in &dz.contributors {
                let norm = c.trim();
                if !norm.is_empty() && !final_artists.iter().any(|a| a.eq_ignore_ascii_case(norm)) {
                    final_artists.push(norm.to_string());
                }
            }
        }
        if let Some(ref mb) = mb_res {
            for a in &mb.artists {
                let norm = a.trim();
                if !norm.is_empty() && !final_artists.iter().any(|existing| existing.eq_ignore_ascii_case(norm)) {
                    final_artists.push(norm.to_string());
                }
            }
        }
        if !final_artists.is_empty() {
            let canon_artist = final_artists.join(" / ");
            if track.artist != canon_artist {
                track.artist = canon_artist;
                if !fields_corrected.contains(&"artist".to_string()) {
                    fields_corrected.push("artist".to_string());
                }
            }
        }

        // 5. Synthesize Album
        let canonical_album = itunes_res
            .as_ref()
            .and_then(|it| it.album_name.clone())
            .or_else(|| deezer_res.as_ref().and_then(|dz| dz.album_title.clone()))
            .or_else(|| mb_res.as_ref().and_then(|mb| mb.album_title.clone()));

        if let Some(ref alb) = canonical_album {
            if track.album.as_deref().unwrap_or("").trim().is_empty() {
                track.album = Some(alb.clone());
                fields_updated.push("album".to_string());
            } else if let Some(existing) = &track.album {
                let score = jaro_winkler(&existing.to_lowercase(), &alb.to_lowercase());
                if score < 0.6 {
                    track.album = Some(alb.clone());
                    fields_corrected.push("album".to_string());
                }
            }
        }

        // 6. Synthesize Year
        let canonical_year = itunes_res
            .as_ref()
            .and_then(|it| it.release_year)
            .or_else(|| deezer_res.as_ref().and_then(|dz| dz.release_year))
            .or_else(|| mb_res.as_ref().and_then(|mb| mb.release_year));

        if let Some(yr) = canonical_year {
            if track.year.is_none() || track.year == Some(0) {
                track.year = Some(yr);
                fields_updated.push("year".to_string());
            } else if let Some(existing_yr) = track.year {
                if (existing_yr - yr).abs() > 3 {
                    track.year = Some(yr);
                    fields_corrected.push("year".to_string());
                }
            }
        }

        // 7. Synthesize Genre
        let canonical_genre = itunes_res
            .as_ref()
            .and_then(|it| it.primary_genre.clone())
            .or_else(|| deezer_res.as_ref().and_then(|dz| dz.genre.clone()))
            .or_else(|| {
                mb_res.as_ref().and_then(|mb| {
                    mb.tags
                        .iter()
                        .find(|t| !t.contains("seen") && !t.contains("favorite"))
                        .map(|t| capitalize_words(t))
                })
            });

        if let Some(ref g) = canonical_genre {
            if track.genre.as_deref().unwrap_or("").trim().is_empty() {
                track.genre = Some(g.clone());
                fields_updated.push("genre".to_string());
            }
        }

        // 8. Synthesize Track Number
        let canonical_track_num = itunes_res
            .as_ref()
            .and_then(|it| it.track_number)
            .or_else(|| deezer_res.as_ref().and_then(|dz| dz.track_number));

        if let Some(tn) = canonical_track_num {
            if track.track_number.is_none() || track.track_number == Some(0) {
                track.track_number = Some(tn);
                fields_updated.push("trackNumber".to_string());
            }
        }

        // 9. Extended Creative Credits: Producer & Composer
        let mut discovered_producers = Vec::new();
        let mut discovered_composers = Vec::new();

        if let Some(ref mb) = mb_res {
            for p in &mb.producers {
                if !discovered_producers.contains(p) {
                    discovered_producers.push(p.clone());
                }
            }
            for c in &mb.composers {
                if !discovered_composers.contains(c) {
                    discovered_composers.push(c.clone());
                }
            }
        }

        if !discovered_producers.is_empty() {
            let prod_str = discovered_producers.join(" / ");
            if track.producer.as_deref().unwrap_or("").trim().is_empty() {
                track.producer = Some(prod_str);
                fields_updated.push("producer".to_string());
            }
        }

        if !discovered_composers.is_empty() {
            let comp_str = discovered_composers.join(" / ");
            if track.composer.as_deref().unwrap_or("").trim().is_empty() {
                track.composer = Some(comp_str);
                fields_updated.push("composer".to_string());
            }
        }

        // 10. Vocal Language
        let detected_language = mb_res
            .as_ref()
            .and_then(|mb| mb.language.clone())
            .or_else(|| {
                // Infer from lyric or script
                if lyric_res.is_instrumental() {
                    Some("Instrumental".to_string())
                } else if let Some(ref text) = lyric_res.plain_lyrics {
                    Some(detect_script_language(text))
                } else {
                    None
                }
            });

        if let Some(ref lang) = detected_language {
            if track.language.as_deref().unwrap_or("").trim().is_empty() {
                track.language = Some(lang.clone());
                fields_updated.push("language".to_string());
            }
        }

        // 11. Vibe & Mood Classification
        let mut all_tags = Vec::new();
        if let Some(ref g) = track.genre {
            all_tags.push(g.to_lowercase());
        }
        if let Some(ref mb) = mb_res {
            for t in &mb.tags {
                all_tags.push(t.clone());
            }
        }
        let detected_mood = infer_mood(&all_tags, track.year);
        if track.mood.as_deref().unwrap_or("").trim().is_empty() {
            track.mood = Some(detected_mood.clone());
            fields_updated.push("mood".to_string());
        }

        // 12. Ultra High Resolution Artwork
        let best_artwork = itunes_res
            .as_ref()
            .and_then(|it| it.high_res_artwork.clone())
            .or_else(|| deezer_res.as_ref().and_then(|dz| dz.artwork_url.clone()));

        let mut artwork_enhanced = false;
        if let Some(art) = best_artwork {
            if track.artwork_uri.as_deref().unwrap_or("").trim().is_empty() {
                track.artwork_uri = Some(art);
                fields_updated.push("artworkUri".to_string());
                artwork_enhanced = true;
            } else if let Some(existing_art) = &track.artwork_uri {
                // If existing is low res thumbnail or placeholder, replace with high-res
                if existing_art.contains("100x100") || existing_art.contains("50x50") || existing_art.contains("default") {
                    track.artwork_uri = Some(art);
                    fields_corrected.push("artworkUri".to_string());
                    artwork_enhanced = true;
                }
            }
        }

        // 13. Lyrics Status & URI
        let lyrics_found = lyric_res.lyrics_available;
        track.lyrics_available = lyrics_found;
        if lyrics_found {
            track.lyrics_uri = lyric_res.lyrics_uri.clone();
            fields_updated.push("lyricsUri".to_string());
        }

        // 14. Verification & Server Status Tags
        track.metachecked = true;
        track.metadata_status = "CONFIRMED".to_string();

        let confidence_score = if confidence_count > 0 {
            (confidence_sum / confidence_count as f32).clamp(0.75, 1.0)
        } else if !sources_consulted.is_empty() {
            0.88
        } else {
            0.60
        };

        let audit = EnrichmentAudit {
            matched: !sources_consulted.is_empty(),
            confidence_score,
            fields_updated,
            fields_corrected,
            sources_consulted,
            raw_title_cleaned,
            artists_reformatted,
            artwork_enhanced,
            lyrics_found,
            detected_language,
            detected_mood: Some(detected_mood),
        };

        let lyrics_sample = lyric_res.synced_lyrics.as_ref().and_then(|s| {
            s.lines().take(3).collect::<Vec<&str>>().join("\n").into()
        });

        let response = SingleSongResponse {
            success: true,
            song: track,
            audit,
            lyrics_sample,
        };

        self.cache.insert(cache_key, response.clone()).await;
        response
    }

    pub fn get_cache(&self) -> &Cache<String, SingleSongResponse> {
        &self.cache
    }
}

trait LyricHelper {
    fn is_instrumental(&self) -> bool;
}

impl LyricHelper for crate::models::LyricResponse {
    fn is_instrumental(&self) -> bool {
        self.plain_lyrics
            .as_deref()
            .map(|p| p.to_lowercase().contains("instrumental"))
            .unwrap_or(false)
    }
}

fn capitalize_words(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

fn detect_script_language(text: &str) -> String {
    let mut has_cjk = false;
    let mut has_hangul = false;
    let mut has_kana = false;
    let mut has_cyrillic = false;
    let mut has_arabic = false;
    let mut has_devanagari = false;

    for c in text.chars() {
        match c as u32 {
            0x4E00..=0x9FFF => has_cjk = true,
            0xAC00..=0xD7AF => has_hangul = true,
            0x3040..=0x30FF => has_kana = true,
            0x0400..=0x04FF => has_cyrillic = true,
            0x0600..=0x06FF => has_arabic = true,
            0x0900..=0x097F => has_devanagari = true,
            _ => {}
        }
    }

    if has_hangul {
        "Korean".to_string()
    } else if has_kana {
        "Japanese".to_string()
    } else if has_cjk {
        "Chinese".to_string()
    } else if has_cyrillic {
        "Russian".to_string()
    } else if has_arabic {
        "Arabic".to_string()
    } else if has_devanagari {
        "Hindi".to_string()
    } else {
        "English".to_string()
    }
}

fn infer_mood(tags: &[String], _year: Option<i32>) -> String {
    let joined = tags.join(" ");

    if joined.contains("ambient")
        || joined.contains("acoustic")
        || joined.contains("lo-fi")
        || joined.contains("chill")
        || joined.contains("relax")
        || joined.contains("downtempo")
        || joined.contains("soul")
        || joined.contains("r&b")
    {
        "Chill".to_string()
    } else if joined.contains("dance")
        || joined.contains("electronic")
        || joined.contains("edm")
        || joined.contains("house")
        || joined.contains("upbeat")
        || joined.contains("energetic")
        || joined.contains("pop")
        || joined.contains("hip-hop")
        || joined.contains("rap")
    {
        "Energetic".to_string()
    } else if joined.contains("metal")
        || joined.contains("goth")
        || joined.contains("industrial")
        || joined.contains("dark")
        || joined.contains("trap")
        || joined.contains("drill")
    {
        "Dark".to_string()
    } else if joined.contains("classical")
        || joined.contains("piano")
        || joined.contains("focus")
        || joined.contains("instrumental")
        || joined.contains("soundtrack")
    {
        "Focus".to_string()
    } else if joined.contains("sad")
        || joined.contains("blues")
        || joined.contains("melanchol")
        || joined.contains("slow")
        || joined.contains("ballad")
    {
        "Melancholic".to_string()
    } else if joined.contains("funk")
        || joined.contains("disco")
        || joined.contains("summer")
        || joined.contains("uplifting")
    {
        "Uplifting".to_string()
    } else {
        "Chill".to_string()
    }
}
