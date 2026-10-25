use regex::Regex;
use std::sync::LazyLock;

static BRACKET_NOISE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s*[\(\[\{][^\)\]\}]*(?:video|audio|visualizer|lyrics|lyric|remaster(?:ed)?|clean|explicit|4k|60fps|hd|hq|extended|radio\s*edit|live)[^\)\]\}]*[\)\]\}]\s*")
        .unwrap()
});

static UNMATCHED_BRACKETS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[\(\[\{][^\)\]\}]*$|^[^\(\[\{]*[\)\]\}]").unwrap()
});

static FEAT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s*[\(\[]?(?:feat\.?|ft\.?|featuring)\s+([^\)\]]+)[\)\]]?").unwrap()
});

static PIPE_SUFFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\s*\|.*$").unwrap()
});

static FORMAT_EXTENSION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\.(?:mp3|flac|m4a|wav|aac|ogg|opus|webm|wma|alac|aiff|ape)$").unwrap()
});

static FORMAT_NOISE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s*[\(\[\{]?(?:320\s*kbps|128\s*kbps|256\s*kbps|mp3|flac|lossless|cd\s*rip)[\)\]\}]?\s*").unwrap()
});

static EXTRA_SPACES_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\s+").unwrap()
});

/// Cleans title and parses out artists.
#[derive(Debug, Clone)]
pub struct CleanedTrackMeta {
    pub cleaned_title: String,
    pub formatted_artist: String,
    pub extracted_artists: Vec<String>,
    pub was_title_cleaned: bool,
    pub were_artists_reformatted: bool,
}

pub fn clean_track(raw_title: &str, raw_artist: &str) -> CleanedTrackMeta {
    let mut title = raw_title.trim().to_string();
    let mut initial_artist = raw_artist.trim().to_string();
    let original_title = title.clone();
    let original_artist = initial_artist.clone();

    // 1. Check if artist is empty or generic, and title has "Artist - Song Title" format
    let is_generic_artist = initial_artist.is_empty()
        || initial_artist.eq_ignore_ascii_case("unknown")
        || initial_artist.eq_ignore_ascii_case("unknown artist")
        || initial_artist.eq_ignore_ascii_case("various artists")
        || initial_artist.eq_ignore_ascii_case("youtube");

    if (is_generic_artist || initial_artist.is_empty()) && title.contains(" - ") {
        let parts: Vec<&str> = title.splitn(2, " - ").collect();
        if parts.len() == 2 && !parts[0].trim().is_empty() && !parts[1].trim().is_empty() {
            initial_artist = parts[0].trim().to_string();
            title = parts[1].trim().to_string();
        }
    }

    // 2. Strip file format extensions (.mp3, .flac, .m4a, etc.) and format noise (320kbps, etc.)
    title = FORMAT_EXTENSION_RE.replace(&title, "").to_string();
    title = FORMAT_NOISE_RE.replace_all(&title, " ").to_string();
    initial_artist = FORMAT_EXTENSION_RE.replace(&initial_artist, "").to_string();
    initial_artist = FORMAT_NOISE_RE.replace_all(&initial_artist, " ").to_string();

    // 3. Remove pipe suffixes like "Song Title | Official Audio"
    title = PIPE_SUFFIX_RE.replace(&title, "").to_string();

    // 3. Extract featured artists from title: "Starboy (feat. Daft Punk)"
    let mut extra_artists = Vec::new();
    if let Some(caps) = FEAT_RE.captures(&title) {
        if let Some(m) = caps.get(1) {
            let feat_str = m.as_str().trim();
            for sub in split_artists(feat_str) {
                if !sub.is_empty() {
                    extra_artists.push(sub);
                }
            }
        }
        title = FEAT_RE.replace_all(&title, "").to_string();
    }

    // 4. Strip bracketed video/audio/lyrics noise
    title = BRACKET_NOISE_RE.replace_all(&title, "").to_string();
    title = UNMATCHED_BRACKETS_RE.replace_all(&title, "").to_string();

    // 5. Clean whitespace & quotation marks
    title = EXTRA_SPACES_RE.replace_all(&title, " ").trim().to_string();
    title = title.trim_matches(|c| c == '"' || c == '\'' || c == '`').trim().to_string();

    // If title became empty after aggressive stripping, restore original without outer noise
    if title.is_empty() {
        title = raw_title.trim().to_string();
    }

    // 6. Gather all artists: initial artist + extra artists
    let mut artist_list = split_artists(&initial_artist);
    for extra in extra_artists {
        let exists = artist_list.iter().any(|a| a.eq_ignore_ascii_case(&extra));
        if !exists && !extra.is_empty() {
            artist_list.push(extra);
        }
    }

    if artist_list.is_empty() {
        artist_list.push("Unknown Artist".to_string());
    }

    // 7. Format canonical artist with " / " separator
    let formatted_artist = artist_list.join(" / ");

    let was_title_cleaned = title != original_title;
    let were_artists_reformatted = formatted_artist != original_artist;

    CleanedTrackMeta {
        cleaned_title: title,
        formatted_artist,
        extracted_artists: artist_list,
        was_title_cleaned,
        were_artists_reformatted,
    }
}

/// Splits a raw artist string by multiple possible separators: " / ", " & ", ", ", " feat. ", etc.
pub fn split_artists(raw: &str) -> Vec<String> {
    let mut result = Vec::new();
    if raw.trim().is_empty() {
        return result;
    }

    // Normalize potential separators to a unique token "###"
    let mut s = raw.to_string();
    let separators = [
        " / ",
        " & ",
        " and ",
        " feat. ",
        " ft. ",
        " featuring ",
        " Feat. ",
        " Ft. ",
        ", ",
        "; ",
        " x ",
        " X ",
        " with ",
        " With ",
    ];

    for sep in separators {
        s = s.replace(sep, "###");
    }

    for item in s.split("###") {
        let trimmed = item
            .trim()
            .trim_matches(|c| c == '"' || c == '\'' || c == '`')
            .trim();
        if !trimmed.is_empty() && !result.iter().any(|existing: &String| existing.eq_ignore_ascii_case(trimmed)) {
            result.push(trimmed.to_string());
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_youtube_title() {
        let res = clean_track(
            "Starboy (Official Music Video) [4K] ft. Daft Punk",
            "The Weeknd",
        );
        assert_eq!(res.cleaned_title, "Starboy");
        assert_eq!(res.formatted_artist, "The Weeknd / Daft Punk");
        assert!(res.was_title_cleaned);
    }

    #[test]
    fn test_artist_dash_title() {
        let res = clean_track(
            "Eminem feat. Rihanna - Love The Way You Lie (Audio)",
            "Unknown",
        );
        assert_eq!(res.cleaned_title, "Love The Way You Lie");
        assert_eq!(res.formatted_artist, "Eminem / Rihanna");
    }

    #[test]
    fn test_clean_audio_bracket() {
        let res = clean_track(
            "Levitating [Clean Audio]",
            "Dua Lipa & DaBaby",
        );
        assert_eq!(res.cleaned_title, "Levitating");
        assert_eq!(res.formatted_artist, "Dua Lipa / DaBaby");
    }

    #[test]
    fn test_strip_format_extension_and_mp3_noise() {
        let res = clean_track(
            "Blinding Lights [320kbps MP3].mp3",
            "The Weeknd",
        );
        assert_eq!(res.cleaned_title, "Blinding Lights");
        assert_eq!(res.formatted_artist, "The Weeknd");
    }
}
