use crate::{
    config::{PluginConfig, ProviderParams},
    ext::TrackInfoExt,
    format::{elrc, lrc},
    providers::{LyricsProvider, USER_AGENT},
    types::{Lyrics, LyricsKind},
};
use nd_pdk::{
    host::http::{self, HTTPRequest, HTTPResponse},
    lyrics::{Error, TrackInfo},
};
use serde::Deserialize;
use std::collections::HashMap;

const DEFAULT_BASE_URL: &str = "https://api.lrcmux.dev";

#[derive(Deserialize)]
struct JsonResponse {
    meta: JsonMeta,
    lines: Option<Vec<Line>>,
}

#[derive(Deserialize)]
struct JsonMeta {
    level: SyncLevel,
    #[serde(default)]
    instrumental: bool,
}

#[derive(Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum SyncLevel {
    Word,
    Line,
    None,
}

#[derive(Deserialize)]
struct Line {
    text: String,
    start: Option<i64>,
    end: Option<i64>,
    words: Option<Vec<Word>>,
}

#[derive(Deserialize)]
struct Word {
    text: String,
    start: i64,
    end: Option<i64>,
}

const KNOWN_SOURCES: &[&str] = &["genius", "kugou", "musixmatch", "netease", "ytmusic"];

pub struct LrcMux {
    base_url: String,
    sources: Option<String>,
}

impl LrcMux {
    pub fn create(params: &ProviderParams) -> Box<dyn LyricsProvider> {
        Box::new(Self {
            base_url: params
                .get("baseUrl")
                .unwrap_or(DEFAULT_BASE_URL)
                .to_string(),
            sources: restrict_sources(params.get("sources")),
        })
    }
}

fn restrict_sources(configured: Option<&str>) -> Option<String> {
    let picked: Vec<&str> = configured?
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    if picked.is_empty() || KNOWN_SOURCES.iter().all(|known| picked.contains(known)) {
        return None;
    }
    Some(picked.join(","))
}

impl LyricsProvider for LrcMux {
    fn supported_kinds(&self) -> &'static [LyricsKind] {
        &[LyricsKind::Elrc, LyricsKind::Lrc, LyricsKind::Plain]
    }

    fn log_params(&self) -> Vec<(&'static str, String)> {
        let mut params = vec![("baseUrl", self.base_url.clone())];
        if let Some(sources) = &self.sources {
            params.push(("sources", sources.clone()));
        }
        params
    }

    fn fetch_lyrics(&self, track: &TrackInfo, cfg: &PluginConfig) -> Result<Option<Lyrics>, Error> {
        let first_artist = track
            .first_artist()
            .ok_or_else(|| Error::new("missing artist"))?;

        let duration = track.duration.round() as i64;

        let duration = duration.to_string();
        let mut query = vec![
            ("artist", first_artist),
            ("title", track.title.as_str()),
            ("album", track.album.as_str()),
            ("duration", duration.as_str()),
        ];
        if let Some(sources) = &self.sources {
            query.push(("sources", sources.as_str()));
        }

        let qs = serde_urlencoded::to_string(&query)
            .map_err(|e| Error::new(format!("lrcmux: failed to encode query: {e}")))?;

        let response = send_request(&format!("{}/get?{qs}", self.base_url))?;

        match response.status_code {
            200 => {
                let parsed: JsonResponse = serde_json::from_slice(&response.body)
                    .map_err(|e| Error::new(format!("lrcmux: failed to parse response: {e}")))?;

                if parsed.meta.instrumental {
                    return Ok(Some(Lyrics::Instrumental));
                }

                let level = parsed.meta.level;
                let lines = parsed.lines.unwrap_or_default();

                for &kind in &cfg.lyrics_type_priority {
                    match kind {
                        LyricsKind::Elrc => {
                            if level != SyncLevel::Word {
                                continue;
                            }
                            let elrc = build_elrc(&lines);
                            if elrc.is_empty() {
                                continue;
                            }
                            return Ok(Some(Lyrics::Elrc(elrc)));
                        }
                        LyricsKind::Lrc => {
                            if level == SyncLevel::None {
                                continue;
                            }
                            let lrc = build_lrc(&lines);
                            if lrc.is_empty() {
                                continue;
                            }
                            return Ok(Some(Lyrics::Lrc(lrc)));
                        }
                        LyricsKind::Plain => {
                            let text = lines
                                .iter()
                                .map(|l| l.text.as_str())
                                .collect::<Vec<_>>()
                                .join("\n");
                            return Ok(Some(Lyrics::Plain(text)));
                        }
                        _ => {}
                    }
                }
                Ok(None)
            }
            404 => Ok(None),
            code => Err(Error::new(format!(
                "lrcmux: returned unexpected status {code}"
            ))),
        }
    }
}

