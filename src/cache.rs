use crate::{LyricsKind, config::LyricsMode};
use flate2::{Compression, read::DeflateDecoder, write::DeflateEncoder};
use nd_pdk::{host::cache, lyrics::Error as LyricsError};
use std::io::{Read, Write};

const PREFIX_SYNCED: &str = "lrc:synced:";
const PREFIX_PLAIN: &str = "lrc:plain:";
const DEFAULT_TTL: i64 = 86_400;

pub struct LyricsCache {
    ttl: i64,
}

impl LyricsCache {
    pub fn new(ttl_seconds: i64) -> Self {
        Self {
            ttl: if ttl_seconds > 0 {
                ttl_seconds
            } else {
                DEFAULT_TTL
            },
        }
    }

    pub fn read(&self, track_id: &str, mode: LyricsMode) -> Option<String> {
        let order = mode.resolve_order();

        for kind in order {
            if let Some(text) = self.get(track_id, *kind) {
                return Some(text);
            }
        }

        None
    }

    pub fn write(&self, track_id: &str, text: &str, kind: LyricsKind) -> Result<(), LyricsError> {
        let compressed = compress(text.as_bytes())
            .map_err(|e| LyricsError::new(format!("compression failed: {e}")))?;

        cache::set_bytes(&cache_key(track_id, kind), compressed, self.ttl)
            .map_err(|e| LyricsError::new(format!("failed to write to cache: {e}")))?;

        Ok(())
    }

    fn get(&self, track_id: &str, kind: LyricsKind) -> Option<String> {
        let bytes = cache::get_bytes(&cache_key(track_id, kind)).ok()??;
        decompress(&bytes).ok()
    }
}

fn cache_key(track_id: &str, kind: LyricsKind) -> String {
    let prefix = match kind {
        LyricsKind::Synchronized => PREFIX_SYNCED,
        LyricsKind::Plain => PREFIX_PLAIN,
    };
    format!("{}{}", prefix, track_id)
}

fn compress(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    encoder.finish()
}

fn decompress(data: &[u8]) -> Result<String, std::io::Error> {
    let mut decoder = DeflateDecoder::new(data);
    let mut out = String::new();
    decoder.read_to_string(&mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_decompress_roundtrip() {
        let input = "some lyrics text\nwith multiple lines";

        let compressed = compress(input.as_bytes()).unwrap();
        let decompressed = decompress(&compressed).unwrap();

        assert_eq!(decompressed, input);
    }

    #[test]
    fn test_decompress_invalid_data_fails() {
        let invalid = b"not compressed data";

        assert!(decompress(invalid).is_err());
    }

    #[test]
    fn test_cache_key_generation() {
        let id = "track123";

        let synced = cache_key(id, LyricsKind::Synchronized);
        let plain = cache_key(id, LyricsKind::Plain);

        assert_eq!(synced, "lrc:synced:track123");
        assert_eq!(plain, "lrc:plain:track123");
    }

    #[test]
    fn test_cache_ttl_default() {
        let cache = LyricsCache::new(0);
        assert_eq!(cache.ttl, DEFAULT_TTL);

        let cache = LyricsCache::new(-10);
        assert_eq!(cache.ttl, DEFAULT_TTL);
    }

    #[test]
    fn test_cache_ttl_custom() {
        let cache = LyricsCache::new(123);
        assert_eq!(cache.ttl, 123);
    }
}
