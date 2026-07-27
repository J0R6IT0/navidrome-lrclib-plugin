use crate::cache::{CacheLookup, LyricsCache};
use crate::config::PluginConfig;
use crate::fetch::{self, Outcome};
use crate::types::Lyrics;
use crate::writing;
use extism_pdk::{debug, info, warn};
use nd_pdk::lyrics::{Error as LyricsError, GetLyricsResponse, LyricsText, TrackInfo};

pub fn get_lyrics(track: TrackInfo) -> Result<GetLyricsResponse, LyricsError> {
    let cfg = PluginConfig::load()?;
    let cache = build_cache(&cfg);
    let desc = track_label(&track);

    if let Some(response) = lookup_cache(&cache, &track, &cfg, &desc) {
        return Ok(response);
    }

    match fetch::run(&track, &cfg, &cache) {
        Outcome::Found(lyrics) => {
            write_if_enabled(&track, &lyrics, &cfg);
            save(&cache, &track.id, &lyrics, &cfg);
            Ok(respond(lyrics.text(&cfg).into_owned()))
        }
        Outcome::NotFound => {
            info!("no lyrics found for '{desc}' from any provider");
            Err(LyricsError::new("no lyrics found from any provider"))
        }
        Outcome::ProviderError => {
            warn!("all providers errored while fetching lyrics for '{desc}'");
            Err(LyricsError::new("no lyrics found from any provider"))
        }
    }
}

fn build_cache(cfg: &PluginConfig) -> Option<LyricsCache> {
    cfg.enable_cache
        .then(|| LyricsCache::new(cfg.negative_cache_ttl_hours.saturating_mul(3600)))
}

fn lookup_cache(
    cache: &Option<LyricsCache>,
    track: &TrackInfo,
    cfg: &PluginConfig,
    desc: &str,
) -> Option<GetLyricsResponse> {
    let Some(cache) = cache else {
        debug!("cache disabled, querying providers for '{desc}'");
        return None;
    };

    match cache.lookup(&track.id, cfg) {
        CacheLookup::Found(lyrics) => {
            info!("cache hit ({}) for '{desc}'", lyrics.kind().slug());
            write_if_enabled(track, &lyrics, cfg);
            Some(respond(lyrics.text(cfg).into_owned()))
        }
        CacheLookup::Miss => {
            info!("cache miss for '{desc}', querying providers");
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

fn save(cache: &Option<LyricsCache>, track_id: &str, lyrics: &Lyrics, cfg: &PluginConfig) {
    if let Some(cache) = cache {
        match cache.write(track_id, lyrics, cfg) {
            Ok(()) => info!(
                "cached {} lyrics for track '{track_id}' (ttl {}h)",
                lyrics.kind().slug(),
                cfg.cache_ttl_hours_for(lyrics.kind())
            ),
            Err(err) => warn!("failed to persist lyrics to cache: {err}"),
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

fn respond(text: String) -> GetLyricsResponse {
    GetLyricsResponse {
        lyrics: vec![LyricsText {
            lang: "xxx".into(),
            text,
        }],
    }
}
