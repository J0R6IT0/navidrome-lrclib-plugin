use crate::{
    config::{PluginConfig, ProviderParams},
    ext::TrackInfoExt,
    providers::{LyricsProvider, ProviderResult, error::ProviderError, http::Http},
    types::{Lyrics, LyricsKind},
};
use extism_pdk::info;
use nd_pdk::{host::cache, lyrics::TrackInfo};
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;

const BASE_URL: &str = "https://amp-api.music.apple.com/v1";
const STOREFRONT_URL: &str = "https://api.music.apple.com/v1/me/storefront";
const DEFAULT_STOREFRONT: &str = "us";

const DEV_TOKEN_CACHE_KEY: &str = "applemusic:dev-token";
const STOREFRONT_CACHE_PREFIX: &str = "applemusic:storefront:";

const DEV_TOKEN_TTL: i64 = 7 * 24 * 3600;
const STOREFRONT_TTL: i64 = 30 * 24 * 3600;

enum LookupError {
    Unauthorized,
    Fatal(ProviderError),
}

impl From<ProviderError> for LookupError {
    fn from(e: ProviderError) -> Self {
        LookupError::Fatal(e)
    }
}

fn is_unauthorized(status: i32) -> bool {
    status == 401 || status == 403
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    results: Option<SearchResults>,
}

#[derive(Debug, Deserialize)]
struct SearchResults {
    songs: Option<SongData>,
}

#[derive(Debug, Deserialize)]
struct SongData {
    #[serde(default)]
    data: Vec<Song>,
}

