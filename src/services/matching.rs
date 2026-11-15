use crate::services::cleaner::{clean_catalog_title, clean_track, featured_artists, split_artists};
use icu_normalizer::DecomposingNormalizer;
use regex::Regex;
use std::sync::LazyLock;
use strsim::normalized_levenshtein;

static SEPARATORS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\p{L}\p{M}\p{N}]+").unwrap());

static LATIN_ACCENTS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\p{Latin})\p{M}+").unwrap());

static SOURCE_TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s*[(\[]\s*from\s+([^\])]+)[)\]]\s*$").unwrap());

static VERSION_WORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:live|remix|mix|karaoke|instrumental|acoustic|cover|sped up|slowed|reverb|nightcore|demo|stripped|edit|extended|remaster(?:ed)?|re recorded|session|alternate|version|clean|explicit|reprise)\b").unwrap()
});

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct SearchQuery {
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub year: Option<i32>,
    pub duration_ms: Option<i64>,
    pub youtube_id: Option<String>,
}

impl SearchQuery {
    pub fn new(title: &str, artist: &str, album: Option<&str>, year: Option<i32>, ms: i64) -> Self {
        let cleaned = clean_track(title, artist);
        let album = album
            .filter(|s| !album_key(s).is_empty())
            .map(str::trim)
            .map(str::to_owned)
            .or_else(|| soundtrack_name(&cleaned.cleaned_title));
        let mut artists = cleaned.extracted_artists;
        if artists.len() > 1 {
            let performers: Vec<_> = artists
                .iter()
                .filter(|name| {
                    !same_album(album.as_deref(), Some(name))
                        && !name
                            .parse::<i32>()
                            .is_ok_and(|y| (1900..=2100).contains(&y))
                })
                .cloned()
                .collect();
            if !performers.is_empty() {
                artists = performers;
            }
        }
        Self {
            album,
            title: cleaned.cleaned_title,
            artists,
            year: year.filter(|y| (1000..=2100).contains(y)),
            duration_ms: (ms > 0).then_some(ms),
            youtube_id: None,
        }
    }

    pub fn key(&self) -> Self {
        Self {
            title: self.title.to_lowercase(),
            artists: self.artists.iter().map(|a| a.to_lowercase()).collect(),
            album: self.album.as_ref().map(|s| s.to_lowercase()),
            year: self.year,
            duration_ms: self.duration_ms,
            youtube_id: self.youtube_id.clone(),
        }
    }

    pub fn set_youtube_id(&mut self, id: Option<&str>) {
        self.youtube_id = id
            .filter(|id| {
                id.len() == 11
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            })
            .map(str::to_owned);
    }

    pub fn artist(&self) -> &str {
        self.artists.first().map(String::as_str).unwrap_or("")
    }

    pub fn term(&self) -> String {
        self.term_with(&self.title, self.artist())
    }

    pub fn term_with(&self, title: &str, hint: &str) -> String {
        let hint = if hint.is_empty() {
            self.album.as_deref().unwrap_or("")
        } else {
            hint
        };
        [title, hint].join(" ").trim().to_string()
    }

    pub fn alternate_term(&self) -> String {
        let title = short_title(&self.title).unwrap_or(&self.title);
        let hint = self
            .album
            .as_deref()
            .or_else(|| self.artists.get(1).map(String::as_str))
            .unwrap_or(self.artist());
        self.term_with(title, hint)
    }

