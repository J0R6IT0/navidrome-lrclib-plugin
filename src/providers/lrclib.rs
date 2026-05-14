use crate::{
    config::PluginConfig,
    providers::{LyricsProvider, USER_AGENT},
    types::LyricsType,
};
use nd_pdk::{
    host::http::{self, HTTPRequest, HTTPResponse},
    lyrics::{Error as LyricsError, TrackInfo},
};
use serde::Deserialize;
use std::collections::HashMap;

const DEFAULT_BASE_URL: &str = "https://lrclib.net";
const DURATION_TOLERANCE_SECS: f32 = 2.0;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibRecord {
    synced_lyrics: Option<String>,
    plain_lyrics: Option<String>,
    duration: Option<f32>,
    #[serde(default)]
    instrumental: bool,
}

pub struct Lrclib {
    base_url: String,
}

impl Lrclib {
    pub fn create(param: Option<&str>) -> Box<dyn LyricsProvider> {
        Box::new(Self {
            base_url: param.unwrap_or(DEFAULT_BASE_URL).to_string(),
        })
    }
}

impl LyricsProvider for Lrclib {
    fn fetch_lyrics(
        &self,
        track: &TrackInfo,
        cfg: &PluginConfig,
    ) -> Result<Option<(String, LyricsType)>, LyricsError> {
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

        if let Some(record) = get_by_metadata(
            &self.base_url,
            &all_artists,
            &track.title,
            &track.album,
            track.duration,
        )? && let Some(result) = pick_text(record, cfg)
        {
            return Ok(Some(result));
        }

        let query = format!("{first_artist} {}", track.title);
        if let Some(record) = search_by_query(&self.base_url, &query, track.duration)?
            && let Some(result) = pick_text(record, cfg)
        {
            return Ok(Some(result));
        }

        Ok(None)
    }
}

fn get_by_metadata(
    base_url: &str,
    artist: &str,
    title: &str,
    album: &str,
    duration: f32,
) -> Result<Option<LrclibRecord>, LyricsError> {
    let query = serde_urlencoded::to_string([
        ("artist_name", artist),
        ("track_name", title),
        ("album_name", album),
        ("duration", &duration.round().to_string()),
    ])
    .map_err(|e| LyricsError::new(format!("lrclib: failed to encode query: {e}")))?;

    let response = send_request(&format!("{base_url}/api/get?{query}"))?;

    match response.status_code {
        200 => serde_json::from_slice(&response.body)
            .map(Some)
            .map_err(|e| LyricsError::new(format!("lrclib: failed to parse get response: {e}"))),
        404 => Ok(None),
        code => Err(LyricsError::new(format!(
            "lrclib: get endpoint returned status {code}"
        ))),
    }
}

fn search_by_query(
    base_url: &str,
    q: &str,
    target_duration: f32,
) -> Result<Option<LrclibRecord>, LyricsError> {
    let query = serde_urlencoded::to_string([("q", q)])
        .map_err(|e| LyricsError::new(format!("lrclib: failed to encode search query: {e}")))?;

    let response = send_request(&format!("{base_url}/api/search?{query}"))?;

    if response.status_code != 200 {
        return Err(LyricsError::new(format!(
            "lrclib: search endpoint returned status {}",
            response.status_code
        )));
    }

    let records: Vec<LrclibRecord> = serde_json::from_slice(&response.body)
        .map_err(|e| LyricsError::new(format!("lrclib: failed to parse search response: {e}")))?;

    Ok(records.into_iter().find(|r| {
        r.duration
            .map(|d| (d - target_duration).abs() <= DURATION_TOLERANCE_SECS)
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
        timeout_ms: 15_000,
    })
    .map_err(|e| LyricsError::new(format!("lrclib: HTTP request failed: {e}")))?
    .ok_or_else(|| LyricsError::new("lrclib: received empty HTTP response"))
}

fn pick_text(record: LrclibRecord, cfg: &PluginConfig) -> Option<(String, LyricsType)> {
    if record.instrumental {
        return Some(("Instrumental".to_string(), LyricsType::Plain));
    }

    let synced = record.synced_lyrics.filter(|s| !s.trim().is_empty());
    let plain = record.plain_lyrics.filter(|s| !s.trim().is_empty());

    for &kind in cfg.resolve_order() {
        let text = match kind {
            LyricsType::Synced => synced.as_ref(),
            LyricsType::Plain => plain.as_ref(),
        };

        if let Some(text) = text {
            return Some((text.clone(), kind));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(priority: Vec<LyricsType>) -> PluginConfig {
        PluginConfig {
            lyrics_type_priority: priority,
            ..Default::default()
        }
    }

    #[test]
    fn test_pick_text_synced_priority() {
        let cfg = make_config(vec![LyricsType::Synced, LyricsType::Plain]);
        let record = LrclibRecord {
            synced_lyrics: Some("[00:00.00] Hello".to_string()),
            plain_lyrics: Some("Hello".to_string()),
            duration: Some(180.0),
            instrumental: false,
        };

        let result = pick_text(record, &cfg);
        assert_eq!(
            result,
            Some(("[00:00.00] Hello".to_string(), LyricsType::Synced))
        );
    }

    #[test]
    fn test_pick_text_falls_back_to_plain() {
        let cfg = make_config(vec![LyricsType::Synced, LyricsType::Plain]);
        let record = LrclibRecord {
            synced_lyrics: None,
            plain_lyrics: Some("Hello".to_string()),
            duration: Some(180.0),
            instrumental: false,
        };

        let result = pick_text(record, &cfg);
        assert_eq!(result, Some(("Hello".to_string(), LyricsType::Plain)));
    }

    #[test]
    fn test_pick_text_skips_empty_synced() {
        let cfg = make_config(vec![LyricsType::Synced, LyricsType::Plain]);
        let record = LrclibRecord {
            synced_lyrics: Some("   ".to_string()), // whitespace only
            plain_lyrics: Some("Hello".to_string()),
            duration: Some(180.0),
            instrumental: false,
        };

        let result = pick_text(record, &cfg);
        assert_eq!(result, Some(("Hello".to_string(), LyricsType::Plain)));
    }

    #[test]
    fn test_pick_text_instrumental() {
        let cfg = make_config(vec![LyricsType::Synced, LyricsType::Plain]);
        let record = LrclibRecord {
            synced_lyrics: None,
            plain_lyrics: None,
            duration: Some(180.0),
            instrumental: true,
        };

        let result = pick_text(record, &cfg);
        assert_eq!(
            result,
            Some(("Instrumental".to_string(), LyricsType::Plain))
        );
    }

    #[test]
    fn test_pick_text_no_lyrics_available() {
        let cfg = make_config(vec![LyricsType::Synced, LyricsType::Plain]);
        let record = LrclibRecord {
            synced_lyrics: None,
            plain_lyrics: None,
            duration: Some(180.0),
            instrumental: false,
        };

        let result = pick_text(record, &cfg);
        assert_eq!(result, None);
    }
}
