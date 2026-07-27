use crate::{
    config::{PluginConfig, ProviderParams},
    ext::TrackInfoExt,
    format::lrc,
    providers::{BROWSER_USER_AGENT, LyricsProvider},
    types::{Lyrics, LyricsKind},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use extism_pdk::{info, warn};
use nd_pdk::{
    host::http::{self, HTTPRequest, HTTPResponse},
    lyrics::{Error, TrackInfo},
};
use serde::Deserialize;
use std::collections::HashMap;

mod krc;

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
    #[serde(default)]
    krctype: i64,
}

#[derive(Debug, Deserialize)]
struct DownloadResponse {
    content: String,
}

pub struct Kugou;

impl Kugou {
    pub fn create(_params: &ProviderParams) -> Box<dyn LyricsProvider> {
        Box::new(Self)
    }
}

impl LyricsProvider for Kugou {
    fn supported_kinds(&self) -> &'static [LyricsKind] {
        &[LyricsKind::Lrc, LyricsKind::Elrc]
    }

    fn fetch_lyrics(&self, track: &TrackInfo, cfg: &PluginConfig) -> Result<Option<Lyrics>, Error> {
        let first_artist = track
            .first_artist()
            .ok_or_else(|| Error::new("missing artist"))?;

        let keyword = format!("{} {first_artist}", track.title);
        let target_ms = (track.duration * 1000.0).round() as u64;

        let song = match find_song(&keyword, target_ms, cfg.duration_tolerance_ms())? {
            Some(s) => s,
            None => return Ok(None),
        };

        if song
            .trans_param
            .as_ref()
            .is_some_and(|p| p.language == "纯音乐")
        {
            info!("kugou: track is instrumental");
            return Ok(Some(Lyrics::Instrumental));
        }

        let candidate = match find_candidate(&song.hash, song.duration)? {
            Some(c) => c,
            None => return Ok(None),
        };

        for &kind in &cfg.lyrics_type_priority {
            match kind {
                LyricsKind::Elrc if candidate.krctype != 0 => {
                    let bytes = download_raw(&candidate.id, &candidate.accesskey, "krc")?;
                    match krc::to_enhanced_lrc(&bytes) {
                        Ok(elrc) if !elrc.trim().is_empty() => {
                            return Ok(Some(Lyrics::Elrc(elrc)));
                        }
                        Ok(_) => {}
                        Err(e) => warn!("kugou: failed to decode krc lyrics: {e}"),
                    }
                }
                LyricsKind::Lrc => {
                    let bytes = download_raw(&candidate.id, &candidate.accesskey, "lrc")?;
                    let lrc = String::from_utf8(bytes).map_err(|e| {
                        Error::new(format!("kugou: lyrics content is not valid UTF-8: {e}"))
                    })?;

                    if lrc::is_instrumental(&lrc) {
                        info!("kugou: track is instrumental");
                        return Ok(Some(Lyrics::Instrumental));
                    }
                    if !lrc.trim().is_empty() {
                        return Ok(Some(Lyrics::Lrc(lrc)));
                    }
                }
                _ => {}
            }
        }

        Ok(None)
    }
}

fn find_song(keyword: &str, target_ms: u64, tolerance_ms: u64) -> Result<Option<SongInfo>, Error> {
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

    Ok(parsed.data.info.into_iter().find(|s| {
        s.duration
            .map(|secs| (secs * 1000).abs_diff(target_ms) <= tolerance_ms)
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

fn download_raw(id: &str, accesskey: &str, fmt: &str) -> Result<Vec<u8>, Error> {
    let query = serde_urlencoded::to_string([
        ("ver", "1"),
        ("client", "pc"),
        ("id", id),
        ("accesskey", accesskey),
        ("fmt", fmt),
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

    STANDARD
        .decode(&parsed.content)
        .map_err(|e| Error::new(format!("kugou: failed to decode lyrics content: {e}")))
}

fn send_request(url: &str) -> Result<HTTPResponse, Error> {
    let mut headers = HashMap::new();
    headers.insert("User-Agent".into(), BROWSER_USER_AGENT.into());

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
