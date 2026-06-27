use crate::{
    config::PluginConfig,
    format::lrc,
    providers::{FIREFOX_USER_AGENT, LyricsProvider},
    types::Lyrics,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use nd_pdk::{
    host::http::{self, HTTPRequest, HTTPResponse},
    lyrics::{Error, TrackInfo},
};
use serde::Deserialize;
use std::collections::HashMap;

const SONG_SEARCH_URL: &str = "http://mobilecdn.kugou.com/api/v3/search/song";
const LYRICS_SEARCH_URL: &str = "http://lyrics.kugou.com/search";
const DOWNLOAD_URL: &str = "https://lyrics.kugou.com/download";

#[derive(Debug, Deserialize)]
struct SongSearchResponse {
    data: SongSearchData,
}

#[derive(Debug, Deserialize)]
struct SongSearchData {
    #[serde(default)]
    info: Vec<SongInfo>,
}

#[derive(Debug, Deserialize)]
struct SongInfo {
    hash: String,
    duration: Option<u64>,
    trans_param: Option<TransParam>,
}

#[derive(Debug, Deserialize)]
struct TransParam {
    #[serde(default)]
    language: String,
}

#[derive(Debug, Deserialize)]
struct LyricsSearchResponse {
    status: u32,
    #[serde(default)]
    candidates: Vec<Candidate>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    id: String,
    accesskey: String,
}

#[derive(Debug, Deserialize)]
struct DownloadResponse {
    content: String,
}

pub struct Kugou;

impl Kugou {
    pub fn create(_param: Option<&str>) -> Box<dyn LyricsProvider> {
        Box::new(Self)
    }
}

impl LyricsProvider for Kugou {
    fn fetch_lyrics(&self, track: &TrackInfo, cfg: &PluginConfig) -> Result<Option<Lyrics>, Error> {
        if !cfg.wants_lrc() {
            return Ok(None);
        }

        let first_artist = track
            .artists
            .first()
            .ok_or_else(|| Error::new("missing artist"))?
            .name
            .as_str();

        let keyword = format!("{} {first_artist}", track.title);

        let song = match find_song(&keyword, track.duration)? {
            Some(s) => s,
            None => return Ok(None),
        };

        if song
            .trans_param
            .as_ref()
            .is_some_and(|p| p.language == "纯音乐")
        {
            return Ok(Some(Lyrics::Instrumental));
        }

        let candidate = match find_candidate(&song.hash, song.duration)? {
            Some(c) => c,
            None => return Ok(None),
        };

        let lrc = download_lrc(&candidate.id, &candidate.accesskey)?;

        if lrc::is_instrumental(&lrc) {
            Ok(Some(Lyrics::Instrumental))
        } else {
            Ok(Some(Lyrics::Lrc(lrc)))
        }
    }
}

fn find_song(keyword: &str, target_duration: f32) -> Result<Option<SongInfo>, Error> {
    let query = serde_urlencoded::to_string([
        ("format", "json"),
        ("keyword", keyword),
        ("page", "1"),
        ("pagesize", "10"),
        ("showtype", "1"),
    ])
    .map_err(|e| Error::new(format!("kugou: failed to encode song search query: {e}")))?;

    let response = send_request(&format!("{SONG_SEARCH_URL}?{query}"))?;

    if response.status_code != 200 {
        return Err(Error::new(format!(
            "kugou: song search returned status {}",
            response.status_code
        )));
    }

    let parsed: SongSearchResponse = serde_json::from_slice(&response.body)
        .map_err(|e| Error::new(format!("kugou: failed to parse song search response: {e}")))?;

    let tolerance = 2u64;
    let target_secs = target_duration.round() as u64;

    Ok(parsed.data.info.into_iter().find(|s| {
        s.duration
            .map(|d| d.abs_diff(target_secs) <= tolerance)
            .unwrap_or(false)
    }))
}

fn find_candidate(hash: &str, duration: Option<u64>) -> Result<Option<Candidate>, Error> {
    let duration_str = duration.unwrap_or(0).to_string();
    let query = serde_urlencoded::to_string([
        ("ver", "1"),
        ("man", "yes"),
        ("client", "mobi"),
        ("keyword", ""),
        ("duration", &duration_str),
        ("hash", hash),
        ("album_audio_id", ""),
    ])
    .map_err(|e| Error::new(format!("kugou: failed to encode lyrics search query: {e}")))?;

    let response = send_request(&format!("{LYRICS_SEARCH_URL}?{query}"))?;

    if response.status_code != 200 {
        return Err(Error::new(format!(
            "kugou: lyrics search returned status {}",
            response.status_code
        )));
    }

    let parsed: LyricsSearchResponse = serde_json::from_slice(&response.body).map_err(|e| {
        Error::new(format!(
            "kugou: failed to parse lyrics search response: {e}"
        ))
    })?;

    if parsed.status != 200 {
        return Ok(None);
    }

    Ok(parsed.candidates.into_iter().next())
}

fn download_lrc(id: &str, accesskey: &str) -> Result<String, Error> {
    let query = serde_urlencoded::to_string([
        ("ver", "1"),
        ("client", "pc"),
        ("id", id),
        ("accesskey", accesskey),
        ("fmt", "lrc"),
        ("charset", "utf8"),
    ])
    .map_err(|e| Error::new(format!("kugou: failed to encode download query: {e}")))?;

    let response = send_request(&format!("{DOWNLOAD_URL}?{query}"))?;

    if response.status_code != 200 {
        return Err(Error::new(format!(
            "kugou: download endpoint returned status {}",
            response.status_code
        )));
    }

    let parsed: DownloadResponse = serde_json::from_slice(&response.body)
        .map_err(|e| Error::new(format!("kugou: failed to parse download response: {e}")))?;

    let bytes = STANDARD
        .decode(&parsed.content)
        .map_err(|e| Error::new(format!("kugou: failed to decode lyrics content: {e}")))?;

    String::from_utf8(bytes)
        .map_err(|e| Error::new(format!("kugou: lyrics content is not valid UTF-8: {e}")))
}

fn send_request(url: &str) -> Result<HTTPResponse, Error> {
    let mut headers = HashMap::new();
    headers.insert("User-Agent".into(), FIREFOX_USER_AGENT.into());

    http::send(HTTPRequest {
        url: url.into(),
        method: "GET".into(),
        headers,
        no_follow_redirects: false,
        body: Vec::new(),
        timeout_ms: 15_000,
    })
    .map_err(|e| Error::new(format!("kugou: HTTP request failed: {e}")))?
    .ok_or_else(|| Error::new("kugou: received empty HTTP response"))
}
