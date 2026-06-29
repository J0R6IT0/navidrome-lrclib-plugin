use crate::{
    config::{PluginConfig, ProviderParams},
    providers::{BROWSER_USER_AGENT, LyricsProvider},
    types::{Lyrics, LyricsKind},
};
use nd_pdk::{
    host::http::{self, HTTPRequest, HTTPResponse},
    lyrics::{Error, TrackInfo},
};
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;

const BASE_URL: &str = "https://amp-api.music.apple.com/v1";
const STOREFRONT_URL: &str = "https://api.music.apple.com/v1/me/storefront";
const DEFAULT_STOREFRONT: &str = "us";

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
}

impl LyricsProvider for AppleMusic {
    fn supported_kinds(&self) -> &'static [LyricsKind] {
        &[LyricsKind::Ttml]
    }

    fn fetch_lyrics(
        &self,
        track: &TrackInfo,
        _cfg: &PluginConfig,
    ) -> Result<Option<Lyrics>, Error> {
        let token = self.media_user_token.as_deref().ok_or_else(|| {
            Error::new(
                "applemusic: a media-user-token is required, configure it in the provider settings",
            )
        })?;

        let first_artist = track
            .artists
            .first()
            .ok_or_else(|| Error::new("missing artist"))?
            .name
            .as_str();

        let dev_token = fetch_dev_token()?;
        let storefront = match &self.storefront {
            Some(s) => s.clone(),
            None => fetch_storefront(&dev_token, token),
        };

        let query = format!("{} {first_artist}", track.title);
        let target_ms = (track.duration * 1000.0).round() as u64;

        let song = match search_song(&dev_token, token, &storefront, &query, target_ms)? {
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
            &dev_token,
            token,
            &storefront,
            &song.id,
            translation_language,
            script,
        )
    }
}

