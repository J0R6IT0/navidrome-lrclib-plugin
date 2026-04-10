use crate::config::PluginConfig;
use crate::storage::LyricsKind;
use nd_pdk::{
    host::http::{self, HTTPRequest, HTTPResponse},
    lyrics::{Error as LyricsError, TrackInfo},
};
use serde::Deserialize;
use std::collections::HashMap;

const USER_AGENT: &str =
    "navidrome-lrclib-plugin/2.0.0 (https://github.com/J0R6IT0/navidrome-lrclib-plugin)";
const BASE_URL: &str = "https://lrclib.net/api";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LyricsRecord {
    synced_lyrics: Option<String>,
    plain_lyrics: Option<String>,
    #[serde(default)]
    duration: f32,
    #[serde(default)]
    instrumental: bool,
}

pub fn fetch_lyrics_text(
    track: &TrackInfo,
    cfg: &PluginConfig,
) -> Result<Option<(String, LyricsKind)>, LyricsError> {
    let first_artist = track
        .artists
        .first()
        .ok_or_else(|| LyricsError::new("missing artist"))?
        .name
        .as_str();

    let all_artists = track
        .artists
        .iter()
        .map(|a| a.name.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    let duration_str = track.duration.round().to_string();

    if let Some(record) = get_by_metadata(&all_artists, &track.title, &track.album, &duration_str)?
    {
        if let Some(result) = pick_text(record, cfg.fetch_synced) {
            return Ok(Some(result));
        }
    }

    let query = format!("{} {}", first_artist, track.title);
    if let Some(record) = search_by_query(&query, track.duration)? {
        if let Some(result) = pick_text(record, cfg.fetch_synced) {
            return Ok(Some(result));
        }
    }

    Ok(None)
}

fn get_by_metadata(
    artist: &str,
    title: &str,
    album: &str,
    duration: &str,
) -> Result<Option<LyricsRecord>, LyricsError> {
    let query = serde_urlencoded::to_string([
        ("artist_name", artist),
        ("track_name", title),
        ("album_name", album),
        ("duration", duration),
    ])
    .map_err(|e| LyricsError::new(e.to_string()))?;

    let response = send_request(&format!("{}/get?{}", BASE_URL, query))?;

    match response.status_code {
        200 => serde_json::from_slice(&response.body)
            .map(Some)
            .map_err(|e| LyricsError::new(e.to_string())),
        404 => Ok(None),
        code => Err(LyricsError::new(format!("lrclib returned status {}", code))),
    }
}

fn search_by_query(q: &str, duration: f32) -> Result<Option<LyricsRecord>, LyricsError> {
    let query =
        serde_urlencoded::to_string([("q", q)]).map_err(|e| LyricsError::new(e.to_string()))?;

    let response = send_request(&format!("{}/search?{}", BASE_URL, query))?;

    if response.status_code != 200 {
        return Err(LyricsError::new(format!(
            "lrclib search returned status {}",
            response.status_code
        )));
    }

    let records: Vec<LyricsRecord> =
        serde_json::from_slice(&response.body).map_err(|e| LyricsError::new(e.to_string()))?;

    Ok(records
        .into_iter()
        .find(|r| (r.duration - duration).abs() <= 2.0))
}

fn send_request(url: &str) -> Result<HTTPResponse, LyricsError> {
    let mut headers = HashMap::new();
    headers.insert("User-Agent".into(), USER_AGENT.into());

    http::send(HTTPRequest {
        url: url.into(),
        method: "GET".into(),
        headers,
        no_follow_redirects: false,
        body: Vec::new(),
        timeout_ms: 10_000,
    })
    .map_err(|e| LyricsError::new(e.to_string()))?
    .ok_or_else(|| LyricsError::new("empty HTTP response"))
}

fn pick_text(record: LyricsRecord, prefer_synced: bool) -> Option<(String, LyricsKind)> {
    if record.instrumental {
        return Some(("Instrumental".to_string(), LyricsKind::Plain));
    }

    let synced = record.synced_lyrics.filter(|s| !s.is_empty());
    let plain = record.plain_lyrics.filter(|s| !s.is_empty());

    if prefer_synced {
        synced
            .map(|t| (t, LyricsKind::Synchronized))
            .or_else(|| plain.map(|t| (t, LyricsKind::Plain)))
    } else {
        plain.map(|t| (t, LyricsKind::Plain))
    }
}
