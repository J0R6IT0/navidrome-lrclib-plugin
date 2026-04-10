use nd_pdk::{host::config, lyrics::Error as LyricsError};

pub struct PluginConfig {
    pub fetch_synced: bool,
    pub use_search_fallback: bool,
    pub write_lyrics: bool,
}

impl PluginConfig {
    pub fn load() -> Result<Self, LyricsError> {
        Ok(Self {
            fetch_synced: get_bool("fetchSyncedLyrics", true)?,
            use_search_fallback: get_bool("useSearchFallback", true)?,
            write_lyrics: get_bool("writeLyrics", false)?,
        })
    }
}

fn get_bool(key: &str, default: bool) -> Result<bool, LyricsError> {
    config::get(key)
        .map_err(|e| LyricsError::new(e.to_string()))
        .map(|v| v.map(|s| s == "true").unwrap_or(default))
}
