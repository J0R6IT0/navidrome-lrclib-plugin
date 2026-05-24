use crate::cache::LyricsCache;
use crate::providers::register_providers;
use crate::registry::ProviderRegistry;
use crate::sanitize::{sanitize_lrc, strip_section_labels};
use crate::types::LyricsType;
use config::PluginConfig;
use extism_pdk::warn;
use nd_pdk::lyrics::{
    Error as LyricsError, GetLyricsRequest, GetLyricsResponse, Lyrics, LyricsText, TrackInfo,
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

        let cache = cfg.enable_cache.then(|| {
            LyricsCache::new(
                cfg.cache_ttl_hours * 3600,
                cfg.negative_cache_ttl_hours * 3600,
            )
        });

        if let Some(cached) = cache.as_ref().and_then(|c| c.read(&track.id, &cfg)) {
            write_lyrics_if_enabled(&track, &cached.text, cached.kind, &cfg);
            return Ok(make_response(cached.text));
        }

        if cache.as_ref().is_some_and(|c| c.is_negative(&track.id)) {
            return Err(LyricsError::new("no lyrics found (cached)"));
        }

        match fetch_from_providers(&track, &cfg) {
            FetchOutcome::Found { text, kind } => {
                write_lyrics_if_enabled(&track, &text, kind, &cfg);
                save_to_cache(&cache, &track.id, &text, kind);
                Ok(make_response(text))
            }
            FetchOutcome::NotFound => {
                if cfg.negative_cache {
                    save_negative_to_cache(&cache, &track.id);
                }
                Err(LyricsError::new("no lyrics found from any provider"))
            }
            FetchOutcome::ProviderError => {
                Err(LyricsError::new("no lyrics found from any provider"))
            }
        }
    }
}

enum FetchOutcome {
    Found {
        text: String,
        kind: LyricsType,
    },
    NotFound,
    /// At least one provider returned an error.
    ProviderError,
}

fn fetch_from_providers(track: &TrackInfo, cfg: &PluginConfig) -> FetchOutcome {
    let mut registry = ProviderRegistry::new();
    register_providers(&mut registry);

    let mut had_error = false;

    for entry in &cfg.providers {
        let Some(provider) = registry.create(entry) else {
            warn!("unknown provider '{}', skipping", entry);
            continue;
        };

        match provider.fetch_lyrics(track, cfg) {
            Ok(Some((text, kind))) => {
                return FetchOutcome::Found {
                    text: sanitize(text, kind, cfg),
                    kind,
                };
            }
            Ok(None) => {}
            Err(e) => {
                warn!("provider '{}' failed: {}", entry, e);
                had_error = true;
            }
        }
    }

    if had_error {
        FetchOutcome::ProviderError
    } else {
        FetchOutcome::NotFound
    }
}

fn sanitize(text: String, kind: LyricsType, cfg: &PluginConfig) -> String {
    let text = if kind == LyricsType::Synced {
        sanitize_lrc(&text)
    } else {
        text
    };

    if cfg.strip_section_labels {
        strip_section_labels(&text)
    } else {
        text
    }
}

fn write_lyrics_if_enabled(track: &TrackInfo, text: &str, kind: LyricsType, cfg: &PluginConfig) {
    if cfg.write_lyrics && writing::write(track, text, kind, cfg).is_err() {
        warn!("failed to write lyrics file to disk");
    }
}

fn save_to_cache(cache: &Option<LyricsCache>, track_id: &str, text: &str, kind: LyricsType) {
    if let Some(cache) = cache
        && cache.write(track_id, text, kind).is_err()
    {
        warn!("failed to persist lyrics to cache");
    }
}

fn save_negative_to_cache(cache: &Option<LyricsCache>, track_id: &str) {
    if let Some(cache) = cache
        && cache.write_negative(track_id).is_err()
    {
        warn!("failed to persist negative cache entry");
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
