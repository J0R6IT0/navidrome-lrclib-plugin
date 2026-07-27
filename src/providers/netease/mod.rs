use crate::{
    config::{PluginConfig, ProviderParams},
    ext::TrackInfoExt,
    format::lrc,
    providers::{BROWSER_USER_AGENT, LyricsProvider},
    types::{Lyrics, LyricsKind},
};
use extism_pdk::info;
use nd_pdk::{
    host::http::{self, HTTPRequest, HTTPResponse},
    lyrics::{Error, TrackInfo},
};
use serde::Deserialize;
use std::collections::HashMap;

mod yrc;

const SEARCH_URL: &str = "https://music.163.com/api/search/get";
const LYRICS_URL: &str = "https://music.163.com/api/song/lyric/v1";

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
    duration_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LyricsResponse {
    lrc: Option<LyricContent>,
    yrc: Option<LyricContent>,
    #[serde(default)]
    pure_music: bool,
}

#[derive(Debug, Deserialize)]
struct LyricContent {
    lyric: Option<String>,
}

impl LyricContent {
    fn text(content: &Option<LyricContent>) -> Option<&str> {
        content
            .as_ref()
            .and_then(|c| c.lyric.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }
}

pub struct NetEase;

impl NetEase {
    pub fn create(_params: &ProviderParams) -> Box<dyn LyricsProvider> {
        Box::new(Self)
    }
}

impl LyricsProvider for NetEase {
    fn supported_kinds(&self) -> &'static [LyricsKind] {
        // Even though NetEase can sometimes return plain lyrics in the "lrc" field,
        // that is considered a problem on their side, so plain lyrics are not officially
        // supported by this provider.
        &[LyricsKind::Lrc, LyricsKind::Elrc]
    }

    fn fetch_lyrics(&self, track: &TrackInfo, cfg: &PluginConfig) -> Result<Option<Lyrics>, Error> {
        let first_artist = track
            .first_artist()
            .ok_or_else(|| Error::new("missing artist"))?;

        let query = format!("{first_artist} {}", track.title);
        let target_ms = (track.duration * 1000.0).round() as u64;

        let song = match search_song(&query, target_ms, cfg.duration_tolerance_ms())? {
            Some(s) => s,
            None => return Ok(None),
        };

        let response = fetch_lyrics(song.id)?;

        if response.pure_music {
            info!("netease: track is instrumental");
            return Ok(Some(Lyrics::Instrumental));
        }

        let lrc = LyricContent::text(&response.lrc).map(strip_metadata);

        for &kind in &cfg.lyrics_type_priority {
            match kind {
                LyricsKind::Elrc => {
                    if let Some(raw) = LyricContent::text(&response.yrc) {
                        let elrc = yrc::to_enhanced_lrc(raw);
                        if !elrc.trim().is_empty() {
                            return Ok(Some(Lyrics::Elrc(elrc)));
                        }
                    }
                }
                LyricsKind::Lrc => {
                    if let Some(text) = &lrc
                        && lrc::is_synced(text)
                    {
                        return Ok(Some(Lyrics::Lrc(text.clone())));
                    }
                }
                LyricsKind::Plain => {
                    if let Some(text) = &lrc
                        && !lrc::is_synced(text)
                    {
                        return Ok(Some(Lyrics::Plain(text.clone())));
                    }
                }
                _ => {}
            }
        }

        Ok(None)
    }
}

fn search_song(query: &str, target_ms: u64, tolerance_ms: u64) -> Result<Option<Song>, Error> {
    let qs =
        serde_urlencoded::to_string([("s", query), ("type", "1"), ("limit", "5"), ("offset", "0")])
            .map_err(|e| Error::new(format!("netease: failed to encode search query: {e}")))?;

    let parsed: SearchResponse = get_json(SEARCH_URL, &qs, "search")?;

    Ok(parsed.result.songs.into_iter().find(|s| {
        s.duration_ms
            .map(|d| d.abs_diff(target_ms) <= tolerance_ms)
            .unwrap_or(false)
    }))
}

fn fetch_lyrics(song_id: u64) -> Result<LyricsResponse, Error> {
    let qs = serde_urlencoded::to_string([
        ("id", song_id.to_string()),
        ("lv", "-1".to_string()),
        ("kv", "-1".to_string()),
        ("tv", "-1".to_string()),
        ("yv", "1".to_string()),
    ])
    .map_err(|e| Error::new(format!("netease: failed to encode lyrics query: {e}")))?;

    get_json(LYRICS_URL, &qs, "lyrics")
}

fn strip_metadata(lyric: &str) -> String {
    lyric
        .lines()
        .filter(|line| !line.trim_start().starts_with('{'))
        .collect::<Vec<_>>()
        .join("\n")
}

fn get_json<T: serde::de::DeserializeOwned>(
    base_url: &str,
    query: &str,
    what: &str,
) -> Result<T, Error> {
    let response = send_request(&format!("{base_url}?{query}"))?;

    if response.status_code != 200 {
        return Err(Error::new(format!(
            "netease: {what} returned status {}",
            response.status_code
        )));
    }

    serde_json::from_slice(&response.body)
        .map_err(|e| Error::new(format!("netease: failed to parse {what} response: {e}")))
}

fn send_request(url: &str) -> Result<HTTPResponse, Error> {
    let mut headers = HashMap::new();
    headers.insert("User-Agent".into(), BROWSER_USER_AGENT.into());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_metadata_removes_json_lines() {
        let input = concat!(
            "{\"t\":0,\"c\":[{\"tx\":\"foo: \"}]}\n",
            "{\"t\":1000,\"c\":[{\"tx\":\"bar: \"}]}\n",
            "[00:28.15]foo\n",
            "[00:32.38]bar"
        );
        assert_eq!(strip_metadata(input), "[00:28.15]foo\n[00:32.38]bar");
    }

    #[test]
    fn test_strip_metadata_keeps_plain_lyrics() {
        let input = "Hello\nWorld";
        assert_eq!(strip_metadata(input), "Hello\nWorld");
    }

    #[test]
    fn test_strip_metadata_empty() {
        assert_eq!(strip_metadata(""), "");
    }

    #[test]
    fn test_lyric_content_text_blank_is_none() {
        let content = Some(LyricContent {
            lyric: Some("   \n  ".to_string()),
        });
        assert_eq!(LyricContent::text(&content), None);
    }

    #[test]
    fn test_lyric_content_text_missing_is_none() {
        assert_eq!(LyricContent::text(&None), None);
        assert_eq!(
            LyricContent::text(&Some(LyricContent { lyric: None })),
            None
        );
    }

    #[test]
    fn test_lyric_content_text_trims() {
        let content = Some(LyricContent {
            lyric: Some("  [00:01.00]hi  ".to_string()),
        });
        assert_eq!(LyricContent::text(&content), Some("[00:01.00]hi"));
    }
}
