use nd_pdk::{host::config, lyrics::Error as LyricsError};

use crate::LyricsKind;

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
    pub update_lyrics: bool,
    pub enable_cache: bool,
    pub cache_ttl: i64,
}

impl PluginConfig {
    pub fn load() -> Result<Self, LyricsError> {
        Ok(Self {
            lyrics_mode: get_string("lyricsMode")?
                .map(|s| LyricsMode::from_str(&s))
                .unwrap_or(LyricsMode::BothPreferSynced),

            write_lyrics: get_bool("writeLyrics", false)?,
            update_lyrics: get_bool("updateLyrics", false)?,
            enable_cache: get_bool("enableCache", true)?,
            cache_ttl: get_i64("cacheTtl", 0)?,
        })
    }
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
}