    pub fn swapped(&self) -> Option<Self> {
        (self.artists.len() == 1 && self.album.is_none()).then(|| Self {
            title: self.artist().to_string(),
            artists: vec![self.title.clone()],
            ..self.clone()
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Source {
    MusicBrainz,
    Deezer,
    ITunes,
    YouTube,
    Lrclib,
    NetEase,
}

impl Source {
    pub fn name(self) -> &'static str {
        match self {
            Self::MusicBrainz => "MusicBrainz",
            Self::Deezer => "Deezer",
            Self::ITunes => "iTunes",
            Self::YouTube => "YouTube",
            Self::Lrclib => "LRCLIB",
            Self::NetEase => "NetEase",
        }
    }

    pub fn has_structured_credits(self) -> bool {
        matches!(
            self,
            Self::MusicBrainz | Self::Deezer | Self::YouTube | Self::NetEase
        )
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct Recording {
    pub title: String,
    pub artists: Vec<String>,
    pub aliases: Vec<String>,
    pub album: Option<String>,
    pub year: Option<i32>,
    pub track_number: Option<i32>,
    pub duration_ms: Option<i64>,
    pub genre: Option<String>,
    pub artwork: Option<String>,
    pub composers: Vec<String>,
    pub producers: Vec<String>,
    pub language: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct Candidate {
    pub source: Source,
    pub id: String,
    pub release_id: Option<String>,
    pub recording: Recording,
}

impl Candidate {
    pub fn new(source: Source, id: impl Into<String>, mut recording: Recording) -> Self {
        add_names(&mut recording.artists, &featured_artists(&recording.title));
        Self {
            source,
            id: id.into(),
            release_id: None,
            recording,
        }
    }
}

pub(super) fn normalize(value: &str) -> String {
    let value = value.to_lowercase().replace(['\'', '’'], "");
    if value.is_ascii() {
        return SEPARATORS.replace_all(&value, " ").trim().to_string();
    }
    let normalizer = DecomposingNormalizer::new_nfd();
    let decomposed = normalizer.normalize(&value);
    let normalized = LATIN_ACCENTS.replace_all(&decomposed, "$1");
    SEPARATORS.replace_all(&normalized, " ").trim().to_string()
}

pub(super) fn album_key(value: &str) -> String {
    let normalized = normalize(value);
    let mut key = normalized.as_str();
    while let Some(stripped) = [
        " original motion picture soundtrack",
        " original soundtrack",
        " motion picture soundtrack",
        " soundtrack",
        " single",
        " ep",
    ]
    .iter()
    .find_map(|suffix| key.strip_suffix(suffix))
    {
        key = stripped;
    }
    match key {
        "" | "unknown" | "unknown album" | "downloads" | "untitled" => String::new(),
        other => other.to_string(),
    }
}

pub(super) fn same_album(a: Option<&str>, b: Option<&str>) -> bool {
    let (a, b) = (a.map(album_key), b.map(album_key));
    a.as_ref().is_some_and(|key| !key.is_empty()) && a == b
}

pub(super) fn year_from_date(date: &str) -> Option<i32> {
    date.get(..4)?
        .parse()
        .ok()
        .filter(|y| (1000..=2100).contains(y))
}

pub(super) fn add_names(target: &mut Vec<String>, names: &[String]) {
    for name in names {
        let key = normalize(name);
        if !key.is_empty() && !target.iter().any(|n| normalize(n) == key) {
            target.push(name.trim().to_string());
        }
    }
}

pub(super) fn soundtrack_name(title: &str) -> Option<String> {
    Some(
        SOURCE_TITLE.captures(title)?[1]
            .trim_matches([' ', '"', '\'', '“', '”'])
            .to_string(),
    )
}

fn title_key(value: &str) -> String {
    normalize(&SOURCE_TITLE.replace(&clean_catalog_title(value), ""))
}

fn short_title(title: &str) -> Option<&str> {
    let first = title.split_whitespace().next()?;
    (first.chars().count() >= 7 && title.split_whitespace().count() >= 3).then_some(first)
}

fn same_short_title(a: &str, b: &str, allow_typo: bool) -> bool {
    let (full, short) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    short_title(full).is_some_and(|first| {
        first == short
            || (allow_typo
                && !short.contains(' ')
                && short.chars().count() >= 9
                && normalized_levenshtein(first, short) >= 0.85)
    })
}

pub(super) fn catalog_artists(credit: &str, query: &SearchQuery) -> Vec<String> {
    let names = split_artists(credit);
    // Keep an unstructured group name intact when it is already the entire input credit.
    if names.len() > 1
        && query.artists.len() > 1
        && normalize(credit) == normalize(&query.artists.join(" "))
        && !credit.contains(" / ")
    {
        vec![credit.trim().to_string()]
    } else {
        names
    }
}

pub(super) fn has_exact_title(query: &SearchQuery, recording: &Recording) -> bool {
    title_key(&query.title) == title_key(&recording.title)
}

fn shared_artist(a: &[String], b: &[String]) -> bool {
    let joined_a = normalize(&a.join(" "));
    let joined_b = normalize(&b.join(" "));
    (!joined_a.is_empty() && joined_a == joined_b)
        || a.iter().any(|name| {
            b.iter()
                .any(|other| normalize(name) == normalize(other) && !normalize(name).is_empty())
        })
}

pub(super) fn duration_delta(input: Option<i64>, candidate: Option<i64>) -> Option<u64> {
    let a = input.filter(|d| *d > 0)?;
    let b = candidate.filter(|d| *d > 0)?;
    Some(a.abs_diff(b))
}

pub(super) fn score(query: &SearchQuery, recording: &Recording) -> Option<u16> {
    let input = title_key(&query.title);
    let title = title_key(&recording.title);
    if input.is_empty() || title.is_empty() || recording.artists.is_empty() {
        return None;
    }

    let album_matches = same_album(query.album.as_deref(), recording.album.as_deref());
    let duration_diff = duration_delta(query.duration_ms, recording.duration_ms);
    let close_duration = duration_diff.is_some_and(|d| d <= 4_000);
    let duration_limit = (query.duration_ms.unwrap_or(0) as u64 / 12).clamp(10_000, 25_000);
    if duration_diff.is_some_and(|d| d > duration_limit) {
        return None;
    }

    let artist_matches = shared_artist(&query.artists, &recording.artists);
    let version_label = VERSION_WORD.is_match(&input) || VERSION_WORD.is_match(&title);
    let exact_title = input == title
        || (!version_label && recording.aliases.iter().any(|a| title_key(a) == input));
    let album_supports_match =
        exact_title && album_matches && (query.artists.is_empty() || close_duration);
    let duration_supports_match = exact_title
        && query.artists.is_empty()
        && duration_diff.is_some_and(|d| d <= 1_500)
        && input.chars().count() >= 8;
    if !(artist_matches || album_supports_match || duration_supports_match) {
        return None;
    }

    let title_points = if exact_title {
        70
    } else if version_label {
        // Exact version names keep nearby titles like 'Club Mix' and 'Dub Mix' separate.
        return None;
    } else if artist_matches
        && (album_matches || close_duration)
        && same_short_title(&input, &title, album_matches && close_duration)
    {
        52
    } else if artist_matches && input.chars().count().min(title.chars().count()) >= 7 {
        let similarity = normalized_levenshtein(&input, &title);
        let minimum = if album_matches || close_duration {
            0.83
        } else {
            0.92
        };
        if similarity < minimum {
            return None;
        }
        48 + (similarity * 15.0) as u16
    } else {
        return None;
    };

    let matching_names = query
        .artists
        .iter()
        .filter(|a| {
            recording
                .artists
                .iter()
                .any(|b| normalize(a) == normalize(b))
        })
        .count()
        .saturating_sub(1)
        .min(3) as u16;
    let duration_points = match duration_diff {
        Some(0..=4_000) => 12,
        Some(4_001..=8_000) => 7,
        Some(_) => 0,
        None => 3,
    };
    let year_points = match query.year.zip(recording.year) {
        Some((a, b)) if a == b => 5,
        _ => 0,
    };
    Some(
        title_points
            + u16::from(artist_matches) * 24
            + matching_names * 3
            + u16::from(album_matches) * 20
            + duration_points
            + year_points,
    )
}

pub(super) fn same_recording(a: &Recording, b: &Recording) -> bool {
    let difference = duration_delta(a.duration_ms, b.duration_ms);
    if !shared_artist(&a.artists, &b.artists) || difference.is_some_and(|d| d > 8_000) {
        return false;
    }
    let (title_a, title_b) = (title_key(&a.title), title_key(&b.title));
    if title_a == title_b {
        return true;
    }
    if VERSION_WORD.is_match(&title_a) || VERSION_WORD.is_match(&title_b) {
        return false;
    }
    a.aliases.iter().any(|alias| title_key(alias) == title_b)
        || b.aliases.iter().any(|alias| title_key(alias) == title_a)
        || (same_album(a.album.as_deref(), b.album.as_deref())
            && difference.is_some()
            && title_a.chars().count().min(title_b.chars().count()) >= 7
            && (same_short_title(&title_a, &title_b, false)
                || (difference.is_some_and(|d| d <= 4_000)
                    && normalized_levenshtein(&title_a, &title_b) >= 0.90)))
}

pub(super) fn rank_candidates(query: &SearchQuery, candidates: &[Candidate]) -> Vec<Candidate> {
    let valid: Vec<_> = candidates
        .iter()
        .filter_map(|c| score(query, &c.recording).map(|s| (c, s)))
        .collect();
    let mut ranked: Vec<_> = valid
        .iter()
        .map(|&(candidate, points)| {
            let mut peers = Vec::new();
            for &(other, _) in &valid {
                if other.source != candidate.source
                    && !peers.contains(&other.source)
                    && same_recording(&candidate.recording, &other.recording)
                {
                    peers.push(other.source);
                }
            }
            (candidate, points + (peers.len().min(3) as u16 * 4))
        })
        .collect();
    ranked.sort_by_key(|(c, points)| {
        (
            std::cmp::Reverse(*points),
            c.recording.year.unwrap_or(i32::MAX),
        )
    });

    let Some((best, best_score)) = ranked.first().copied() else {
        return Vec::new();
    };
    // With no artist evidence, similarly ranked recordings with different artists are ambiguous.
    if !shared_artist(&query.artists, &best.recording.artists)
        && ranked.iter().skip(1).any(|(c, points)| {
            best_score.saturating_sub(*points) <= 4
                && !same_recording(&best.recording, &c.recording)
        })
    {
        return Vec::new();
    }

    let mut group = vec![best.clone()];
    for (candidate, _) in ranked.into_iter().skip(1) {
        if !group.iter().any(|c| c.source == candidate.source)
            && same_recording(&best.recording, &candidate.recording)
        {
            group.push(candidate.clone());
        }
    }
    if query.artists.is_empty() && query.album.is_none() && group.len() < 2 {
        Vec::new()
    } else {
        group
    }
}
