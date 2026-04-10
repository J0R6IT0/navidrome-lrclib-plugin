use nd_pdk::{host::config, lyrics::Error as LyricsError};

pub struct PluginConfig {
    pub fetch_synced: bool,
    pub write_lyrics: bool,
    pub enable_cache: bool,
    pub cache_ttl: i64,
}

impl PluginConfig {
    pub fn load() -> Result<Self, LyricsError> {
        Ok(Self {
            fetch_synced: get_bool("fetchSyncedLyrics", true)?,
            write_lyrics: get_bool("writeLyrics", false)?,
            enable_cache: get_bool("enableCache", true)?,
            cache_ttl: get_i64("cacheTtl", 0)?,
        })
    }
}

fn get_bool(key: &str, default: bool) -> Result<bool, LyricsError> {
    config::get(key)
        .map_err(|e| LyricsError::new(e.to_string()))
        .map(|v| v.map(|s| s == "true").unwrap_or(default))
}

fn get_i64(key: &str, default: i64) -> Result<i64, LyricsError> {
    config::get(key)
        .map_err(|e| LyricsError::new(e.to_string()))
        .map(|v| v.and_then(|s| s.parse().ok()).unwrap_or(default))
}