fn fetch_dev_token() -> Result<String, Error> {
    let home = send_request("https://music.apple.com/", &HashMap::new())?;
    let html = String::from_utf8_lossy(&home.body);

    let script_re =
        Regex::new(r#"<script type="module" crossorigin src="(/assets/index[^"]+\.js)""#)
            .map_err(|e| Error::new(format!("applemusic: invalid script regex: {e}")))?;

    let script_path = script_re
        .captures(&html)
        .and_then(|c| c.get(1))
        .ok_or_else(|| Error::new("applemusic: could not find index script tag"))?
        .as_str();

    let script_url = format!("https://music.apple.com{script_path}");
    let script = send_request(&script_url, &HashMap::new())?;
    let js = String::from_utf8_lossy(&script.body);

    let var_re = Regex::new(r#"\.headers\.Authorization\s*=\s*`Bearer \$\{([A-Za-z0-9_$]+)\}`"#)
        .map_err(|e| Error::new(format!("applemusic: invalid token var regex: {e}")))?;

    let var_name = var_re
        .captures(&js)
        .and_then(|c| c.get(1))
        .ok_or_else(|| Error::new("applemusic: could not find auth token variable"))?
        .as_str();

    let value_re = Regex::new(&format!(
        r#"{}\s*=\s*"(eyJ[A-Za-z0-9._-]+)""#,
        regex::escape(var_name)
    ))
    .map_err(|e| Error::new(format!("applemusic: invalid token value regex: {e}")))?;

    let token = value_re
        .captures(&js)
        .and_then(|c| c.get(1))
        .ok_or_else(|| Error::new("applemusic: could not find auth token value"))?
        .as_str();

    Ok(token.to_string())
}

fn fetch_storefront(dev_token: &str, media_user_token: &str) -> String {
    let headers = api_headers(dev_token, media_user_token);
    let response = match send_request(STOREFRONT_URL, &headers) {
        Ok(r) if r.status_code == 200 => r,
        _ => return DEFAULT_STOREFRONT.to_string(),
    };

    serde_json::from_slice::<StorefrontResponse>(&response.body)
        .ok()
        .and_then(|r| r.data.into_iter().next())
        .map(|e| e.id)
        .unwrap_or_else(|| DEFAULT_STOREFRONT.to_string())
}

fn search_song(
    dev_token: &str,
    media_user_token: &str,
    storefront: &str,
    query: &str,
    target_ms: u64,
) -> Result<Option<Song>, Error> {
    let qs = serde_urlencoded::to_string([("types", "songs"), ("term", query)])
        .map_err(|e| Error::new(format!("applemusic: failed to encode search query: {e}")))?;

    let url = format!("{BASE_URL}/catalog/{storefront}/search?{qs}");
    let headers = api_headers(dev_token, media_user_token);
    let response = send_request(&url, &headers)?;

    if response.status_code != 200 {
        return Err(Error::new(format!(
            "applemusic: search returned status {}",
            response.status_code
        )));
    }

    let parsed: SearchResponse = serde_json::from_slice(&response.body)
        .map_err(|e| Error::new(format!("applemusic: failed to parse search response: {e}")))?;

    let songs = parsed
        .results
        .and_then(|r| r.songs)
        .map(|s| s.data)
        .unwrap_or_default();

    let mut candidates = songs
        .into_iter()
        .filter(|s| s.attributes.has_lyrics != Some(false));

    let first = match candidates.next() {
        Some(s) => s,
        None => return Ok(None),
    };

    let first_diff = duration_diff(&first, target_ms);
    let (best, _) = candidates.fold((first, first_diff), |(best, best_diff), song| {
        let diff = duration_diff(&song, target_ms);
        if diff < best_diff {
            (song, diff)
        } else {
            (best, best_diff)
        }
    });

    Ok(Some(best))
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
) -> Result<Option<Lyrics>, Error> {
    let url = build_lyrics_url(storefront, song_id, translation_language, script)?;
    let headers = api_headers(dev_token, media_user_token);
    let response = send_request(&url, &headers)?;

    if response.status_code == 404 {
        return Ok(None);
    }

    if response.status_code != 200 {
        return Err(Error::new(format!(
            "applemusic: lyrics endpoint returned status {}",
            response.status_code
        )));
    }

    let parsed: LyricsResponse = serde_json::from_slice(&response.body)
        .map_err(|e| Error::new(format!("applemusic: failed to parse lyrics response: {e}")))?;

    let ttml = parsed
        .data
        .into_iter()
        .next()
        .and_then(|e| e.attributes.ttml_localizations.or(e.attributes.ttml))
        .filter(|t| !t.trim().is_empty());

    Ok(ttml.map(Lyrics::Ttml))
}

fn build_lyrics_url(
    storefront: &str,
    song_id: &str,
    translation_language: Option<&str>,
    script: Option<&str>,
) -> Result<String, Error> {
    let base = format!("{BASE_URL}/catalog/{storefront}/songs/{song_id}/syllable-lyrics");

    if translation_language.is_none() && script.is_none() {
        return Ok(base);
    }

    let mut params: Vec<(&str, &str)> = vec![("extend", "ttmlLocalizations")];
    if let Some(lang) = translation_language {
        params.push(("l[lyrics]", lang));
    }
    if let Some(script) = script {
        params.push(("l[script]", script));
    }

    let qs = serde_urlencoded::to_string(&params)
        .map_err(|e| Error::new(format!("applemusic: failed to encode lyrics query: {e}")))?;

    Ok(format!("{base}?{qs}"))
}

fn api_headers(dev_token: &str, media_user_token: &str) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    headers.insert("Authorization".into(), format!("Bearer {dev_token}"));
    headers.insert("User-Agent".into(), BROWSER_USER_AGENT.into());
    headers.insert("Origin".into(), "https://music.apple.com".into());
    headers.insert("Referer".into(), "https://music.apple.com".into());
    headers.insert("media-user-token".into(), media_user_token.into());
    headers
}

fn send_request(url: &str, headers: &HashMap<String, String>) -> Result<HTTPResponse, Error> {
    let mut headers = headers.clone();
    headers
        .entry("User-Agent".into())
        .or_insert_with(|| BROWSER_USER_AGENT.into());

    http::send(HTTPRequest {
        url: url.into(),
        method: "GET".into(),
        headers,
        no_follow_redirects: false,
        body: Vec::new(),
        timeout_ms: 15_000,
    })
    .map_err(|e| Error::new(format!("applemusic: HTTP request failed: {e}")))?
    .ok_or_else(|| Error::new("applemusic: received empty HTTP response"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_lyrics_url_plain() {
        assert_eq!(
            build_lyrics_url("us", "123", None, None).unwrap(),
            "https://amp-api.music.apple.com/v1/catalog/us/songs/123/syllable-lyrics"
        );
    }

    #[test]
    fn test_build_lyrics_url_translation() {
        assert_eq!(
            build_lyrics_url("us", "123", Some("en-US"), None).unwrap(),
            "https://amp-api.music.apple.com/v1/catalog/us/songs/123/syllable-lyrics\
             ?extend=ttmlLocalizations&l%5Blyrics%5D=en-US"
        );
    }

    #[test]
    fn test_build_lyrics_url_script() {
        assert_eq!(
            build_lyrics_url("us", "123", None, Some("und-Latn")).unwrap(),
            "https://amp-api.music.apple.com/v1/catalog/us/songs/123/syllable-lyrics\
             ?extend=ttmlLocalizations&l%5Bscript%5D=und-Latn"
        );
    }

    #[test]
    fn test_build_lyrics_url_translation_and_script() {
        assert_eq!(
            build_lyrics_url("gb", "123", Some("es-ES"), Some("und-Latn")).unwrap(),
            "https://amp-api.music.apple.com/v1/catalog/gb/songs/123/syllable-lyrics\
             ?extend=ttmlLocalizations&l%5Blyrics%5D=es-ES&l%5Bscript%5D=und-Latn"
        );
    }
}
