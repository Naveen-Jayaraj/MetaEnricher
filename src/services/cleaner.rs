use regex::Regex;
use std::sync::LazyLock;

static FILE_EXTENSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\.(?:mp3|flac|m4a|wav|aac|ogg|opus|webm|wma|alac|aiff|ape)$").unwrap()
});

static VIDEO_NOISE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s*[(\[{]\s*(?:(?:official\s+)?(?:(?:music\s+|lyric[s]?\s+)?video|(?:clean\s+)?audio|visuali[sz]er)|lyrics?|4k|60fps|1080p|720p|hd|hq|(?:\d{2,3}\s*kbps\s*)?(?:mp3|flac|lossless)|\d{2,3}\s*kbps|cd\s*rip)\s*[)\]}]").unwrap()
});

static FEATURE_CREDIT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:\s*[(\[]\s*|\s+)(?:feat\.?|ft\.?|featuring)\s+([^\r\n)\]\[|(]+)[)\]]?")
        .unwrap()
});

static ARTIST_SEPARATOR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s*[,;·]\s*|\s+/\s+|\s+(?:&|and|x|with|feat\.?|ft\.?|featuring)\s+").unwrap()
});

static CHANNEL_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:\s+-\s+topic|\s*vevo)$").unwrap());

static VERSION_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:live|remix|mix|karaoke|instrumental|acoustic|cover|sped[ -]up|slowed|nightcore|demo|stripped|edit|extended|remaster(?:ed)?|session|alternate|version|reprise)\b").unwrap()
});

#[derive(Debug)]
pub struct CleanedTrackMeta {
    pub cleaned_title: String,
    pub extracted_artists: Vec<String>,
}

pub fn clean_track(raw_title: &str, raw_artist: &str) -> CleanedTrackMeta {
    let title = strip_title_noise(raw_title);
    let artist = clean_artist(raw_artist);
    let mut artist = artist.as_str();

    if let Some((prefix, suffix)) = title.split_once(" - ") {
        if !prefix.trim().is_empty()
            && !suffix.trim().is_empty()
            && (prefix.trim().eq_ignore_ascii_case(artist)
                || (is_unknown_artist(artist) && !VERSION_SUFFIX.is_match(suffix)))
        {
            let parsed_artist = clean_artist(prefix);
            return finish_title(suffix, &parsed_artist);
        }
    }

    if is_unknown_artist(artist) {
        artist = "";
    }
    finish_title(&title, artist)
}

pub fn clean_catalog_title(raw_title: &str) -> String {
    finish_title(&strip_title_noise(raw_title), "").cleaned_title
}

pub fn featured_artists(title: &str) -> Vec<String> {
    FEATURE_CREDIT
        .captures_iter(title.split('|').next().unwrap_or(title))
        .flat_map(|credits| split_artists(&credits[1]))
        .collect()
}

fn strip_title_noise(raw: &str) -> String {
    let title = FILE_EXTENSION.replace(raw.trim(), "");
    VIDEO_NOISE
        .replace_all(&title.replace(['–', '—'], "-"), "")
        .into_owned()
}

fn finish_title(title: &str, artist: &str) -> CleanedTrackMeta {
    let mut artists = split_artists(artist);
    let base = match title.split_once('|') {
        Some((head, tail)) if !VERSION_SUFFIX.is_match(tail) => head,
        _ => title,
    };
    for name in featured_artists(base) {
        if !artists.iter().any(|a| a.eq_ignore_ascii_case(&name)) {
            artists.push(name);
        }
    }
    let cleaned_title = collapse_spaces(&FEATURE_CREDIT.replace_all(base, ""));
    let cleaned_title = if cleaned_title.is_empty() {
        title.trim().to_owned()
    } else {
        cleaned_title
    };
    CleanedTrackMeta {
        cleaned_title,
        extracted_artists: artists,
    }
}

fn clean_artist(raw: &str) -> String {
    let name = FILE_EXTENSION.replace(raw.trim(), "");
    let name = VIDEO_NOISE.replace_all(&name, "");
    collapse_spaces(&CHANNEL_SUFFIX.replace(&name, ""))
}

fn collapse_spaces(raw: &str) -> String {
    raw.trim_matches([' ', '"', '\''])
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_unknown_artist(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "" | "unknown" | "unknown artist" | "unknown channel" | "various artists" | "youtube"
    )
}

pub fn split_artists(raw: &str) -> Vec<String> {
    let mut artists = Vec::new();
    for credit in ARTIST_SEPARATOR.split(raw) {
        let name = collapse_spaces(credit);
        if !is_unknown_artist(&name)
            && !artists
                .iter()
                .any(|a: &String| a.eq_ignore_ascii_case(&name))
        {
            artists.push(name);
        }
    }
    artists
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_youtube_title() {
        let res = clean_track(
            "Starboy (Official Music Video) [4K] ft. Daft Punk",
            "The Weeknd",
        );
        assert_eq!(res.cleaned_title, "Starboy");
        assert_eq!(res.extracted_artists, ["The Weeknd", "Daft Punk"]);
    }

    #[test]
    fn artist_dash_title() {
        let res = clean_track(
            "Eminem feat. Rihanna - Love The Way You Lie (Audio)",
            "Unknown",
        );
        assert_eq!(res.cleaned_title, "Love The Way You Lie");
        assert_eq!(res.extracted_artists, ["Eminem", "Rihanna"]);
    }

    #[test]
    fn clean_audio_bracket() {
        let res = clean_track("Levitating [Clean Audio]", "Dua Lipa & DaBaby");
        assert_eq!(res.cleaned_title, "Levitating");
        assert_eq!(res.extracted_artists, ["Dua Lipa", "DaBaby"]);
    }

    #[test]
    fn strip_format_extension_and_mp3_noise() {
        let res = clean_track("Blinding Lights [320kbps MP3].mp3", "The Weeknd");
        assert_eq!(res.cleaned_title, "Blinding Lights");
        assert_eq!(res.extracted_artists, ["The Weeknd"]);
    }

    #[test]
    fn keep_recording_versions_and_words_that_resemble_noise() {
        let res = clean_track("Live Forever (Live at Knebworth) [HD]", "Oasis");
        assert_eq!(res.cleaned_title, "Live Forever (Live at Knebworth)");
        let res = clean_track("Flacquer (Audio, Video, Disco)", "AC/DC");
        assert_eq!(res.cleaned_title, "Flacquer (Audio, Video, Disco)");
        assert_eq!(res.extracted_artists, ["AC/DC"]);
    }
}
