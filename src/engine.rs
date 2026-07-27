use crate::cache::{CacheLookup, LyricsCache};
use crate::config::PluginConfig;
use crate::ext::TrackInfoExt;
use crate::fetch::{self, Outcome};
use crate::types::Lyrics;
use crate::writing;
use extism_pdk::{debug, info, warn};
use nd_pdk::lyrics::{Error as LyricsError, GetLyricsResponse, TrackInfo};
use std::rc::Rc;

pub fn get_lyrics(track: TrackInfo) -> Result<GetLyricsResponse, LyricsError> {
    let cfg = Rc::new(PluginConfig::load()?);
    let cache = cfg.enable_cache.then(|| LyricsCache::new(cfg.clone()));

    if let Some(cache) = &cache {
        if let Some(response) = lookup_cache(cache, &track, &cfg) {
            return Ok(response);
        }
    } else {
        debug!("cache disabled, querying providers for '{}'", track.label());
    }

    match fetch::run(&track, &cfg, &cache) {
        Outcome::Found(lyrics) => {
            write_if_enabled(&track, &lyrics, &cfg);
            save(&cache, &track, &lyrics, &cfg);
            Ok(lyrics.to_response(&cfg))
        }
        Outcome::NotFound => Err(LyricsError::new(format!(
            "no lyrics found for '{}' from any provider",
            track.label()
        ))),
        Outcome::ProviderError => Err(LyricsError::new(format!(
            "all providers errored while fetching lyrics for '{}'",
            track.label()
        ))),
    }
}

fn lookup_cache(
    cache: &LyricsCache,
    track: &TrackInfo,
    cfg: &PluginConfig,
) -> Option<GetLyricsResponse> {
    match cache.lookup(&track.id) {
        CacheLookup::Found(lyrics) => {
            info!(
                "cache hit ({}) for '{}'",
                lyrics.kind().slug(),
                track.label()
            );
            write_if_enabled(track, &lyrics, cfg);
            Some(lyrics.to_response(cfg))
        }
        CacheLookup::Miss => {
            info!("cache miss for '{}', querying providers", track.label());
            None
        }
    }
}

fn write_if_enabled(track: &TrackInfo, lyrics: &Lyrics, cfg: &PluginConfig) {
    if cfg.write_lyrics
        && let Err(err) = writing::write(track, lyrics, cfg)
    {
        warn!("failed to write lyrics file to disk: {err}");
    }
}

fn save(cache: &Option<LyricsCache>, track: &TrackInfo, lyrics: &Lyrics, cfg: &PluginConfig) {
    if let Some(cache) = cache {
        match cache.write(&track.id, lyrics) {
            Ok(()) => info!(
                "cached {} lyrics for track '{}' (ttl {}h)",
                lyrics.kind().slug(),
                track.label(),
                cfg.cache_ttl_hours_for(lyrics.kind())
            ),
            Err(err) => warn!("failed to persist lyrics to cache: {err}"),
        }
    }
}
