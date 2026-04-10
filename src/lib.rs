use crate::{cache::LyricsCache, storage::LyricsKind};
use config::PluginConfig;
use extism_pdk::warn;
use lrclib::fetch_lyrics_text;
use nd_pdk::lyrics::{
    Error as LyricsError, GetLyricsRequest, GetLyricsResponse, Lyrics, LyricsText,
};
use storage::LyricsStorage;

mod cache;
mod config;
mod lrclib;
mod storage;

#[derive(Default)]
struct Plugin;

nd_pdk::register_lyrics!(Plugin);

impl Lyrics for Plugin {
    fn get_lyrics(&self, req: GetLyricsRequest) -> Result<GetLyricsResponse, LyricsError> {
        let track = req.track;
        let cfg = PluginConfig::load()?;

        let cache = cfg.enable_cache.then(|| LyricsCache::new(cfg.cache_ttl));
        let storage = cfg.write_lyrics.then(|| LyricsStorage::new()).transpose()?;

        if let Some(ref cache) = cache {
            if let Some(cached) = cache.read(&track.id, cfg.fetch_synced) {
                return Ok(make_response(cached));
            }
        }

        if let Some(ref storage) = storage {
            if let Some(stored) = storage.read(&track.id, cfg.fetch_synced)? {
                return Ok(make_response(stored));
            }
        }

        let (text, kind): (String, LyricsKind) =
            fetch_lyrics_text(&track, &cfg)?.ok_or_else(|| LyricsError::new("no lyrics found"))?;

        if let Some(ref storage) = storage {
            let result = storage.write(&track.id, &text, kind);
            if result.is_err() {
                warn!("failed to write .lrc file");
            }
        }

        if let Some(ref cache) = cache {
            let result = cache.write(&track.id, &text, kind);
            if result.is_err() {
                warn!("failed to write to cache");
            }
        }

        Ok(make_response(text))
    }
}

fn make_response(text: String) -> GetLyricsResponse {
    GetLyricsResponse {
        lyrics: vec![LyricsText {
            lang: "xxx".into(),
            text,
        }],
    }
}
