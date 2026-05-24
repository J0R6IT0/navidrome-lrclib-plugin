use crate::providers::register_providers;
use crate::registry::ProviderRegistry;
use crate::sanitize::{sanitize_lrc, strip_section_labels};
use crate::types::LyricsType;
use crate::{cache::LyricsCache, providers::LyricsProvider};
use config::PluginConfig;
use extism_pdk::warn;
use nd_pdk::lyrics::{
    Error as LyricsError, GetLyricsRequest, GetLyricsResponse, Lyrics, LyricsText,
};

mod cache;
mod config;
mod providers;
mod registry;
mod sanitize;
mod types;
mod writing;

#[derive(Default)]
struct Plugin;

nd_pdk::register_lyrics!(Plugin);

impl Lyrics for Plugin {
    fn get_lyrics(&self, req: GetLyricsRequest) -> Result<GetLyricsResponse, LyricsError> {
        let track = req.track;
        let cfg = PluginConfig::load()?;
        let cache = cfg
            .enable_cache
            .then(|| LyricsCache::new(cfg.cache_ttl_hours * 3600));

        if let Some(cached) = cache.as_ref().and_then(|c| c.read(&track.id, &cfg)) {
            write_lyrics_if_enabled(&track, &cached.text, cached.kind, &cfg);
            return Ok(make_response(cached.text));
        }

        let mut registry = ProviderRegistry::new();
        register_providers(&mut registry);

        for entry in &cfg.providers {
            let Some(provider) = registry.create(entry) else {
                warn!("unknown provider '{}', skipping", entry);
                continue;
            };

            let label = entry.to_string();
            let Some((text, kind)) = fetch_from_provider(provider.as_ref(), &track, &cfg, &label)
            else {
                continue;
            };

            let mut sanitized = if kind == LyricsType::Synced {
                sanitize_lrc(&text)
            } else {
                text
            };

            if cfg.strip_section_labels {
                sanitized = strip_section_labels(&sanitized);
            }

            write_lyrics_if_enabled(&track, &sanitized, kind, &cfg);
            save_to_cache(&cache, &track.id, &sanitized, kind);

            return Ok(make_response(sanitized));
        }

        Err(LyricsError::new("no lyrics found from any provider"))
    }
}

fn fetch_from_provider(
    provider: &dyn LyricsProvider,
    track: &nd_pdk::lyrics::TrackInfo,
    cfg: &PluginConfig,
    provider_id: &str,
) -> Option<(String, LyricsType)> {
    match provider.fetch_lyrics(track, cfg) {
        Ok(Some(result)) => Some(result),
        Ok(None) => None,
        Err(e) => {
            warn!("provider '{}' failed: {}", provider_id, e);
            None
        }
    }
}

fn write_lyrics_if_enabled(
    track: &nd_pdk::lyrics::TrackInfo,
    text: &str,
    kind: LyricsType,
    cfg: &PluginConfig,
) {
    if !cfg.write_lyrics {
        return;
    }

    if writing::write(track, text, kind, cfg).is_err() {
        warn!("failed to write lyrics file to disk");
    }
}

fn save_to_cache(cache: &Option<LyricsCache>, track_id: &str, text: &str, kind: LyricsType) {
    let Some(cache) = cache else { return };

    if cache.write(track_id, text, kind).is_err() {
        warn!("failed to persist lyrics to cache");
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
