use crate::{config::PluginConfig, types::LyricsType};
use extism_pdk::warn;
use flate2::{Compression, read::DeflateDecoder, write::DeflateEncoder};
use nd_pdk::{host::cache, lyrics::Error as LyricsError};
use std::io::{Read, Write};

const PREFIX_SYNCED: &str = "lrc:synced:";
const PREFIX_PLAIN: &str = "lrc:plain:";

fn cache_key(track_id: &str, kind: LyricsType) -> String {
    let prefix = match kind {
        LyricsType::Synced => PREFIX_SYNCED,
        LyricsType::Plain => PREFIX_PLAIN,
    };
    format!("{prefix}{track_id}")
}

pub struct LyricsCache {
    ttl: i64,
}

#[derive(Debug)]
pub struct CachedLyrics {
    pub text: String,
    pub kind: LyricsType,
}

impl LyricsCache {
    pub fn new(ttl_seconds: i64) -> Self {
        Self { ttl: ttl_seconds }
    }

    pub fn read(&self, track_id: &str, cfg: &PluginConfig) -> Option<CachedLyrics> {
        cfg.resolve_order()
            .iter()
            .find_map(|&kind| self.get(track_id, kind))
    }

    pub fn write(&self, track_id: &str, text: &str, kind: LyricsType) -> Result<(), LyricsError> {
        let compressed = compress(text.as_bytes())
            .map_err(|e| LyricsError::new(format!("compression failed: {e}")))?;

        cache::set_bytes(&cache_key(track_id, kind), compressed, self.ttl)
            .map_err(|e| LyricsError::new(format!("failed to write to cache: {e}")))?;

        Ok(())
    }

    fn get(&self, track_id: &str, kind: LyricsType) -> Option<CachedLyrics> {
        let bytes = cache::get_bytes(&cache_key(track_id, kind)).ok()??;

        match decompress(&bytes) {
            Ok(text) => Some(CachedLyrics { text, kind }),
            Err(e) => {
                warn!("cache corruption detected for track {track_id}: {e}");
                None
            }
        }
    }
}

fn compress(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(data)?;
    encoder.finish()
}

fn decompress(data: &[u8]) -> Result<String, LyricsError> {
    let mut decoder = DeflateDecoder::new(data);
    let mut bytes = Vec::new();
    decoder
        .read_to_end(&mut bytes)
        .map_err(|e| LyricsError::new(format!("decompression failed: {e}")))?;

    String::from_utf8(bytes).map_err(|e| LyricsError::new(format!("invalid UTF-8 in cache: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_synced() {
        assert_eq!(cache_key("abc123", LyricsType::Synced), "lrc:synced:abc123");
    }

    #[test]
    fn test_cache_key_plain() {
        assert_eq!(cache_key("abc123", LyricsType::Plain), "lrc:plain:abc123");
    }

    #[test]
    fn test_compress_decompress_roundtrip() {
        let original = "[00:12.34] Hello, world!\n[00:15.67] Testing...";
        let compressed = compress(original.as_bytes()).expect("compression failed");
        let decompressed = decompress(&compressed).expect("decompression failed");

        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_compress_decompress_empty() {
        let compressed = compress(&[]).expect("compression of empty failed");
        let decompressed = decompress(&compressed).expect("decompression of empty failed");

        assert_eq!(decompressed, "");
    }

    #[test]
    fn test_compress_decompress_large_payload() {
        let original = "Lorem ipsum ".repeat(8_000);
        let compressed = compress(original.as_bytes()).expect("compression failed");

        assert!(
            compressed.len() < original.len(),
            "compressed payload should be smaller"
        );

        let decompressed = decompress(&compressed).expect("decompression failed");
        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_decompress_invalid_deflate_data() {
        let garbage = vec![0xFF, 0xFE, 0xFD, 0xFC];
        let result = decompress(&garbage);

        assert!(result.is_err());
    }

    #[test]
    fn test_decompress_valid_deflate_but_invalid_utf8() {
        let invalid_utf8: &[u8] = &[0x80, 0x81, 0x82];
        let compressed = compress(invalid_utf8).expect("compression failed");
        let result = decompress(&compressed);

        assert!(result.is_err());
    }
}