fn timed_words(line: &Line, words: &[Word]) -> Vec<elrc::Word> {
    words
        .iter()
        .enumerate()
        .map(|(i, w)| elrc::Word {
            text: w.text.clone(),
            start_ms: w.start,
            end_ms: w
                .end
                .or_else(|| words.get(i + 1).map(|next| next.start))
                .or(line.end)
                .unwrap_or(w.start),
        })
        .collect()
}

fn build_elrc(lines: &[Line]) -> String {
    lines
        .iter()
        .filter_map(|l| elrc::render_line(l.start?, &timed_words(l, l.words.as_deref()?)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_lrc(lines: &[Line]) -> String {
    lines
        .iter()
        .filter_map(|l| Some(format!("[{}] {}", lrc::format_timestamp(l.start?), l.text)))
        .collect::<Vec<_>>()
        .join("\n")
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
    .map_err(|e| Error::new(format!("lrcmux: HTTP request failed: {e}")))?
    .ok_or_else(|| Error::new("lrcmux: received empty response"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sources_are_unrestricted_when_unset_or_complete() {
        assert_eq!(restrict_sources(None), None);
        assert_eq!(restrict_sources(Some("")), None);
        assert_eq!(restrict_sources(Some(" , ")), None);
        assert_eq!(
            restrict_sources(Some("genius,kugou,musixmatch,netease,ytmusic")),
            None
        );
        assert_eq!(
            restrict_sources(Some("ytmusic,netease,musixmatch,kugou,genius,future")),
            None
        );
    }

    #[test]
    fn test_sources_restrict_when_a_source_is_deselected() {
        assert_eq!(
            restrict_sources(Some("genius,kugou,musixmatch,netease")),
            Some("genius,kugou,musixmatch,netease".to_string())
        );
        assert_eq!(
            restrict_sources(Some(" musixmatch , kugou ")),
            Some("musixmatch,kugou".to_string())
        );
    }

    fn word(text: &str, start: i64, end: i64) -> Word {
        Word {
            text: text.into(),
            start,
            end: Some(end),
        }
    }

    #[test]
    fn test_elrc_output() {
        let lines = vec![Line {
            text: "hello world".into(),
            start: Some(1000),
            end: Some(3000),
            words: Some(vec![word("hello", 1000, 2000), word("world", 2000, 3000)]),
        }];
        assert_eq!(
            build_elrc(&lines),
            "[00:01.00]<00:01.00>hello<00:02.00>world<00:03.00>"
        );
    }

    #[test]
    fn test_elrc_ends_on_the_last_word_not_on_the_line() {
        let lines = vec![Line {
            text: "Yeah".into(),
            start: Some(14075),
            end: Some(27672),
            words: Some(vec![word("Yeah", 14075, 14252)]),
        }];
        assert_eq!(build_elrc(&lines), "[00:14.08]<00:14.08>Yeah<00:14.25>");
    }

    #[test]
    fn test_elrc_falls_back_to_the_line_end_when_the_last_word_has_none() {
        let lines = vec![Line {
            text: "hello".into(),
            start: Some(1000),
            end: Some(3000),
            words: Some(vec![Word {
                text: "hello".into(),
                start: 1000,
                end: None,
            }]),
        }];
        assert_eq!(build_elrc(&lines), "[00:01.00]<00:01.00>hello<00:03.00>");
    }

    #[test]
    fn test_elrc_falls_back_to_the_next_word_before_the_line_end() {
        let lines = vec![Line {
            text: "hello world".into(),
            start: Some(1000),
            end: Some(9000),
            words: Some(vec![
                Word {
                    text: "hello ".into(),
                    start: 1000,
                    end: None,
                },
                word("world", 2000, 3000),
            ]),
        }];
        assert_eq!(
            build_elrc(&lines),
            "[00:01.00]<00:01.00>hello <00:02.00>world<00:03.00>"
        );
    }

    #[test]
    fn test_lrc_output() {
        let lines = vec![
            Line {
                text: "first".into(),
                start: Some(1000),
                end: None,
                words: None,
            },
            Line {
                text: "second".into(),
                start: Some(2500),
                end: None,
                words: None,
            },
        ];
        assert_eq!(build_lrc(&lines), "[00:01.00] first\n[00:02.50] second");
    }

    #[test]
    fn test_build_is_empty_when_no_line_has_timings() {
        let lines = vec![Line {
            text: "plain".into(),
            start: None,
            end: None,
            words: None,
        }];
        assert!(build_elrc(&lines).is_empty());
        assert!(build_lrc(&lines).is_empty());
    }
}
