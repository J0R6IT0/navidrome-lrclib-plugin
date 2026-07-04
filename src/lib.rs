use crate::cache::{CacheLookup, LyricsCache};
use crate::providers::register_providers;
use crate::registry::ProviderRegistry;
use crate::types::Lyrics;
use config::PluginConfig;
use extism_pdk::{debug, info, warn};
use nd_pdk::lyrics::{
    Error as LyricsError, GetLyricsRequest, GetLyricsResponse, Lyrics as LyricsPlugin, LyricsText,
    TrackInfo,
};

mod cache;
mod config;
mod format;
mod providers;
mod registry;
mod selection;
mod types;
mod writing;

#[derive(Default)]
struct Plugin;

nd_pdk::register_lyrics!(Plugin);

impl LyricsPlugin for Plugin {
    fn get_lyrics(&self, req: GetLyricsRequest) -> Result<GetLyricsResponse, LyricsError> {
        let track = req.track;
        let cfg = PluginConfig::load()?;

        let cache = cfg.enable_cache.then(|| {
            LyricsCache::new(
                cfg.cache_ttl_hours.saturating_mul(3600),
                cfg.negative_cache_ttl_hours.saturating_mul(3600),
            )
        });

        let track_desc = track_label(&track);

        if let Some(cache) = &cache {
            match cache.lookup(&track.id, &cfg) {
                CacheLookup::Found(lyrics) => {
                    info!("cache hit ({}) for '{}'", lyrics.kind().slug(), track_desc);
                    write_lyrics_if_enabled(&track, &lyrics, &cfg);
                    return Ok(make_response(lyrics.text(&cfg).to_string()));
                }
                CacheLookup::Negative => {
                    info!("cache hit (negative) for '{track_desc}', skipping providers");
                    return Err(LyricsError::new("no lyrics found (cached)"));
                }
                CacheLookup::Miss => {
                    info!("cache miss for '{track_desc}', querying providers");
                }
            }
        } else {
            debug!("cache disabled, querying providers for '{track_desc}'");
        }

        match fetch_from_providers(&track, &cfg) {
            FetchOutcome::Found(lyrics) => {
                write_lyrics_if_enabled(&track, &lyrics, &cfg);
                save_to_cache(&cache, &track.id, &lyrics, &cfg);
                Ok(make_response(lyrics.text(&cfg).to_string()))
            }
            FetchOutcome::NotFound => {
                info!("no lyrics found for '{track_desc}' from any provider");
                if cfg.negative_cache {
                    save_negative_to_cache(&cache, &track.id, &cfg);
                }
                Err(LyricsError::new("no lyrics found from any provider"))
            }
            FetchOutcome::ProviderError => {
                warn!("all providers errored while fetching lyrics for '{track_desc}'");
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

    for entry in selection::order_providers(cfg) {
        let Some(provider) = registry.create(entry) else {
            warn!("unknown provider '{}', skipping", entry.name);
            continue;
        };

        if !provider
            .supported_kinds()
            .iter()
            .any(|&kind| cfg.wants(kind))
        {
            continue;
        }

        let label = provider_label(&entry.name, &provider.log_params());
        info!("trying provider '{}'", label);
        match provider.fetch_lyrics(track, cfg) {
            Ok(Some(mut lyrics)) => {
                lyrics.sanitize(cfg);
                if lyrics.is_empty() {
                    warn!(
                        "provider '{}' returned empty lyrics after sanitization, skipping",
                        label
                    );
                } else {
                    info!(
                        "provider '{}' returned {} lyrics",
                        label,
                        lyrics.kind().slug()
                    );
                    return FetchOutcome::Found(lyrics);
                }
            }
            Ok(None) => {
                info!("provider '{}' returned no lyrics", label);
            }
            Err(e) => {
                warn!("provider '{}' failed: {}", label, e);
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

fn provider_label(name: &str, params: &[(&'static str, String)]) -> String {
    if params.is_empty() {
        return name.to_string();
    }

    let joined = params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(", ");

    format!("{name}({joined})")
}

fn write_lyrics_if_enabled(track: &TrackInfo, lyrics: &Lyrics, cfg: &PluginConfig) {
    if cfg.write_lyrics
        && let Err(err) = writing::write(track, lyrics, cfg)
    {
        warn!("failed to write lyrics file to disk: {err}");
    }
}

fn save_to_cache(cache: &Option<LyricsCache>, track_id: &str, lyrics: &Lyrics, cfg: &PluginConfig) {
    if let Some(cache) = cache {
        match cache.write(track_id, lyrics, cfg) {
            Ok(()) => info!(
                "cached {} lyrics for track '{track_id}' (ttl {}h)",
                lyrics.kind().slug(),
                cfg.cache_ttl_hours
            ),
            Err(err) => warn!("failed to persist lyrics to cache: {err}"),
        }
    }
}

fn save_negative_to_cache(cache: &Option<LyricsCache>, track_id: &str, cfg: &PluginConfig) {
    if let Some(cache) = cache {
        match cache.write_negative(track_id) {
            Ok(()) => info!(
                "cached negative result for track '{track_id}' (ttl {}h)",
                cfg.negative_cache_ttl_hours
            ),
            Err(err) => warn!("failed to persist negative cache entry: {err}"),
        }
    }
}

fn track_label(track: &TrackInfo) -> String {
    match (track.artist.trim(), track.title.trim()) {
        ("", "") => track.id.clone(),
        ("", title) => title.to_string(),
        (artist, "") => artist.to_string(),
        (artist, title) => format!("{artist} - {title}"),
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
