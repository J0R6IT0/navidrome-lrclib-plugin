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

    let lyrics = match cache_lookup(cache.as_ref(), &track) {
        Some(lyrics) => lyrics,
        None => {
            let lyrics = fetch_fresh(&track, &cfg, cache.as_ref())?;
            cache_save(cache.as_ref(), &track, &lyrics, &cfg);
            lyrics
        }
    };

    write_if_enabled(&track, &lyrics, &cfg);
    Ok(lyrics.to_response(&cfg))
}

fn fetch_fresh(
    track: &TrackInfo,
    cfg: &PluginConfig,
    cache: Option<&LyricsCache>,
) -> Result<Lyrics, LyricsError> {
    match fetch::run(track, cfg, cache) {
        Outcome::Found(lyrics) => Ok(lyrics),
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

fn cache_lookup(cache: Option<&LyricsCache>, track: &TrackInfo) -> Option<Lyrics> {
    let Some(cache) = cache else {
        debug!("cache disabled");
        return None;
    };

    match cache.lookup(&track.id) {
        CacheLookup::Found(lyrics) => {
            info!(
                "cache hit ({}) for '{}'",
                lyrics.kind().slug(),
                track.label()
            );
            Some(lyrics)
        }
        CacheLookup::Miss => {
            info!("cache miss for '{}'", track.label());
            None
        }
    }
}

fn cache_save(cache: Option<&LyricsCache>, track: &TrackInfo, lyrics: &Lyrics, cfg: &PluginConfig) {
    let Some(cache) = cache else {
        return;
    };

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

fn write_if_enabled(track: &TrackInfo, lyrics: &Lyrics, cfg: &PluginConfig) {
    if cfg.write_lyrics
        && let Err(err) = writing::write(track, lyrics, cfg)
    {
        warn!("failed to write lyrics file to disk: {err}");
    }
}
