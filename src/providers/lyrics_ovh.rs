use crate::{
    LyricsKind,
    config::LyricsMode,
    providers::{LyricsProvider, USER_AGENT},
};
use nd_pdk::{
    host::http::{self, HTTPRequest, HTTPResponse},
    lyrics::{Error, TrackInfo},
};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Deserialize;
use std::collections::HashMap;

const BASE_URL: &str = "https://api.lyrics.ovh/v1";

#[derive(Debug, Deserialize)]
struct Response {
    lyrics: String,
}

pub struct LyricsOvh;

impl LyricsProvider for LyricsOvh {
    fn id(&self) -> &'static str {
        "lyrics.ovh"
    }

    fn fetch_lyrics(
        &self,
        track: &TrackInfo,
        lyrics_mode: LyricsMode,
    ) -> Result<Option<(String, crate::LyricsKind)>, Error> {
        if matches!(lyrics_mode, LyricsMode::SyncedOnly) {
            return Ok(None);
        }

        let first_artist = track
            .artists
            .first()
            .ok_or_else(|| Error::new("missing artist"))?
            .name
            .as_str();

        let encoded_artist = utf8_percent_encode(first_artist, NON_ALPHANUMERIC).to_string();
        let encoded_title = utf8_percent_encode(&track.title, NON_ALPHANUMERIC).to_string();

        let raw = send_request(&format!(
            "{}/{}/{}",
            BASE_URL, encoded_artist, encoded_title
        ))?;

        if raw.status_code != 200 {
            return Err(Error::new(format!(
                "lyrics.ovh search returned status {}",
                raw.status_code
            )));
        }

        let response: Response =
            serde_json::from_slice(&raw.body).map_err(|e| Error::new(e.to_string()))?;

        Ok(Some((response.lyrics, LyricsKind::Plain)))
    }
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
    .map_err(|e| Error::new(e.to_string()))?
    .ok_or_else(|| Error::new("empty HTTP response"))
}
