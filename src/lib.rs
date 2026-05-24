use crate::cache::{CacheLookup, LyricsCache};
use crate::providers::register_providers;
use crate::registry::ProviderRegistry;
use crate::types::Lyrics;
use config::PluginConfig;
use extism_pdk::warn;
use nd_pdk::lyrics::{
    Error as LyricsError, GetLyricsRequest, GetLyricsResponse, Lyrics as LyricsPlugin, LyricsText,
    TrackInfo,
};

mod cache;
mod config;
mod format;
mod providers;
mod registry;
mod types;
mod writing;

#[derive(Default)]
struct Plugin;

nd_pdk::register_lyrics!(Plugin);

impl LyricsPlugin for Plugin {
    fn get_lyrics(&self, req: GetLyricsRequest) -> Result<GetLyricsResponse, LyricsError> {
        let track = req.track;
        let cfg = PluginConfig::load()?;

        if track.title.to_ascii_lowercase().contains("instrumental") {
            return Ok(make_response(cfg.instrumental_text));
        }

        let cache = cfg.enable_cache.then(|| {
            LyricsCache::new(
                cfg.cache_ttl_hours * 3600,
                cfg.negative_cache_ttl_hours * 3600,
            )
        });

        if let Some(cache) = &cache {
            match cache.lookup(&track.id, &cfg) {
                CacheLookup::Found(lyrics) => {
                    write_lyrics_if_enabled(&track, &lyrics, &cfg);
                    return Ok(make_response(lyrics.text(&cfg).to_string()));
                }
                CacheLookup::Negative => {
                    return Err(LyricsError::new("no lyrics found (cached)"));
                }
                CacheLookup::Miss => {}
            }
        }

        match fetch_from_providers(&track, &cfg) {
            FetchOutcome::Found(lyrics) => {
                write_lyrics_if_enabled(&track, &lyrics, &cfg);
                save_to_cache(&cache, &track.id, &lyrics, &cfg);
                Ok(make_response(lyrics.text(&cfg).to_string()))
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
    Found(Lyrics),
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
            Ok(Some(mut lyrics)) => {
                lyrics.sanitize(cfg);
                return FetchOutcome::Found(lyrics);
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

fn write_lyrics_if_enabled(track: &TrackInfo, lyrics: &Lyrics, cfg: &PluginConfig) {
    if cfg.write_lyrics
        && let Err(err) = writing::write(track, lyrics, cfg)
    {
        warn!("failed to write lyrics file to disk: {err}");
    }
}

fn save_to_cache(cache: &Option<LyricsCache>, track_id: &str, lyrics: &Lyrics, cfg: &PluginConfig) {
    if let Some(cache) = cache
        && let Err(err) = cache.write(track_id, lyrics, cfg)
    {
        warn!("failed to persist lyrics to cache: {err}");
    }
}

fn save_negative_to_cache(cache: &Option<LyricsCache>, track_id: &str) {
    if let Some(cache) = cache
        && let Err(err) = cache.write_negative(track_id)
    {
        warn!("failed to persist negative cache entry: {err}");
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
