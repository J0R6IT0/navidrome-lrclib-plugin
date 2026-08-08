use crate::{
    config::{PluginConfig, ProviderParams},
    ext::TrackInfoExt,
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

pub struct LrcMux {
    base_url: String,
}

impl LrcMux {
    pub fn create(params: &ProviderParams) -> Box<dyn LyricsProvider> {
        Box::new(Self {
            base_url: params
                .get("baseUrl")
                .unwrap_or(DEFAULT_BASE_URL)
                .to_string(),
        })
    }
}

impl LyricsProvider for LrcMux {
    fn supported_kinds(&self) -> &'static [LyricsKind] {
        &[LyricsKind::Elrc, LyricsKind::Lrc, LyricsKind::Plain]
    }

    fn log_params(&self) -> Vec<(&'static str, String)> {
        vec![("baseUrl", self.base_url.clone())]
    }

    fn fetch_lyrics(&self, track: &TrackInfo, cfg: &PluginConfig) -> Result<Option<Lyrics>, Error> {
        let first_artist = track
            .first_artist()
            .ok_or_else(|| Error::new("missing artist"))?;

        let duration = track.duration.round() as i64;

        let qs = serde_urlencoded::to_string([
            ("artist", first_artist),
            ("title", track.title.as_str()),
            ("album", track.album.as_str()),
            ("duration", &duration.to_string()),
        ])
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

/// Appends whitespace-only tokens to the word before them, dropping their own
/// timestamp so the next word's start closes the merged one.
///
/// Responses from Musixmatch contain whitespaces as standalone words, and those
/// separators hold most of the duration, so it looks like words are instantly
/// highlighted in clients like Feishin. Ideally this should be fixed upstream,
/// but this implementation should still work fine even if it gets fixed.
fn merge_separators(words: &[Word]) -> Vec<Word> {
    let mut out: Vec<Word> = Vec::with_capacity(words.len());
    for w in words {
        if w.text.trim().is_empty()
            && let Some(prev) = out.last_mut()
        {
            prev.text.push_str(&w.text);
            prev.end = w.end.or(prev.end);
            continue;
        }
        out.push(Word {
            text: w.text.clone(),
            start: w.start,
            end: w.end,
        });
    }
    out
}

fn build_elrc(lines: &[Line]) -> String {
    lines
        .iter()
        .filter_map(|l| {
            let start = l.start?;
            let words = merge_separators(l.words.as_deref()?);
            let end = words.last()?.end.or(l.end)?;

            let mut buf = format!("[{}]", ms_to_ts(start));
            for w in &words {
                buf.push_str(&format!("<{}>{}", ms_to_ts(w.start), w.text));
            }
            buf.push_str(&format!("<{}>", ms_to_ts(end)));
            Some(buf)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_lrc(lines: &[Line]) -> String {
    lines
        .iter()
        .filter_map(|l| Some(format!("[{}] {}", ms_to_ts(l.start?), l.text)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn ms_to_ts(ms: i64) -> String {
    let ms = ms.max(0);
    let total_cs = ms / 10;
    let cs = total_cs % 100;
    let total_secs = total_cs / 100;
    let secs = total_secs % 60;
    let mins = total_secs / 60;
    format!("{mins:02}:{secs:02}.{cs:02}")
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
    fn test_elrc_folds_separator_tokens_into_the_preceding_word() {
        let lines = vec![Line {
            text: "Looks like blue".into(),
            start: Some(42350),
            end: Some(44081),
            words: Some(vec![
                word("Looks", 42350, 42362),
                word(" ", 42362, 42726),
                word("like", 42726, 42746),
                word(" ", 42746, 42969),
                word("blue", 42969, 42983),
            ]),
        }];
        assert_eq!(
            build_elrc(&lines),
            "[00:42.35]<00:42.35>Looks <00:42.72>like <00:42.96>blue<00:42.98>"
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
        assert_eq!(build_elrc(&lines), "[00:14.07]<00:14.07>Yeah<00:14.25>");
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
    fn test_elrc_extends_the_last_word_through_a_trailing_separator() {
        let lines = vec![Line {
            text: "hello".into(),
            start: Some(1000),
            end: Some(9000),
            words: Some(vec![word("hello", 1000, 2000), word(" ", 2000, 2500)]),
        }];
        assert_eq!(build_elrc(&lines), "[00:01.00]<00:01.00>hello <00:02.50>");
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
