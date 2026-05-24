use crate::{
    config::PluginConfig,
    format::lrc,
    providers::{FIREFOX_USER_AGENT, LyricsProvider},
    types::Lyrics,
};
use nd_pdk::{
    host::http::{self, HTTPRequest, HTTPResponse},
    lyrics::{Error, TrackInfo},
};
use serde::Deserialize;
use std::collections::HashMap;

const SEARCH_URL: &str = "https://music.163.com/api/search/get";
const LYRICS_URL: &str = "https://music.163.com/api/song/lyric";

const DURATION_TOLERANCE_MS: u64 = 3_000;

#[derive(Debug, Deserialize)]
struct SearchResponse {
    result: SearchResult,
}

#[derive(Debug, Deserialize)]
struct SearchResult {
    #[serde(default)]
    songs: Vec<Song>,
}

#[derive(Debug, Deserialize)]
struct Song {
    id: u64,
    #[serde(rename = "duration")]
    duration_ms: u64,
}

#[derive(Debug, Deserialize)]
struct LyricsResponse {
    lrc: Option<LrcContent>,
}

#[derive(Debug, Deserialize)]
struct LrcContent {
    lyric: Option<String>,
}

pub struct NetEase;

impl NetEase {
    pub fn create(_param: Option<&str>) -> Box<dyn LyricsProvider> {
        Box::new(Self)
    }
}

impl LyricsProvider for NetEase {
    fn fetch_lyrics(&self, track: &TrackInfo, cfg: &PluginConfig) -> Result<Option<Lyrics>, Error> {
        if !cfg.wants_synced() {
            return Ok(None);
        }

        let first_artist = track
            .artists
            .first()
            .ok_or_else(|| Error::new("missing artist"))?
            .name
            .as_str();

        let query = format!("{first_artist} {}", track.title);
        let target_ms = (track.duration * 1000.0).round() as u64;

        let song = match search_song(&query, target_ms)? {
            Some(s) => s,
            None => return Ok(None),
        };

        let lrc = match fetch_lrc(song.id)? {
            Some(s) => s,
            None => return Ok(None),
        };

        if lrc::is_instrumental(&lrc) {
            Ok(Some(Lyrics::Instrumental))
        } else {
            Ok(Some(Lyrics::Synced(lrc)))
        }
    }
}

fn search_song(query: &str, target_ms: u64) -> Result<Option<Song>, Error> {
    let qs =
        serde_urlencoded::to_string([("s", query), ("type", "1"), ("limit", "5"), ("offset", "0")])
            .map_err(|e| Error::new(format!("netease: failed to encode search query: {e}")))?;

    let response = send_request(&format!("{SEARCH_URL}?{qs}"))?;

    if response.status_code != 200 {
        return Err(Error::new(format!(
            "netease: search returned status {}",
            response.status_code
        )));
    }

    let parsed: SearchResponse = serde_json::from_slice(&response.body)
        .map_err(|e| Error::new(format!("netease: failed to parse search response: {e}")))?;

    Ok(parsed
        .result
        .songs
        .into_iter()
        .find(|s| s.duration_ms.abs_diff(target_ms) <= DURATION_TOLERANCE_MS))
}

fn fetch_lrc(song_id: u64) -> Result<Option<String>, Error> {
    let qs = serde_urlencoded::to_string([
        ("id", song_id.to_string()),
        ("kv", "-1".to_string()),
        ("lv", "-1".to_string()),
        ("tv", "-1".to_string()),
    ])
    .map_err(|e| Error::new(format!("netease: failed to encode lyrics query: {e}")))?;

    let response = send_request(&format!("{LYRICS_URL}?{qs}"))?;

    if response.status_code != 200 {
        return Err(Error::new(format!(
            "netease: lyrics endpoint returned status {}",
            response.status_code
        )));
    }

    let parsed: LyricsResponse = serde_json::from_slice(&response.body)
        .map_err(|e| Error::new(format!("netease: failed to parse lyrics response: {e}")))?;

    Ok(parsed
        .lrc
        .and_then(|l| l.lyric)
        .filter(|s| !s.trim().is_empty()))
}

fn send_request(url: &str) -> Result<HTTPResponse, Error> {
    let mut headers = HashMap::new();
    headers.insert("User-Agent".into(), FIREFOX_USER_AGENT.into());
    headers.insert("Referer".into(), "https://music.163.com".into());

    http::send(HTTPRequest {
        url: url.into(),
        method: "GET".into(),
        headers,
        no_follow_redirects: false,
        body: Vec::new(),
        timeout_ms: 15_000,
    })
    .map_err(|e| Error::new(format!("netease: HTTP request failed: {e}")))?
    .ok_or_else(|| Error::new("netease: received empty HTTP response"))
}
