use crate::{
    config::PluginConfig,
    types::{Lyrics, LyricsKind},
};
use extism_pdk::warn;
use flate2::{Compression, read::DeflateDecoder, write::DeflateEncoder};
use nd_pdk::{host::kvstore, lyrics::Error as LyricsError};
use std::{
    io::{Read, Write},
    rc::Rc,
};

const PREFIX_NEGATIVE: &str = "miss:";

const SENTINEL: &[u8] = &[1u8];

fn cache_key(track_id: &str, kind: LyricsKind) -> String {
    format!("{}:{track_id}", kind.slug())
}

fn negative_cache_key(track_id: &str, provider: &str) -> String {
    format!("{PREFIX_NEGATIVE}{provider}:{track_id}")
}

pub enum CacheLookup {
    Found(Lyrics),
    Miss,
}

pub struct LyricsCache {
    cfg: Rc<PluginConfig>,
}

impl LyricsCache {
    pub fn new(cfg: Rc<PluginConfig>) -> Self {
        Self { cfg }
    }

    pub fn lookup(&self, track_id: &str) -> CacheLookup {
        if let Some(lyrics) = self.read(track_id) {
            return CacheLookup::Found(lyrics);
        }

        if self.is_instrumental(track_id) {
            return CacheLookup::Found(Lyrics::Instrumental);
        }

        CacheLookup::Miss
    }

    fn read(&self, track_id: &str) -> Option<Lyrics> {
        self.cfg
            .lyrics_type_priority
            .iter()
            .find_map(|&kind| self.get(track_id, kind))
    }

    pub fn write(&self, track_id: &str, lyrics: &Lyrics) -> Result<(), LyricsError> {
        let bytes = match lyrics {
            Lyrics::Instrumental => SENTINEL.to_vec(),
            _ => compress(lyrics.text(&self.cfg).as_bytes())
                .map_err(|e| LyricsError::new(format!("compression failed: {e}")))?,
        };

        let kind = lyrics.kind();
        let ttl = self.cfg.cache_ttl_hours_for(kind).saturating_mul(3600);

        kvstore::set_with_ttl(&cache_key(track_id, kind), bytes, ttl)
            .map_err(|e| LyricsError::new(format!("failed to write to cache: {e}")))?;

        Ok(())
    }

    fn is_instrumental(&self, track_id: &str) -> bool {
        kvstore::get(&cache_key(track_id, LyricsKind::Instrumental))
            .ok()
            .flatten()
            .is_some()
    }

    pub fn is_negative(&self, track_id: &str, provider: &str) -> bool {
        kvstore::get(&negative_cache_key(track_id, provider))
            .ok()
            .flatten()
            .is_some()
    }

    pub fn write_negative(&self, track_id: &str, provider: &str) -> Result<(), LyricsError> {
        kvstore::set_with_ttl(
            &negative_cache_key(track_id, provider),
            SENTINEL.to_vec(),
            self.cfg.negative_cache_ttl_hours.saturating_mul(3600),
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
        assert_eq!(
            negative_cache_key("abc123", "deadbeef"),
            "miss:deadbeef:abc123"
        );
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
