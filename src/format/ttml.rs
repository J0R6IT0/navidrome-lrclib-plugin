use crate::types::SyncLevel;
use regex::Regex;

pub fn sync_level(doc: &str) -> SyncLevel {
    if let Some(level) = itunes_timing(doc) {
        return level;
    }

    if is_match(doc, r#"(?i)<span\b[^>]*\bbegin\s*=\s*["']"#) {
        SyncLevel::Word
    } else if is_match(doc, r#"(?i)<p\b[^>]*\bbegin\s*=\s*["']"#) {
        SyncLevel::Line
    } else {
        SyncLevel::Plain
    }
}

fn itunes_timing(doc: &str) -> Option<SyncLevel> {
    let re = Regex::new(r#"(?i)\bitunes:timing\s*=\s*["']\s*([a-z]+)\s*["']"#).unwrap();
    let value = re.captures(doc)?.get(1)?.as_str();

    match value.to_ascii_lowercase().as_str() {
        "word" => Some(SyncLevel::Word),
        "line" => Some(SyncLevel::Line),
        "none" => Some(SyncLevel::Plain),
        _ => None,
    }
}

fn is_match(doc: &str, pattern: &str) -> bool {
    Regex::new(pattern).unwrap().is_match(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_itunes_timing_word() {
        let doc = r#"<tt xmlns:itunes="http://music.apple.com/lyric-ttml-internal" itunes:timing="Word" xml:lang="en"><body></body></tt>"#;
        assert_eq!(sync_level(doc), SyncLevel::Word);
    }

    #[test]
    fn test_itunes_timing_line() {
        let doc = r#"<tt itunes:timing="Line"><body><div><p>Hello</p></div></body></tt>"#;
        assert_eq!(sync_level(doc), SyncLevel::Line);
    }

    #[test]
    fn test_itunes_timing_none_is_plain() {
        let doc = r#"<tt itunes:timing="None"><body><div><p>Hello</p></div></body></tt>"#;
        assert_eq!(sync_level(doc), SyncLevel::Plain);
    }

    #[test]
    fn test_itunes_timing_is_case_insensitive() {
        let doc = r#"<tt ITUNES:TIMING='word'></tt>"#;
        assert_eq!(sync_level(doc), SyncLevel::Word);
    }

    #[test]
    fn test_itunes_timing_wins_over_structure() {
        let doc = concat!(
            r#"<tt itunes:timing="Line"><body><div>"#,
            r#"<p begin="1.0" end="2.0"><span>Hello</span></p>"#,
            r#"</div></body></tt>"#
        );
        assert_eq!(sync_level(doc), SyncLevel::Line);
    }

    #[test]
    fn test_unknown_itunes_timing_falls_back_to_structure() {
        let doc = r#"<tt itunes:timing="Bogus"><body><div><p begin="1.0">Hi</p></div></body></tt>"#;
        assert_eq!(sync_level(doc), SyncLevel::Line);
    }

    #[test]
    fn test_structural_word_without_timing_attribute() {
        let doc = concat!(
            r#"<tt xmlns:itunes="http://music.apple.com/lyrics"><body><div>"#,
            r#"<p begin="00:00:11.011" end="00:00:13.643">"#,
            r#"<span begin="00:00:11.011" end="00:00:11.397">Say </span>"#,
            r#"</p></div></body></tt>"#
        );
        assert_eq!(sync_level(doc), SyncLevel::Word);
    }

    #[test]
    fn test_structural_line_without_timing_attribute() {
        let doc =
            r#"<tt><body><div><p begin="45.452" end="49.551">Sleep well</p></div></body></tt>"#;
        assert_eq!(sync_level(doc), SyncLevel::Line);
    }

    #[test]
    fn test_structural_plain_without_timing_attribute() {
        let doc = r#"<tt><body><div><p>I don't know what to do</p></div></body></tt>"#;
        assert_eq!(sync_level(doc), SyncLevel::Plain);
    }

    #[test]
    fn test_untimed_translation_spans_do_not_count_as_word() {
        let doc = concat!(
            r#"<tt><body><div><p begin="1.0" end="2.0">Hello"#,
            r#"<span ttm:role="x-translation" xml:lang="es">Hola</span>"#,
            r#"</p></div></body></tt>"#
        );
        assert_eq!(sync_level(doc), SyncLevel::Line);
    }

    #[test]
    fn test_single_quoted_and_spaced_attributes() {
        let doc = "<tt><body><div><p begin = '1.0'>Hi</p></div></body></tt>";
        assert_eq!(sync_level(doc), SyncLevel::Line);
    }

    #[test]
    fn test_empty_document_is_plain() {
        assert_eq!(sync_level(""), SyncLevel::Plain);
    }
}
