use crate::{
    config::PluginConfig,
    format::{self, lrc},
};
use nd_pdk::lyrics::{GetLyricsResponse, LyricsText};
use std::borrow::Cow;

#[derive(PartialEq, Debug)]
pub enum Lyrics {
    Plain(String),
    Lrc(String),
    Elrc(String),
    Ttml(String),
    Srt(String),
    Lyricsfile(String),
    Instrumental,
}

impl Lyrics {
    pub fn kind(&self) -> LyricsKind {
        match self {
            Lyrics::Plain(_) => LyricsKind::Plain,
            Lyrics::Lrc(_) => LyricsKind::Lrc,
            Lyrics::Elrc(_) => LyricsKind::Elrc,
            Lyrics::Ttml(_) => LyricsKind::Ttml,
            Lyrics::Srt(_) => LyricsKind::Srt,
            Lyrics::Lyricsfile(_) => LyricsKind::Lyricsfile,
            Lyrics::Instrumental => LyricsKind::Instrumental,
        }
    }

    pub fn text<'a>(&'a self, cfg: &'a PluginConfig) -> Cow<'a, str> {
        match self {
            Lyrics::Plain(s)
            | Lyrics::Lrc(s)
            | Lyrics::Elrc(s)
            | Lyrics::Ttml(s)
            | Lyrics::Srt(s)
            | Lyrics::Lyricsfile(s) => Cow::Borrowed(s),
            Lyrics::Instrumental => Cow::Borrowed(&cfg.instrumental_text),
        }
    }

    pub fn sanitize(&mut self, cfg: &PluginConfig) {
        match self {
            Lyrics::Lrc(s) | Lyrics::Elrc(s) => {
                *s = lrc::sanitize(s);
                if cfg.strip_section_labels {
                    *s = format::strip_section_labels(s);
                }
            }
            Lyrics::Plain(s) => {
                if cfg.strip_section_labels {
                    *s = format::strip_section_labels(s);
                }
            }
            Lyrics::Ttml(_) | Lyrics::Srt(_) | Lyrics::Lyricsfile(_) | Lyrics::Instrumental => {}
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Lyrics::Plain(s)
            | Lyrics::Lrc(s)
            | Lyrics::Elrc(s)
            | Lyrics::Ttml(s)
            | Lyrics::Srt(s)
            | Lyrics::Lyricsfile(s) => s.trim().is_empty(),
            Lyrics::Instrumental => false,
        }
    }

    pub fn to_response(&self, cfg: &PluginConfig) -> GetLyricsResponse {
        GetLyricsResponse {
            lyrics: vec![LyricsText {
                lang: "xxx".into(),
                text: self.text(cfg).into_owned(),
            }],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LyricsKind {
    Plain,
    Lrc,
    Elrc,
    Ttml,
    Srt,
    Lyricsfile,
    Instrumental,
}

impl LyricsKind {
    pub fn slug(&self) -> &'static str {
        match self {
            LyricsKind::Plain => "plain",
            LyricsKind::Lrc => "lrc",
            LyricsKind::Elrc => "elrc",
            LyricsKind::Ttml => "ttml",
            LyricsKind::Srt => "srt",
            LyricsKind::Lyricsfile => "lyricsfile",
            LyricsKind::Instrumental => "instrumental",
        }
    }

    pub fn from_slug(slug: &str) -> Option<LyricsKind> {
        match slug.trim().to_ascii_lowercase().as_str() {
            "plain" => Some(LyricsKind::Plain),
            "lrc" => Some(LyricsKind::Lrc),
            "elrc" => Some(LyricsKind::Elrc),
            "ttml" => Some(LyricsKind::Ttml),
            "srt" => Some(LyricsKind::Srt),
            "lyricsfile" => Some(LyricsKind::Lyricsfile),
            "instrumental" => Some(LyricsKind::Instrumental),
            _ => None,
        }
    }
}
