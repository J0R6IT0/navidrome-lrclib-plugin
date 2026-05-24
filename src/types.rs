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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PluginConfig;

    fn synced(s: &str) -> Lyrics {
        Lyrics::Synced(s.to_string())
    }

    fn plain(s: &str) -> Lyrics {
        Lyrics::Plain(s.to_string())
    }

    #[test]
    fn test_sanitize_synced_strips_metadata_and_section_labels() {
        // sanitize_lrc removes [ar:...] metadata; strip_section_labels removes [Chorus].
        let mut lyrics = synced("[ar:Artist]\n[00:10.00] Hello [Chorus]\n[00:15.00] World");
        lyrics.sanitize(&PluginConfig::default());
        assert_eq!(lyrics, synced("[00:10.00] Hello\n[00:15.00] World"));
    }

    #[test]
    fn test_sanitize_synced_always_strips_section_labels_regardless_of_config() {
        // Synced lyrics always have section labels stripped, even when the config flag is false.
        let mut lyrics = synced("[00:10.00] [Chorus] We will rock you");
        lyrics.sanitize(&PluginConfig {
            strip_section_labels: false,
            ..Default::default()
        });
        assert_eq!(lyrics, synced("[00:10.00] We will rock you"));
    }

    #[test]
    fn test_sanitize_plain_strips_section_labels_when_configured() {
        let mut lyrics = plain("[Verse 1]\nHello there\n[Chorus]\nWe will rock you");
        lyrics.sanitize(&PluginConfig {
            strip_section_labels: true,
            ..Default::default()
        });
        assert_eq!(lyrics, plain("Hello there\nWe will rock you"));
    }

    #[test]
    fn test_sanitize_plain_preserves_section_labels_when_not_configured() {
        let input = "[Verse 1]\nHello there\n[Chorus]\nWe will rock you";
        let mut lyrics = plain(input);
        lyrics.sanitize(&PluginConfig {
            strip_section_labels: false,
            ..Default::default()
        });
        assert_eq!(lyrics, plain(input));
    }

    #[test]
    fn test_sanitize_instrumental_is_noop() {
        let mut lyrics = Lyrics::Instrumental;
        lyrics.sanitize(&PluginConfig::default());
        assert_eq!(lyrics, Lyrics::Instrumental);
    }

    #[test]
    fn test_text_synced() {
        assert_eq!(synced("hello").text(), "hello");
    }

    #[test]
    fn test_text_plain() {
        assert_eq!(plain("hello").text(), "hello");
    }

    #[test]
    fn test_text_instrumental() {
        assert_eq!(Lyrics::Instrumental.text(), "Instrumental");
    }

    #[test]
    fn test_kind() {
        assert_eq!(synced("").kind(), LyricsKind::Synced);
        assert_eq!(plain("").kind(), LyricsKind::Plain);
        assert_eq!(Lyrics::Instrumental.kind(), LyricsKind::Instrumental);
    }
}
