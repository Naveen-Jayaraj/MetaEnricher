pub mod cleaner;
pub mod deezer;
pub mod enricher;
pub mod itunes;
pub mod lyrics;
pub mod musicbrainz;
pub mod web_search;

pub use enricher::MetadataEnricher;
pub use lyrics::LyricsAggregator;
