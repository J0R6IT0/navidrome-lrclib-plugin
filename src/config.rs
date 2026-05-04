use crate::LyricsKind;
use nd_pdk::{host::config, lyrics::Error as LyricsError};

#[derive(Debug, Clone, Copy)]
pub enum LyricsMode {
    BothPreferSynced,
    BothPreferPlain,
    SyncedOnly,
    PlainOnly,
}

impl LyricsMode {
    fn from_str(s: &str) -> Self {
        match s {
            "plain_only" => LyricsMode::PlainOnly,
            "synced_only" => LyricsMode::SyncedOnly,
            "both_prioritize_plain" => LyricsMode::BothPreferPlain,
            _ => LyricsMode::BothPreferSynced,
        }
    }

    pub fn resolve_order(self) -> &'static [LyricsKind] {
        match self {
            LyricsMode::SyncedOnly => &[LyricsKind::Synchronized],
            LyricsMode::PlainOnly => &[LyricsKind::Plain],
            LyricsMode::BothPreferSynced => &[LyricsKind::Synchronized, LyricsKind::Plain],
            LyricsMode::BothPreferPlain => &[LyricsKind::Plain, LyricsKind::Synchronized],
        }
    }
}

pub struct PluginConfig {
    pub lyrics_mode: LyricsMode,
    pub write_lyrics: bool,
    pub overwrite_lyrics: bool,
    pub plain_extension: String,
    pub synced_extension: String,
    pub enable_cache: bool,
    pub cache_ttl: i64,
    pub providers: Vec<String>,
}

impl PluginConfig {
    pub fn load() -> Result<Self, LyricsError> {
        Ok(Self {
            lyrics_mode: get_string("lyricsMode")?
                .map(|s| LyricsMode::from_str(&s))
                .unwrap_or(LyricsMode::BothPreferSynced),
            write_lyrics: get_bool("writeLyrics", false)?,
            overwrite_lyrics: get_bool("overwriteLyrics", false)?,
            plain_extension: get_string("plainExtension")?
                .map(|s| normalize_extension(&s))
                .unwrap_or_else(|| "txt".to_string()),
            synced_extension: get_string("syncedExtension")?
                .map(|s| normalize_extension(&s))
                .unwrap_or_else(|| "lrc".to_string()),
            enable_cache: get_bool("enableCache", true)?,
            cache_ttl: get_i64("cacheTtl", 86400)?,
            providers: get_string("providers")?
                .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
                .unwrap_or_default(),
        })
    }
}

fn normalize_extension(ext: &str) -> String {
    ext.trim().trim_start_matches('.').to_string()
}

fn get_string(key: &str) -> Result<Option<String>, LyricsError> {
    config::get(key).map_err(|e| LyricsError::new(e.to_string()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lyrics_mode_from_str() {
        assert!(matches!(
            LyricsMode::from_str("plain_only"),
            LyricsMode::PlainOnly
        ));
        assert!(matches!(
            LyricsMode::from_str("synced_only"),
            LyricsMode::SyncedOnly
        ));
        assert!(matches!(
            LyricsMode::from_str("both_prioritize_plain"),
            LyricsMode::BothPreferPlain
        ));
        assert!(matches!(
            LyricsMode::from_str("both_prioritize_synced"),
            LyricsMode::BothPreferSynced
        ));
        assert!(matches!(
            LyricsMode::from_str("unknown"),
            LyricsMode::BothPreferSynced
        ));
    }

    #[test]
    fn test_lyrics_mode_resolve_order() {
        assert_eq!(
            LyricsMode::BothPreferSynced.resolve_order(),
            &[LyricsKind::Synchronized, LyricsKind::Plain]
        );
        assert_eq!(
            LyricsMode::BothPreferPlain.resolve_order(),
            &[LyricsKind::Plain, LyricsKind::Synchronized]
        );
        assert_eq!(
            LyricsMode::SyncedOnly.resolve_order(),
            &[LyricsKind::Synchronized]
        );
        assert_eq!(LyricsMode::PlainOnly.resolve_order(), &[LyricsKind::Plain]);
    }

    #[test]
    fn test_normalize_extension() {
        assert_eq!(normalize_extension("lrc"), "lrc");
        assert_eq!(normalize_extension(".lrc"), "lrc");
        assert_eq!(normalize_extension("...lrc"), "lrc");
        assert_eq!(normalize_extension("  .txt  "), "txt");
        assert_eq!(normalize_extension("."), "");
    }
}