#[derive(Debug, Deserialize)]
struct Song {
    id: String,
    attributes: SongAttributes,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SongAttributes {
    duration_in_millis: Option<u64>,
    has_lyrics: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct StorefrontResponse {
    #[serde(default)]
    data: Vec<StorefrontEntry>,
}

#[derive(Debug, Deserialize)]
struct StorefrontEntry {
    id: String,
}

#[derive(Debug, Deserialize)]
struct LyricsResponse {
    #[serde(default)]
    data: Vec<LyricsEntry>,
}

#[derive(Debug, Deserialize)]
struct LyricsEntry {
    attributes: LyricsAttributes,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LyricsAttributes {
    ttml: Option<String>,
    ttml_localizations: Option<String>,
}

pub struct AppleMusic {
    media_user_token: Option<String>,
    storefront: Option<String>,
    include_translations: bool,
    translation_language: Option<String>,
    romanize: bool,
}

const ROMANIZED_SCRIPT: &str = "und-Latn";

impl LyricsProvider for AppleMusic {
    fn supported_kinds(&self) -> &'static [LyricsKind] {
        &[LyricsKind::Ttml]
    }

    fn fetch_lyrics(
        &self,
        track: &TrackInfo,
        cfg: &PluginConfig,
    ) -> ProviderResult<Option<Lyrics>> {
        let token = self.media_user_token.as_deref().ok_or_else(|| {
            ProviderError::other(
                "a media-user-token is required, configure it in the provider settings",
            )
        })?;

        let query = format!("{} {}", track.title, track.first_artist().unwrap());
        let target_ms = track.duration().as_millis() as u64;
        let tolerance_ms = cfg.duration_tolerance.as_millis() as u64;

        let dev_token = get_dev_token(false)?;
        let result = match self.lookup(&dev_token, token, &query, target_ms, tolerance_ms) {
            Err(LookupError::Unauthorized) => {
                info!("applemusic: cached developer token rejected, refreshing");
                let dev_token = get_dev_token(true)?;
                self.lookup(&dev_token, token, &query, target_ms, tolerance_ms)
            }
            result => result,
        };

        result.map_err(|e| match e {
            LookupError::Fatal(e) => e,
            LookupError::Unauthorized => ProviderError::other(
                "authorization failed even after refreshing the developer token",
            ),
        })
    }
}

impl AppleMusic {
    pub fn create(params: &ProviderParams) -> Box<dyn LyricsProvider> {
        Box::new(Self {
            media_user_token: params.get("mediaUserToken").map(str::to_string),
            storefront: params.get("storefront").map(str::to_string),
            include_translations: params.get("includeTranslations") == Some("true"),
            translation_language: params.get("translationLanguage").map(str::to_string),
            romanize: params.get("includeRomanization") == Some("true"),
        })
    }

    fn lookup(
        &self,
        dev_token: &str,
        media_user_token: &str,
        query: &str,
        target_ms: u64,
        tolerance_ms: u64,
    ) -> Result<Option<Lyrics>, LookupError> {
        let storefront = self.resolve_storefront(dev_token, media_user_token)?;

        let song = match search_song(
            dev_token,
            media_user_token,
            &storefront,
            query,
            target_ms,
            tolerance_ms,
        )? {
            Some(s) => s,
            None => return Ok(None),
        };

        if song.attributes.has_lyrics == Some(false) {
            return Ok(None);
        }

        let translation_language = self
            .include_translations
            .then_some(self.translation_language.as_deref())
            .flatten();
        let script = self.romanize.then_some(ROMANIZED_SCRIPT);

        fetch_ttml(
            dev_token,
            media_user_token,
            &storefront,
            &song.id,
            translation_language,
            script,
        )
    }

    fn resolve_storefront(
        &self,
        dev_token: &str,
        media_user_token: &str,
    ) -> Result<String, LookupError> {
        if let Some(s) = &self.storefront {
            return Ok(s.clone());
        }

        let key = storefront_cache_key(media_user_token);
        if let Ok(Some(cached)) = cache::get_string(&key) {
            return Ok(cached);
        }

        match fetch_storefront(dev_token, media_user_token)? {
            Some(storefront) => {
                info!("applemusic: resolved and cached storefront '{storefront}'");
                let _ = cache::set_string(&key, &storefront, STOREFRONT_TTL);
                Ok(storefront)
            }
            None => Ok(DEFAULT_STOREFRONT.to_string()),
        }
    }
}

fn get_dev_token(force_refresh: bool) -> ProviderResult<String> {
    if !force_refresh && let Ok(Some(token)) = cache::get_string(DEV_TOKEN_CACHE_KEY) {
        info!("applemusic: reusing cached developer token");
        return Ok(token);
    }

    info!("applemusic: scraping fresh developer token");
    let token = fetch_dev_token()?;
    let _ = cache::set_string(DEV_TOKEN_CACHE_KEY, &token, DEV_TOKEN_TTL);
    Ok(token)
}

fn storefront_cache_key(media_user_token: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in media_user_token.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{STOREFRONT_CACHE_PREFIX}{hash:016x}")
}

fn fetch_dev_token() -> ProviderResult<String> {
    let home = Http::get("https://music.apple.com/").browser().send()?;
    let html = home.text();

    let script_re =
        Regex::new(r#"<script type="module" crossorigin src="(/assets/index[^"]+\.js)""#)
            .map_err(|e| ProviderError::other(format!("invalid script regex: {e}")))?;

    let script_path = script_re
        .captures(&html)
        .and_then(|c| c.get(1))
        .ok_or_else(|| ProviderError::other("could not find the index script tag"))?
        .as_str();

    let script = Http::get(format!("https://music.apple.com{script_path}"))
        .browser()
        .send()?;
    let js = script.text();

    let var_re = Regex::new(r#"\.headers\.Authorization\s*=\s*`Bearer \$\{([A-Za-z0-9_$]+)\}`"#)
        .map_err(|e| ProviderError::other(format!("invalid token var regex: {e}")))?;

    let var_name = var_re
        .captures(&js)
        .and_then(|c| c.get(1))
        .ok_or_else(|| ProviderError::other("could not find the auth token variable"))?
        .as_str();

    let value_re = Regex::new(&format!(
        r#"{}\s*=\s*"(eyJ[A-Za-z0-9._-]+)""#,
        regex::escape(var_name)
    ))
    .map_err(|e| ProviderError::other(format!("invalid token value regex: {e}")))?;

    let token = value_re
        .captures(&js)
        .and_then(|c| c.get(1))
        .ok_or_else(|| ProviderError::other("could not find the auth token value"))?
        .as_str();

    Ok(token.to_string())
}

fn fetch_storefront(
    dev_token: &str,
    media_user_token: &str,
) -> Result<Option<String>, LookupError> {
    let response = match Http::get(STOREFRONT_URL)
        .browser()
        .headers(api_headers(dev_token, media_user_token))
        .send()
    {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };

    if is_unauthorized(response.status) {
        return Err(LookupError::Unauthorized);
    }

    if response.status != 200 {
        return Ok(None);
    }

    Ok(response
        .json::<StorefrontResponse>("storefront")
        .ok()
        .and_then(|r| r.data.into_iter().next())
        .map(|e| e.id))
}

fn search_song(
    dev_token: &str,
    media_user_token: &str,
    storefront: &str,
    query: &str,
    target_ms: u64,
    tolerance_ms: u64,
) -> Result<Option<Song>, LookupError> {
    let response = Http::get(format!("{BASE_URL}/catalog/{storefront}/search"))
        .param("types", "songs")
        .param("term", query)
        .browser()
        .headers(api_headers(dev_token, media_user_token))
        .send()?;

    if is_unauthorized(response.status) {
        return Err(LookupError::Unauthorized);
    }

    if response.status != 200 {
        return Err(LookupError::Fatal(response.unexpected_status("the search")));
    }

    let parsed: SearchResponse = response.json("search")?;

    let best = parsed
        .results
        .and_then(|r| r.songs)
        .map(|s| s.data)
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.attributes.has_lyrics != Some(false))
        .min_by_key(|s| duration_diff(s, target_ms));

    match best {
        Some(song) if duration_diff(&song, target_ms) <= tolerance_ms => Ok(Some(song)),
        Some(_) => {
            info!("applemusic: closest match exceeds duration tolerance, skipping");
            Ok(None)
        }
        None => Ok(None),
    }
}

fn duration_diff(song: &Song, target_ms: u64) -> u64 {
    song.attributes
        .duration_in_millis
        .map(|d| d.abs_diff(target_ms))
        .unwrap_or(u64::MAX)
}

fn fetch_ttml(
    dev_token: &str,
    media_user_token: &str,
    storefront: &str,
    song_id: &str,
    translation_language: Option<&str>,
    script: Option<&str>,
) -> Result<Option<Lyrics>, LookupError> {
    let response = lyrics_request(storefront, song_id, translation_language, script)
        .browser()
        .headers(api_headers(dev_token, media_user_token))
        .send()?;

    if response.status == 404 {
        return Ok(None);
    }

    if is_unauthorized(response.status) {
        return Err(LookupError::Unauthorized);
    }

    if response.status != 200 {
        return Err(LookupError::Fatal(
            response.unexpected_status("the lyrics endpoint"),
        ));
    }

    let parsed: LyricsResponse = response.json("lyrics")?;

    let ttml = parsed
        .data
        .into_iter()
        .next()
        .and_then(|e| e.attributes.ttml_localizations.or(e.attributes.ttml))
        .filter(|t| !t.trim().is_empty());

    Ok(ttml.map(Lyrics::Ttml))
}

fn lyrics_request(
    storefront: &str,
    song_id: &str,
    translation_language: Option<&str>,
    script: Option<&str>,
) -> Http {
    let mut request = Http::get(format!(
        "{BASE_URL}/catalog/{storefront}/songs/{song_id}/syllable-lyrics"
    ));

    if translation_language.is_none() && script.is_none() {
        return request;
    }

    request = request.param("extend", "ttmlLocalizations");
    if let Some(lang) = translation_language {
        request = request.param("l[lyrics]", lang);
    }
    if let Some(script) = script {
        request = request.param("l[script]", script);
    }

    request
}

fn api_headers(dev_token: &str, media_user_token: &str) -> HashMap<String, String> {
    HashMap::from([
        ("Authorization".into(), format!("Bearer {dev_token}")),
        ("Origin".into(), "https://music.apple.com".into()),
        ("Referer".into(), "https://music.apple.com".into()),
        ("media-user-token".into(), media_user_token.into()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn check_lyrics_url(
        storefront: &str,
        translation_language: Option<&str>,
        script: Option<&str>,
        expected: &str,
    ) {
        let url = lyrics_request(storefront, "123", translation_language, script)
            .url()
            .unwrap();

        assert_eq!(
            url, expected,
            "{storefront} with {translation_language:?} and {script:?}"
        );
    }

    #[test]
    fn a_lyrics_request_without_extras_is_a_bare_url() {
        check_lyrics_url(
            "us",
            None,
            None,
            "https://amp-api.music.apple.com/v1/catalog/us/songs/123/syllable-lyrics",
        );
    }

    #[test]
    fn a_translation_asks_for_the_localizations_extension() {
        check_lyrics_url(
            "us",
            Some("en-US"),
            None,
            "https://amp-api.music.apple.com/v1/catalog/us/songs/123/syllable-lyrics\
             ?extend=ttmlLocalizations&l%5Blyrics%5D=en-US",
        );
    }

    #[test]
    fn a_romanization_asks_for_the_script_instead() {
        check_lyrics_url(
            "us",
            None,
            Some("und-Latn"),
            "https://amp-api.music.apple.com/v1/catalog/us/songs/123/syllable-lyrics\
             ?extend=ttmlLocalizations&l%5Bscript%5D=und-Latn",
        );
    }

    #[test]
    fn a_translation_and_a_romanization_travel_together() {
        check_lyrics_url(
            "gb",
            Some("es-ES"),
            Some("und-Latn"),
            "https://amp-api.music.apple.com/v1/catalog/gb/songs/123/syllable-lyrics\
             ?extend=ttmlLocalizations&l%5Blyrics%5D=es-ES&l%5Bscript%5D=und-Latn",
        );
    }

    #[test]
    fn test_storefront_cache_key_is_stable_and_prefixed() {
        let key = storefront_cache_key("token-abc");
        assert!(key.starts_with(STOREFRONT_CACHE_PREFIX));
        assert_eq!(key, storefront_cache_key("token-abc"));
    }

    #[test]
    fn test_storefront_cache_key_differs_per_token() {
        assert_ne!(
            storefront_cache_key("token-abc"),
            storefront_cache_key("token-xyz")
        );
    }

    fn song_with_duration(duration_in_millis: Option<u64>) -> Song {
        Song {
            id: "1".into(),
            attributes: SongAttributes {
                duration_in_millis,
                has_lyrics: Some(true),
            },
        }
    }

    #[test]
    fn test_duration_diff_within_and_beyond_tolerance() {
        let target = 200_000;
        let tolerance_ms = 3_000;

        let close = song_with_duration(Some(201_500));
        assert!(duration_diff(&close, target) <= tolerance_ms);

        let far = song_with_duration(Some(260_000));
        assert!(duration_diff(&far, target) > tolerance_ms);

        let unknown = song_with_duration(None);
        assert!(duration_diff(&unknown, target) > tolerance_ms);
    }
}
