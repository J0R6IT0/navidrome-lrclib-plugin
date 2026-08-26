use crate::{
    config::{PluginConfig, ProviderParams},
    ext::TrackInfoExt,
    format::lrc,
    providers::{LyricsProvider, ProviderResult, http::Http},
    types::{Lyrics, LyricsKind},
};
use extism_pdk::info;
use nd_pdk::lyrics::TrackInfo;
use serde::Deserialize;

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

    fn fetch_lyrics(
        &self,
        track: &TrackInfo,
        cfg: &PluginConfig,
    ) -> ProviderResult<Option<Lyrics>> {
        let query = format!("{} {}", track.first_artist().unwrap(), track.title);
        let target_ms = track.duration_ms();

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

fn search_song(query: &str, target_ms: u64, tolerance_ms: u64) -> ProviderResult<Option<Song>> {
    let parsed: SearchResponse = get_json(
        Http::get(SEARCH_URL)
            .param("s", query)
            .param("type", "1")
            .param("limit", "5")
            .param("offset", "0"),
        "the search",
    )?;

    Ok(parsed.result.songs.into_iter().find(|s| {
        s.duration_ms
            .map(|d| d.abs_diff(target_ms) <= tolerance_ms)
            .unwrap_or(false)
    }))
}

fn fetch_lyrics(song_id: u64) -> ProviderResult<LyricsResponse> {
    get_json(
        Http::get(LYRICS_URL)
            .param("id", song_id.to_string())
            .param("lv", "-1")
            .param("kv", "-1")
            .param("tv", "-1")
            .param("yv", "1"),
        "the lyrics",
    )
}

fn strip_metadata(lyric: &str) -> String {
    lyric
        .lines()
        .filter(|line| !line.trim_start().starts_with('{'))
        .collect::<Vec<_>>()
        .join("\n")
}

fn get_json<T: serde::de::DeserializeOwned>(request: Http, what: &str) -> ProviderResult<T> {
    let response = request
        .browser()
        .header("Referer", "https://music.163.com")
        .send()?;

    match response.status {
        200 => response.json(what),
        _ => Err(response.unexpected_status(what)),
    }
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
