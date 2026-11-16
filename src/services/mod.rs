mod cleaner;
pub mod db;
mod deezer;
mod enricher;
mod itunes;
pub mod lyrics;
mod matching;
mod musicbrainz;
pub mod uptime;
mod web_search;

pub use db::DatabaseService;
pub use enricher::MetadataEnricher;
pub use lyrics::LyricsAggregator;
pub use uptime::UptimeManager;

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(2))
        .timeout(std::time::Duration::from_secs(3))
        .user_agent(concat!(
            "MetaEnricher/",
            env!("CARGO_PKG_VERSION"),
            " (https://github.com/Naveen-Jayaraj/OfflineMusic)"
        ))
        .build()
        .expect("metadata HTTP client")
}
