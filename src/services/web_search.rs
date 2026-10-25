use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::process::Command;

static MOVIE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:movie|film|album|soundtrack|from\s+the\s+movie|from\s+the\s+film|from)\s*[:\-]?\s*[\x22']?([A-Za-z0-9\.\s]+?)(?:[\x22']|\s*\(|\s*\||\s*[\r\n]|,\s*(?:singer|lyrics|music|starring))")
        .unwrap()
});

static SINGER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:singer|singers|sung\s+by|vocals|vocalist|playback)\s*[:\-]\s*([^\r\n\|<>,]+)")
        .unwrap()
});

static MUSIC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:music\s+director|music\s+by|composed\s+by|composer|music)\s*[:\-]\s*([^\r\n\|<>,]+)")
        .unwrap()
});

static LYRICS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:lyrics\s+by|lyricist|lyrics)\s*[:\-]\s*([^\r\n\|<>,]+)")
        .unwrap()
});

static PRODUCER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:produced\s+by|producer)\s*[:\-]\s*([^\r\n\|<>,]+)")
        .unwrap()
});

static SNIPPET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)<a class="result__snippet"[^>]*>(.*?)</a>"#).unwrap()
});

static TAG_STRIP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<[^>]*>").unwrap()
});

#[derive(Debug, Clone, Default)]
pub struct DisambiguationResult {
    pub resolved_title: Option<String>,
    pub resolved_singers: Vec<String>,
    pub resolved_movie_album: Option<String>,
    pub resolved_composers: Vec<String>,
    pub resolved_producers: Vec<String>,
    pub resolved_lyricists: Vec<String>,
    pub detected_language: Option<String>,
    pub was_intermixed_or_swapped: bool,
    pub raw_snippet: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WebSearchService {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct YtDlpDump {
    title: Option<String>,
    description: Option<String>,
}

impl WebSearchService {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(6))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client }
    }

    /// Performs deep web search and disambiguation for tracks with intermixed artists, movie names, or swapped titles.
    pub async fn disambiguate(&self, raw_title: &str, raw_artist: &str) -> DisambiguationResult {
        let mut result = DisambiguationResult::default();
        let query_text = format!("{} {}", raw_title, raw_artist);

        // 1. Check if title and artist might be swapped / intermixed
        // E.g., if raw_title is 1-2 words representing a known artist/singer and raw_artist looks like a full song line
        let might_be_swapped = looks_like_song_title(raw_artist) && looks_like_artist_name(raw_title);

        // 2. Fetch web search snippets via DuckDuckGo HTML
        let web_snippets = self.fetch_ddg_snippets(&query_text).await;

        // 3. Supplement with YouTube metadata description using yt-dlp
        let yt_metadata = self.fetch_yt_metadata(&query_text).await;

        let combined_text = format!(
            "{}\n{}\n{}",
            web_snippets.join("\n"),
            yt_metadata.as_ref().and_then(|y| y.title.clone()).unwrap_or_default(),
            yt_metadata.as_ref().and_then(|y| y.description.clone()).unwrap_or_default()
        );

        if combined_text.trim().is_empty() {
            return result;
        }

        result.raw_snippet = Some(combined_text.chars().take(500).collect());

        // 4. Extract Singer / Artist
        if let Some(caps) = SINGER_RE.captures(&combined_text) {
            if let Some(m) = caps.get(1) {
                let s = clean_extracted_name(m.as_str());
                if !s.is_empty() {
                    result.resolved_singers.push(s);
                }
            }
        }

        // 5. Extract Music Director / Composer
        if let Some(caps) = MUSIC_RE.captures(&combined_text) {
            if let Some(m) = caps.get(1) {
                let s = clean_extracted_name(m.as_str());
                if !s.is_empty() && !result.resolved_composers.contains(&s) {
                    result.resolved_composers.push(s);
                }
            }
        }

        // 6. Extract Lyricist
        if let Some(caps) = LYRICS_RE.captures(&combined_text) {
            if let Some(m) = caps.get(1) {
                let s = clean_extracted_name(m.as_str());
                if !s.is_empty() && !result.resolved_lyricists.contains(&s) {
                    result.resolved_lyricists.push(s);
                }
            }
        }

        // 7. Extract Movie / Film / Album Name
        if let Some(caps) = MOVIE_RE.captures(&combined_text) {
            if let Some(m) = caps.get(1) {
                let s = clean_extracted_name(m.as_str());
                if !s.is_empty() && s.len() > 1 && s.len() < 50 {
                    result.resolved_movie_album = Some(s);
                }
            }
        }

        // 8. Extract Producer
        if let Some(caps) = PRODUCER_RE.captures(&combined_text) {
            if let Some(m) = caps.get(1) {
                let s = clean_extracted_name(m.as_str());
                if !s.is_empty() {
                    result.resolved_producers.push(s);
                }
            }
        }

        // 9. Detect Language from snippet
        let lower_text = combined_text.to_lowercase();
        if lower_text.contains("malayalam") {
            result.detected_language = Some("Malayalam".to_string());
        } else if lower_text.contains("tamil") {
            result.detected_language = Some("Tamil".to_string());
        } else if lower_text.contains("hindi") || lower_text.contains("bollywood") {
            result.detected_language = Some("Hindi".to_string());
        } else if lower_text.contains("telugu") {
            result.detected_language = Some("Telugu".to_string());
        } else if lower_text.contains("kannada") {
            result.detected_language = Some("Kannada".to_string());
        } else if lower_text.contains("punjabi") {
            result.detected_language = Some("Punjabi".to_string());
        } else if lower_text.contains("spanish") {
            result.detected_language = Some("Spanish".to_string());
        } else if lower_text.contains("korean") {
            result.detected_language = Some("Korean".to_string());
        } else if lower_text.contains("japanese") {
            result.detected_language = Some("Japanese".to_string());
        }

        // 10. Check if title contains a piped format like: "Song Name | Movie Name | Singer | Music"
        if let Some(yt) = &yt_metadata {
            if let Some(title_str) = &yt.title {
                let parts: Vec<&str> = title_str.split('|').map(|p| p.trim()).collect();
                if parts.len() >= 2 {
                    // First part is usually the canonical song title
                    let cand_title = parts[0].trim();
                    if !cand_title.is_empty() {
                        result.resolved_title = Some(cand_title.to_string());
                    }
                    // Look through remaining parts for movie or artist tokens
                    for part in &parts[1..] {
                        let trimmed = part.trim();
                        // If one matches movie format or movie candidate
                        if result.resolved_movie_album.is_none() && trimmed.len() >= 2 && trimmed.len() <= 25 {
                            result.resolved_movie_album = Some(trimmed.to_string());
                        }
                    }
                }
            }
        }

        // 11. Disentangle tokens from input artist:
        // E.g., user passed: "Pa Va, P Jayachandran, Santhosh Varma, Anand Madhusoodanan"
        let artist_tokens: Vec<&str> = raw_artist.split(&[',', ';', '/'][..]).map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        if artist_tokens.len() > 1 {
            for token in &artist_tokens {
                let lower_tok = token.to_lowercase();
                // Check if this token matches movie / album
                if let Some(ref mov) = result.resolved_movie_album {
                    if mov.to_lowercase().contains(&lower_tok) || lower_tok.contains(&mov.to_lowercase()) {
                        // Found movie in artist field!
                        result.was_intermixed_or_swapped = true;
                    }
                }
                // Check if this token is actually composer/music director
                for comp in &result.resolved_composers {
                    if comp.to_lowercase().contains(&lower_tok) || lower_tok.contains(&comp.to_lowercase()) {
                        result.was_intermixed_or_swapped = true;
                    }
                }
                // Check if this token is lyricist
                for lyr in &result.resolved_lyricists {
                    if lyr.to_lowercase().contains(&lower_tok) || lower_tok.contains(&lyr.to_lowercase()) {
                        result.was_intermixed_or_swapped = true;
                    }
                }
            }
        }

        if might_be_swapped {
            result.was_intermixed_or_swapped = true;
        }

        result
    }

    async fn fetch_ddg_snippets(&self, query: &str) -> Vec<String> {
        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            urlencoding::encode(query)
        );

        let mut snippets = Vec::new();
        if let Ok(resp) = self.client.get(&url).send().await {
            if let Ok(html) = resp.text().await {
                for caps in SNIPPET_RE.captures_iter(&html) {
                    if let Some(m) = caps.get(1) {
                        let clean = TAG_STRIP_RE.replace_all(m.as_str(), "").to_string();
                        snippets.push(clean.trim().to_string());
                        if snippets.len() >= 6 {
                            break;
                        }
                    }
                }
            }
        }
        snippets
    }

    async fn fetch_yt_metadata(&self, query: &str) -> Option<YtDlpDump> {
        let search_arg = format!("ytsearch1:{}", query);
        let output = Command::new("yt-dlp")
            .args(&[
                "--dump-json",
                "--no-warnings",
                "--no-playlist",
                &search_arg,
            ])
            .output()
            .await
            .ok()?;

        if output.status.success() {
            let json_str = String::from_utf8_lossy(&output.stdout);
            serde_json::from_str::<YtDlpDump>(&json_str).ok()
        } else {
            None
        }
    }
}

fn clean_extracted_name(raw: &str) -> String {
    raw.trim()
        .trim_matches(|c| c == ':' || c == '-' || c == '|' || c == '"' || c == '\'' || c == '.' || c == ',')
        .trim()
        .to_string()
}

pub fn looks_like_song_title(s: &str) -> bool {
    let lower = s.to_lowercase();
    let word_count = s.split_whitespace().count();
    word_count >= 3
        || lower.contains("kaalam")
        || lower.contains("song")
        || lower.contains("lyrics")
        || lower.contains("theme")
        || lower.contains("remix")
        || lower.contains("video")
        || lower.contains("audio")
}

pub fn looks_like_artist_name(s: &str) -> bool {
    let word_count = s.split_whitespace().count();
    word_count <= 2 || s.contains('.')
}
