use crate::cache::LyricsCache;
use config::PluginConfig;
use extism_pdk::warn;
use lrclib::fetch_lyrics_text;
use nd_pdk::lyrics::{
    Error as LyricsError, GetLyricsRequest, GetLyricsResponse, Lyrics, LyricsText,
};

mod cache;
mod config;
mod lrc;
mod lrclib;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LyricsKind {
    Synchronized,
    Plain,
}

#[derive(Default)]
struct Plugin;

nd_pdk::register_lyrics!(Plugin);

impl Lyrics for Plugin {
    fn get_lyrics(&self, req: GetLyricsRequest) -> Result<GetLyricsResponse, LyricsError> {
        let track = req.track;
        let cfg = PluginConfig::load()?;

        let cache = cfg.enable_cache.then(|| LyricsCache::new(cfg.cache_ttl));

        if let Some(ref cache) = cache {
            if let Some(cached) = cache.read(&track.id, cfg.lyrics_mode) {
                return Ok(make_response(cached));
            }
        }

        let (text, kind): (String, LyricsKind) = fetch_lyrics_text(&track, cfg.lyrics_mode)?
            .ok_or_else(|| LyricsError::new("no lyrics found"))?;

        if cfg.write_lyrics {
            let result = lrc::write(&track, &text, cfg.update_lyrics);
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
