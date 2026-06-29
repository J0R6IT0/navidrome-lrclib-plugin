use crate::{
    config::{PluginConfig, ProviderParams},
    providers::{LyricsProvider, USER_AGENT},
    types::{Lyrics, LyricsKind},
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
    #[serde(default)]
    lyricsfile: Option<String>,
    duration: Option<f32>,
    #[serde(default)]
    instrumental: bool,
}

impl LrclibRecord {
    fn matches_duration(&self, target: f32) -> bool {
        self.duration
            .is_some_and(|d| (d - target).abs() <= DURATION_TOLERANCE_SECS)
    }
}

pub struct Lrclib {
    base_url: String,
}

impl Lrclib {
    pub fn create(params: &ProviderParams) -> Box<dyn LyricsProvider> {
        Box::new(Self {
            base_url: params
                .get("baseUrl")
                .unwrap_or(DEFAULT_BASE_URL)
                .to_string(),
        })
    }
}

impl LyricsProvider for Lrclib {
    fn supported_kinds(&self) -> &'static [LyricsKind] {
        &[LyricsKind::Lrc, LyricsKind::Plain, LyricsKind::Lyricsfile]
    }

    fn fetch_lyrics(
        &self,
        track: &TrackInfo,
        cfg: &PluginConfig,
    ) -> Result<Option<Lyrics>, LyricsError> {
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

        let preferred = preferred_over_plain(cfg);

        let mut plain_fallback: Option<Lyrics> = None;
        if let Some(record) = get_by_metadata(
            &self.base_url,
            &all_artists,
            &track.title,
            &track.album,
            track.duration,
        )? {
            match pick_text(record, cfg) {
                Some(plain @ Lyrics::Plain(_)) if !preferred.is_empty() => {
                    plain_fallback = Some(plain);
                }
                Some(result) => return Ok(Some(result)),
                None => {}
            }
        }

        let query = format!("{first_artist} {}", track.title);
        for record in search_by_query(&self.base_url, &query)? {
            if !record.matches_duration(track.duration) {
                continue;
            }

            let picked = match &plain_fallback {
                Some(_) => pick_kinds(record, &preferred),
                None => pick_text(record, cfg),
            };

            if let Some(result) = picked {
                return Ok(Some(result));
            }
        }

        Ok(plain_fallback)
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

fn search_by_query(base_url: &str, q: &str) -> Result<Vec<LrclibRecord>, LyricsError> {
    let query = serde_urlencoded::to_string([("q", q)])
        .map_err(|e| LyricsError::new(format!("lrclib: failed to encode search query: {e}")))?;

    let response = send_request(&format!("{base_url}/api/search?{query}"))?;

    if response.status_code != 200 {
        return Err(LyricsError::new(format!(
            "lrclib: search endpoint returned status {}",
            response.status_code
        )));
    }

    serde_json::from_slice(&response.body)
        .map_err(|e| LyricsError::new(format!("lrclib: failed to parse search response: {e}")))
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

fn pick_text(record: LrclibRecord, cfg: &PluginConfig) -> Option<Lyrics> {
    if record.instrumental {
        return Some(Lyrics::Instrumental);
    }

    pick_kinds(record, cfg.resolve_order())
}

fn pick_kinds(record: LrclibRecord, order: &[LyricsKind]) -> Option<Lyrics> {
    let nonblank = |s: Option<String>| s.filter(|s| !s.trim().is_empty());
    let mut synced = nonblank(record.synced_lyrics);
    let mut plain = nonblank(record.plain_lyrics);
    let mut lyricsfile = nonblank(record.lyricsfile);

    order.iter().find_map(|kind| match kind {
        LyricsKind::Lrc => synced.take().map(Lyrics::Lrc),
        LyricsKind::Plain => plain.take().map(Lyrics::Plain),
        LyricsKind::Lyricsfile => lyricsfile.take().map(Lyrics::Lyricsfile),
        _ => None,
    })
}

fn preferred_over_plain(cfg: &PluginConfig) -> Vec<LyricsKind> {
    let order = cfg.resolve_order();
    let cutoff = order
        .iter()
        .position(|&k| k == LyricsKind::Plain)
        .unwrap_or(order.len());

    order[..cutoff]
        .iter()
        .copied()
        .filter(|k| matches!(k, LyricsKind::Lrc | LyricsKind::Lyricsfile))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(priority: Vec<LyricsKind>) -> PluginConfig {
        PluginConfig {
            lyrics_type_priority: priority,
            ..Default::default()
        }
    }

    #[test]
    fn test_pick_text_synced_priority() {
        let cfg = make_config(vec![LyricsKind::Lrc, LyricsKind::Plain]);
        let record = LrclibRecord {
            synced_lyrics: Some("[00:00.00] Hello".to_string()),
            plain_lyrics: Some("Hello".to_string()),
            lyricsfile: None,
            duration: Some(180.0),
            instrumental: false,
        };

        let result = pick_text(record, &cfg);
        assert_eq!(result, Some(Lyrics::Lrc("[00:00.00] Hello".to_string())));
    }

    #[test]
    fn test_pick_text_falls_back_to_plain() {
        let cfg = make_config(vec![LyricsKind::Lrc, LyricsKind::Plain]);
        let record = LrclibRecord {
            synced_lyrics: None,
            plain_lyrics: Some("Hello".to_string()),
            lyricsfile: None,
            duration: Some(180.0),
            instrumental: false,
        };

        let result = pick_text(record, &cfg);
        assert_eq!(result, Some(Lyrics::Plain("Hello".to_string())));
    }

    #[test]
    fn test_pick_text_skips_empty_synced() {
        let cfg = make_config(vec![LyricsKind::Lrc, LyricsKind::Plain]);
        let record = LrclibRecord {
            synced_lyrics: Some("   ".to_string()), // whitespace only
            plain_lyrics: Some("Hello".to_string()),
            lyricsfile: None,
            duration: Some(180.0),
            instrumental: false,
        };

        let result = pick_text(record, &cfg);
        assert_eq!(result, Some(Lyrics::Plain("Hello".to_string())));
    }

    #[test]
    fn test_pick_text_instrumental() {
        let cfg = make_config(vec![LyricsKind::Lrc, LyricsKind::Plain]);
        let record = LrclibRecord {
            synced_lyrics: None,
            plain_lyrics: None,
            lyricsfile: None,
            duration: Some(180.0),
            instrumental: true,
        };

        let result = pick_text(record, &cfg);
        assert_eq!(result, Some(Lyrics::Instrumental));
    }

    #[test]
    fn test_pick_text_lyricsfile_priority() {
        let cfg = make_config(vec![LyricsKind::Lyricsfile, LyricsKind::Lrc]);
        let record = LrclibRecord {
            synced_lyrics: Some("[00:00.00] Hello".to_string()),
            plain_lyrics: None,
            lyricsfile: Some("version: 1.0\nlines: []".to_string()),
            duration: Some(180.0),
            instrumental: false,
        };

        let result = pick_text(record, &cfg);
        assert_eq!(
            result,
            Some(Lyrics::Lyricsfile("version: 1.0\nlines: []".to_string()))
        );
    }

    #[test]
    fn test_pick_text_no_lyrics_available() {
        let cfg = make_config(vec![LyricsKind::Lrc, LyricsKind::Plain]);
        let record = LrclibRecord {
            synced_lyrics: None,
            plain_lyrics: None,
            lyricsfile: None,
            duration: Some(180.0),
            instrumental: false,
        };

        let result = pick_text(record, &cfg);
        assert_eq!(result, None);
    }

    #[test]
    fn test_preferred_over_plain_synced_above_plain() {
        let cfg = make_config(vec![
            LyricsKind::Lyricsfile,
            LyricsKind::Lrc,
            LyricsKind::Plain,
        ]);
        assert_eq!(
            preferred_over_plain(&cfg),
            vec![LyricsKind::Lyricsfile, LyricsKind::Lrc]
        );
    }

    #[test]
    fn test_preferred_over_plain_plain_first_is_empty() {
        let cfg = make_config(vec![LyricsKind::Plain, LyricsKind::Lrc]);
        assert_eq!(preferred_over_plain(&cfg), Vec::<LyricsKind>::new());
    }

    #[test]
    fn test_preferred_over_plain_only_counts_above_plain() {
        let cfg = make_config(vec![
            LyricsKind::Lyricsfile,
            LyricsKind::Plain,
            LyricsKind::Lrc,
        ]);
        assert_eq!(preferred_over_plain(&cfg), vec![LyricsKind::Lyricsfile]);
    }

    #[test]
    fn test_preferred_over_plain_no_plain_in_config() {
        let cfg = make_config(vec![LyricsKind::Lrc, LyricsKind::Lyricsfile]);
        assert_eq!(
            preferred_over_plain(&cfg),
            vec![LyricsKind::Lrc, LyricsKind::Lyricsfile]
        );
    }

    #[test]
    fn test_pick_kinds_ignores_instrumental_flag() {
        let record = LrclibRecord {
            synced_lyrics: Some("[00:00.00] Hello".to_string()),
            plain_lyrics: Some("Hello".to_string()),
            lyricsfile: None,
            duration: Some(180.0),
            instrumental: true,
        };

        let result = pick_kinds(record, &[LyricsKind::Lrc]);
        assert_eq!(result, Some(Lyrics::Lrc("[00:00.00] Hello".to_string())));
    }

    #[test]
    fn test_pick_kinds_skips_plain_when_not_in_order() {
        let record = LrclibRecord {
            synced_lyrics: None,
            plain_lyrics: Some("Hello".to_string()),
            lyricsfile: None,
            duration: Some(180.0),
            instrumental: false,
        };

        let result = pick_kinds(record, &[LyricsKind::Lrc, LyricsKind::Lyricsfile]);
        assert_eq!(result, None);
    }
}
