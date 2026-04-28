use crate::{cache::LyricsCache, providers::register_providers, registry::ProviderRegistry};
use config::PluginConfig;
use extism_pdk::warn;
use nd_pdk::lyrics::{
    Error as LyricsError, GetLyricsRequest, GetLyricsResponse, Lyrics, LyricsText,
};

mod cache;
mod config;
mod lrc;
mod providers;
mod registry;

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
        let mut registry = ProviderRegistry::new();
        register_providers(&mut registry);

        let track = req.track;
        let cfg = PluginConfig::load()?;

        let cache = cfg.enable_cache.then(|| LyricsCache::new(cfg.cache_ttl));

        if let Some(ref cache) = cache {
            if let Some(cached) = cache.read(&track.id, cfg.lyrics_mode) {
                return Ok(make_response(cached));
            }
        }

        for provider_id in &cfg.providers {
            let provider = match registry.get(provider_id) {
                Some(p) => p,
                None => continue,
            };

            let result = match provider.fetch_lyrics(&track, cfg.lyrics_mode) {
                Ok(r) => r,
                Err(e) => {
                    warn!("provider {} failed: {}", provider_id, e);
                    continue;
                }
            };

            let (text, kind) = match result {
                Some(r) => r,
                None => continue,
            };

            if cfg.write_lyrics {
                if lrc::write(&track, &text, cfg.update_lyrics).is_err() {
                    warn!("failed to write .lrc file");
                }
            }

            if let Some(cache) = &cache {
                if cache.write(&track.id, &text, kind).is_err() {
                    warn!("failed to write to cache");
                }
            }

            return Ok(make_response(text));
        }

        Err(LyricsError::new("no lyrics found"))
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
