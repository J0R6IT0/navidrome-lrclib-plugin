use crate::{
    config::{PluginConfig, ProviderParams},
    ext::TrackInfoExt,
    providers::{LyricsProvider, USER_AGENT},
    types::{Lyrics, LyricsKind},
};
use nd_pdk::{
    host::http::{self, HTTPRequest, HTTPResponse},
    lyrics::{Error, TrackInfo},
};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Deserialize;
use std::collections::HashMap;

const DEFAULT_BASE_URL: &str = "https://api.lyrics.ovh";

#[derive(Debug, Deserialize)]
struct ApiResponse {
    lyrics: String,
}

pub struct LyricsOvh {
    base_url: String,
}

impl LyricsOvh {
    pub fn create(params: &ProviderParams) -> Box<dyn LyricsProvider> {
        Box::new(Self {
            base_url: params
                .get("baseUrl")
                .unwrap_or(DEFAULT_BASE_URL)
                .to_string(),
        })
    }
}

impl LyricsProvider for LyricsOvh {
    fn supported_kinds(&self) -> &'static [LyricsKind] {
        &[LyricsKind::Plain]
    }

    fn log_params(&self) -> Vec<(&'static str, String)> {
        vec![("baseUrl", self.base_url.clone())]
    }

    fn fetch_lyrics(
        &self,
        track: &TrackInfo,
        _cfg: &PluginConfig,
    ) -> Result<Option<Lyrics>, Error> {
        let first_artist = track
            .first_artist()
            .ok_or_else(|| Error::new("missing artist"))?;

        let url = build_search_url(&self.base_url, first_artist, &track.title);
        let response = send_request(&url)?;

        if response.status_code == 404 {
            return Ok(None);
        }

        if response.status_code != 200 {
            return Err(Error::new(format!(
                "lyrics.ovh returned unexpected status {}",
                response.status_code
            )));
        }

        let body: ApiResponse = serde_json::from_slice(&response.body)
            .map_err(|e| Error::new(format!("failed to parse lyrics.ovh response: {e}")))?;

        if body.lyrics.trim().is_empty() {
            return Ok(None);
        }

        Ok(Some(Lyrics::Plain(body.lyrics)))
    }
}

fn build_search_url(base_url: &str, artist: &str, title: &str) -> String {
    let encoded_artist = utf8_percent_encode(artist, NON_ALPHANUMERIC).to_string();
    let encoded_title = utf8_percent_encode(title, NON_ALPHANUMERIC).to_string();
    format!("{base_url}/v1/{encoded_artist}/{encoded_title}")
}

fn send_request(url: &str) -> Result<HTTPResponse, Error> {
    let mut headers = HashMap::new();
    headers.insert("User-Agent".into(), USER_AGENT.into());

    http::send(HTTPRequest {
        url: url.into(),
        method: "GET".into(),
        headers,
        no_follow_redirects: false,
        body: Vec::new(),
        timeout_ms: 15_000,
    })
    .map_err(|e| Error::new(format!("HTTP request to lyrics.ovh failed: {e}")))?
    .ok_or_else(|| Error::new("received empty HTTP response from lyrics.ovh"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_search_url_simple() {
        let url = build_search_url(DEFAULT_BASE_URL, "The Beatles", "Hey Jude");
        assert_eq!(url, "https://api.lyrics.ovh/v1/The%20Beatles/Hey%20Jude");
    }

    #[test]
    fn test_build_search_url_special_chars() {
        let url = build_search_url(DEFAULT_BASE_URL, "AC/DC", "Back in Black");
        assert_eq!(url, "https://api.lyrics.ovh/v1/AC%2FDC/Back%20in%20Black");
    }

    #[test]
    fn test_build_search_url_unicode() {
        let url = build_search_url(DEFAULT_BASE_URL, "Björk", "Hyperballad");
        assert_eq!(url, "https://api.lyrics.ovh/v1/Bj%C3%B6rk/Hyperballad");
    }

    #[test]
    fn test_build_search_url_custom_base() {
        let url = build_search_url("http://localhost:8080", "Artist", "Title");
        assert_eq!(url, "http://localhost:8080/v1/Artist/Title");
    }

    #[test]
    fn test_build_search_url_empty_artist() {
        let url = build_search_url(DEFAULT_BASE_URL, "", "Title");
        assert_eq!(url, "https://api.lyrics.ovh/v1//Title");
    }
}
