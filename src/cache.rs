use crate::{
    config::PluginConfig,
    types::{Lyrics, LyricsKind},
};
use extism_pdk::warn;
use flate2::{Compression, read::DeflateDecoder, write::DeflateEncoder};
use nd_pdk::{host::kvstore, lyrics::Error as LyricsError};
use std::io::{Read, Write};

const PREFIX_NEGATIVE: &str = "miss:";

const SENTINEL: &[u8] = &[1u8];

fn cache_key(track_id: &str, kind: LyricsKind) -> String {
    format!("{}:{track_id}", kind.slug())
}

fn negative_cache_key(track_id: &str) -> String {
    format!("{PREFIX_NEGATIVE}{track_id}")
}

pub enum CacheLookup {
    Found(Lyrics),
    Negative,
    Miss,
}

pub struct LyricsCache {
    ttl: i64,
    negative_ttl: i64,
}

impl LyricsCache {
    pub fn new(ttl_seconds: i64, negative_ttl_seconds: i64) -> Self {
        Self {
            ttl: ttl_seconds,
            negative_ttl: negative_ttl_seconds,
        }
    }

    pub fn lookup(&self, track_id: &str, cfg: &PluginConfig) -> CacheLookup {
        if let Some(lyrics) = self.read(track_id, cfg) {
            return CacheLookup::Found(lyrics);
        }

        if self.is_instrumental(track_id) {
            return CacheLookup::Found(Lyrics::Instrumental);
        }

        if self.is_negative(track_id) {
            return CacheLookup::Negative;
        }

        CacheLookup::Miss
    }

    fn read(&self, track_id: &str, cfg: &PluginConfig) -> Option<Lyrics> {
        cfg.resolve_order()
            .iter()
            .find_map(|&kind| self.get(track_id, kind))
    }

    pub fn write(
        &self,
        track_id: &str,
        lyrics: &Lyrics,
        cfg: &PluginConfig,
    ) -> Result<(), LyricsError> {
        let bytes = match lyrics {
            Lyrics::Instrumental => SENTINEL.to_vec(),
            _ => compress(lyrics.text(cfg).as_bytes())
                .map_err(|e| LyricsError::new(format!("compression failed: {e}")))?,
        };

        kvstore::set_with_ttl(&cache_key(track_id, lyrics.kind()), bytes, self.ttl)
            .map_err(|e| LyricsError::new(format!("failed to write to cache: {e}")))?;

        Ok(())
    }

    fn is_instrumental(&self, track_id: &str) -> bool {
        kvstore::get(&cache_key(track_id, LyricsKind::Instrumental))
            .ok()
            .flatten()
            .is_some()
    }

    fn is_negative(&self, track_id: &str) -> bool {
        kvstore::get(&negative_cache_key(track_id))
            .ok()
            .flatten()
            .is_some()
    }

    pub fn write_negative(&self, track_id: &str) -> Result<(), LyricsError> {
        kvstore::set_with_ttl(
            &negative_cache_key(track_id),
            SENTINEL.to_vec(),
            self.negative_ttl,
        )
        .map_err(|e| LyricsError::new(format!("failed to write negative cache entry: {e}")))
    }

    fn get(&self, track_id: &str, kind: LyricsKind) -> Option<Lyrics> {
        let bytes = kvstore::get(&cache_key(track_id, kind)).ok()??;

        match kind {
            LyricsKind::Instrumental => {
                if bytes == SENTINEL {
                    Some(Lyrics::Instrumental)
                } else {
                    warn!("invalid instrumental cache entry for track {track_id}");
                    None
                }
            }

            _ => match decompress(&bytes) {
                Ok(text) => Some(match kind {
                    LyricsKind::Plain => Lyrics::Plain(text),
                    LyricsKind::Lrc => Lyrics::Lrc(text),
                    LyricsKind::Elrc => Lyrics::Elrc(text),
                    LyricsKind::Ttml => Lyrics::Ttml(text),
                    LyricsKind::Srt => Lyrics::Srt(text),
                    LyricsKind::Lyricsfile => Lyrics::Lyricsfile(text),
                    LyricsKind::Instrumental => unreachable!(),
                }),

                Err(e) => {
                    warn!("cache corruption detected for track {track_id}: {e}");
                    None
                }
            },
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
    fn test_cache_key_lrc() {
        assert_eq!(cache_key("abc123", LyricsKind::Lrc), "lrc:abc123");
    }

    #[test]
    fn test_cache_key_elrc() {
        assert_eq!(cache_key("abc123", LyricsKind::Elrc), "elrc:abc123");
    }

    #[test]
    fn test_cache_key_plain() {
        assert_eq!(cache_key("abc123", LyricsKind::Plain), "plain:abc123");
    }

    #[test]
    fn test_cache_key_instrumental() {
        assert_eq!(
            cache_key("abc123", LyricsKind::Instrumental),
            "instrumental:abc123"
        );
    }

    #[test]
    fn test_negative_cache_key() {
        assert_eq!(negative_cache_key("abc123"), "miss:abc123");
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
