use std::borrow::Cow;

use crate::{
    config::PluginConfig,
    format::{self, lrc},
};

#[derive(PartialEq, Debug)]
pub enum Lyrics {
    Synced(String),
    Plain(String),
    Instrumental,
}

impl Lyrics {
    pub fn kind(&self) -> LyricsKind {
        match self {
            Lyrics::Synced(_) => LyricsKind::Synced,
            Lyrics::Plain(_) => LyricsKind::Plain,
            Lyrics::Instrumental => LyricsKind::Instrumental,
        }
    }

    pub fn text<'a>(&'a self, cfg: &'a PluginConfig) -> Cow<'a, str> {
        match self {
            Lyrics::Synced(s) => Cow::Borrowed(s),
            Lyrics::Plain(s) => Cow::Borrowed(s),
            Lyrics::Instrumental => Cow::Borrowed(&cfg.instrumental_text),
        }
    }

    pub fn sanitize(&mut self, cfg: &PluginConfig) {
        match self {
            Lyrics::Synced(s) => {
                *s = lrc::sanitize(s);
                if cfg.strip_section_labels {
                    *s = format::strip_section_labels(s)
                }
            }
            Lyrics::Plain(s) => {
                if cfg.strip_section_labels {
                    *s = format::strip_section_labels(s)
                }
            }
            Lyrics::Instrumental => {}
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Lyrics::Synced(s) | Lyrics::Plain(s) => s.trim().is_empty(),
            Lyrics::Instrumental => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LyricsKind {
    Synced,
    Plain,
    Instrumental,
}
