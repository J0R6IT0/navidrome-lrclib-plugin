use super::host::get_i64;
use crate::{config::Result, types::LyricsKind};
use extism_pdk::warn;

pub(super) const DEFAULT_CACHE_TTL: i64 = 168;
pub(super) const DEFAULT_NEGATIVE_CACHE_TTL: i64 = 24;

const MIN_CACHE_TTL: i64 = 1;
const MAX_CACHE_TTL: i64 = 1_000_000;

const DEFAULT_TTML_CACHE_TTL: i64 = 336;
const DEFAULT_LYRICSFILE_CACHE_TTL: i64 = 168;
const DEFAULT_ELRC_CACHE_TTL: i64 = 336;
const DEFAULT_LRC_CACHE_TTL: i64 = 168;
const DEFAULT_SRT_CACHE_TTL: i64 = 168;
const DEFAULT_PLAIN_CACHE_TTL: i64 = 72;
const DEFAULT_INSTRUMENTAL_CACHE_TTL: i64 = 336;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeCacheTtls {
    pub plain: i64,
    pub lrc: i64,
    pub elrc: i64,
    pub ttml: i64,
    pub srt: i64,
    pub lyricsfile: i64,
    pub instrumental: i64,
}

impl TypeCacheTtls {
    pub fn get(&self, kind: LyricsKind) -> i64 {
        match kind {
            LyricsKind::Plain => self.plain,
            LyricsKind::Lrc => self.lrc,
            LyricsKind::Elrc => self.elrc,
            LyricsKind::Ttml => self.ttml,
            LyricsKind::Srt => self.srt,
            LyricsKind::Lyricsfile => self.lyricsfile,
            LyricsKind::Instrumental => self.instrumental,
        }
    }
}

impl Default for TypeCacheTtls {
    fn default() -> Self {
        Self {
            plain: DEFAULT_PLAIN_CACHE_TTL,
            lrc: DEFAULT_LRC_CACHE_TTL,
            elrc: DEFAULT_ELRC_CACHE_TTL,
            ttml: DEFAULT_TTML_CACHE_TTL,
            srt: DEFAULT_SRT_CACHE_TTL,
            lyricsfile: DEFAULT_LYRICSFILE_CACHE_TTL,
            instrumental: DEFAULT_INSTRUMENTAL_CACHE_TTL,
        }
    }
}

pub(super) fn resolve_global() -> Result<i64> {
    resolve("cacheTtlHours", DEFAULT_CACHE_TTL)
}

pub(super) fn resolve_negative() -> Result<i64> {
    resolve("negativeCacheTtlHours", DEFAULT_NEGATIVE_CACHE_TTL)
}

pub(super) fn resolve_per_type() -> Result<TypeCacheTtls> {
    Ok(TypeCacheTtls {
        plain: resolve("plainCacheTtlHours", DEFAULT_PLAIN_CACHE_TTL)?,
        lrc: resolve("lrcCacheTtlHours", DEFAULT_LRC_CACHE_TTL)?,
        elrc: resolve("elrcCacheTtlHours", DEFAULT_ELRC_CACHE_TTL)?,
        ttml: resolve("ttmlCacheTtlHours", DEFAULT_TTML_CACHE_TTL)?,
        srt: resolve("srtCacheTtlHours", DEFAULT_SRT_CACHE_TTL)?,
        lyricsfile: resolve("lyricsfileCacheTtlHours", DEFAULT_LYRICSFILE_CACHE_TTL)?,
        instrumental: resolve("instrumentalCacheTtlHours", DEFAULT_INSTRUMENTAL_CACHE_TTL)?,
    })
}

fn resolve(key: &str, default_value: i64) -> Result<i64> {
    Ok(clamp(key, get_i64(key, default_value)?))
}

fn clamp(key: &str, raw: i64) -> i64 {
    let clamped = raw.clamp(MIN_CACHE_TTL, MAX_CACHE_TTL);
    if clamped != raw {
        warn!("{key} {raw}h is out of range, clamping to {clamped}h");
    }

    clamped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_ttl_fits_go_duration() {
        let max_seconds = MAX_CACHE_TTL.saturating_mul(3600);
        assert!(max_seconds.checked_mul(1_000_000_000).is_some());
    }

    #[test]
    fn test_default_ttls_favour_synced_formats() {
        let ttls = TypeCacheTtls::default();

        assert!(ttls.ttml > ttls.lrc);
        assert!(ttls.elrc > ttls.lrc);
        assert!(ttls.lrc > ttls.plain);
    }
}
