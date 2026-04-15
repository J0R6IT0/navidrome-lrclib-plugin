use crate::{
    LyricsKind,
    config::{LyricsMode, PluginConfig},
};
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
    duration: Option<f32>,
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
        if let Some(result) = pick_text(record, cfg.lyrics_mode) {
            return Ok(Some(result));
        }
    }

    let query = format!("{} {}", first_artist, track.title);
    if let Some(record) = search_by_query(&query, track.duration)? {
        if let Some(result) = pick_text(record, cfg.lyrics_mode) {
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

    Ok(records.into_iter().find(|r| {
        r.duration
            .map(|d| (d - duration).abs() <= 2.0)
            .unwrap_or(false)
    }))
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

fn pick_text(record: LyricsRecord, mode: LyricsMode) -> Option<(String, LyricsKind)> {
    if record.instrumental {
        return Some(("Instrumental".to_string(), LyricsKind::Plain));
    }

    let synced = record.synced_lyrics.filter(|s| !s.trim().is_empty());
    let plain = record.plain_lyrics.filter(|s| !s.trim().is_empty());

    match mode {
        LyricsMode::BothPreferSynced => synced
            .map(|t| (t, LyricsKind::Synchronized))
            .or_else(|| plain.map(|t| (t, LyricsKind::Plain))),
        LyricsMode::BothPreferPlain => plain
            .map(|t| (t, LyricsKind::Plain))
            .or_else(|| synced.map(|t| (t, LyricsKind::Synchronized))),
        LyricsMode::SyncedOnly => synced.map(|t| (t, LyricsKind::Synchronized)),
        LyricsMode::PlainOnly => plain.map(|t| (t, LyricsKind::Plain)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(synced: Option<&str>, plain: Option<&str>, instrumental: bool) -> LyricsRecord {
        LyricsRecord {
            synced_lyrics: synced.map(|s| s.to_string()),
            plain_lyrics: plain.map(|s| s.to_string()),
            duration: Some(200.0),
            instrumental,
        }
    }

    #[test]
    fn test_pick_text_synced_only() {
        let r = record(Some("SYNC"), Some("PLAIN"), false);

        let res = pick_text(r, LyricsMode::SyncedOnly);

        assert_eq!(res, Some(("SYNC".to_string(), LyricsKind::Synchronized)));
    }

    #[test]
    fn test_pick_text_plain_only() {
        let r = record(Some("SYNC"), Some("PLAIN"), false);

        let res = pick_text(r, LyricsMode::PlainOnly);

        assert_eq!(res, Some(("PLAIN".to_string(), LyricsKind::Plain)));
    }

    #[test]
    fn test_pick_text_both_prefer_synced() {
        let r = record(Some("SYNC"), Some("PLAIN"), false);

        let res = pick_text(r, LyricsMode::BothPreferSynced);

        assert_eq!(res, Some(("SYNC".to_string(), LyricsKind::Synchronized)));
    }

    #[test]
    fn test_pick_text_fallback_to_plain() {
        let r = record(None, Some("PLAIN"), false);

        let res = pick_text(r, LyricsMode::BothPreferSynced);

        assert_eq!(res, Some(("PLAIN".to_string(), LyricsKind::Plain)));
    }

    #[test]
    fn test_pick_text_both_prefer_plain() {
        let r = record(Some("SYNC"), Some("PLAIN"), false);

        let res = pick_text(r, LyricsMode::BothPreferPlain);

        assert_eq!(res, Some(("PLAIN".to_string(), LyricsKind::Plain)));
    }

    #[test]
    fn test_pick_text_instrumental_overrides() {
        let r = record(Some("SYNC"), Some("PLAIN"), true);

        let res = pick_text(r, LyricsMode::SyncedOnly);

        assert_eq!(res, Some(("Instrumental".to_string(), LyricsKind::Plain)));
    }

    #[test]
    fn test_empty_strings_are_ignored() {
        let r = LyricsRecord {
            synced_lyrics: Some("".to_string()),
            plain_lyrics: Some("  ".to_string()),
            duration: Some(200.0),
            instrumental: false,
        };

        let res = pick_text(r, LyricsMode::BothPreferSynced);

        assert_eq!(res, None);
    }
}
