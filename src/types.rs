use crate::{
    config::PluginConfig,
    sanitize::{sanitize_lrc, strip_section_labels},
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

    pub fn text(&self) -> &str {
        match self {
            Lyrics::Synced(s) => s,
            Lyrics::Plain(s) => s,
            Lyrics::Instrumental => "Instrumental",
        }
    }

    pub fn sanitize(&mut self, cfg: &PluginConfig) {
        match self {
            Lyrics::Synced(s) => {
                *s = sanitize_lrc(s);
                if cfg.strip_section_labels {
                    *s = strip_section_labels(s)
                }
            }
            Lyrics::Plain(s) => {
                if cfg.strip_section_labels {
                    *s = strip_section_labels(s)
                }
            }
            Lyrics::Instrumental => {}
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LyricsKind {
    Synced,
    Plain,
    Instrumental,
}
