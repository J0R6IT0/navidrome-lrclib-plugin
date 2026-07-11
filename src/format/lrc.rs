use std::borrow::Cow;

const KEEP_TAGS: &[&str] = &["offset"];

const CREDIT_PREFIXES: &[&str] = &[
    "Lyrics by",
    "Composed by",
    "Produced by",
    "Published by",
    "Vocals by",
    "Background Vocals by",
    "Additional Vocal by",
    "Mixing Engineer",
    "Mastered by",
    "Executive Producer",
    "Vocal Engineer",
    "Vocals Produced by",
    "Recorded at",
    "Repertoire Owner",
    "Written by",
    "Arranged by",
    "Music by",
    "Words by",
    "Lyrics",
    "Composer",
    "Lyricist",
    "Producers",
    "Writers",
    "Arranger",
    "Translator",
    "Adapted by",
    "作词",
    "作曲",
    "编曲",
    "制作人",
    "录音",
    "混音",
    "母带",
    "出品人",
    "翻译",
];

/// A leading "Artist - Title" header is only stripped when its timestamp is
/// within this many seconds of the start.
const TITLE_HEADER_MAX_SECS: f64 = 5.0;

const INSTRUMENTAL_MARKERS: &[&str] = &["instrumental", "纯音乐", "no lyrics"];

/// More than this many timed lines means the track has real lyrics, not just an
/// instrumental marker.
const MAX_INSTRUMENTAL_TIMED_LINES: usize = 3;

/// A blank (timestamp-only) line is kept only when the gap to the next line is
/// at least this long. Shorter gaps are provider noise, not a real instrumental
/// pause, and a long enough gap can also let a stripped section label leave one
/// blank line behind.
pub(crate) const BLANK_GAP_MIN_SECS: f64 = 5.0;

pub fn sanitize(lrc: &str) -> String {
    let mut first_time_tag_seen = false;

    let kept: Vec<&str> = lrc
        .lines()
        .filter(|line| keep_line(line, &mut first_time_tag_seen))
        .collect();

    let mut out: Vec<&str> = Vec::with_capacity(kept.len());
    for (i, &line) in kept.iter().enumerate() {
        if is_blank_timed_line(line) {
            let next = kept.get(i + 1).and_then(|l| time_tag_secs(l));
            let long_gap = matches!(
                (time_tag_secs(line), next),
                (Some(start), Some(end)) if end - start >= BLANK_GAP_MIN_SECS
            );
            if !long_gap {
                continue;
            }
        }
        out.push(line);
    }

    out.join("\n")
}

pub fn is_instrumental(lyrics: &str) -> bool {
    let timed_lines = lyrics
        .lines()
        .filter(|line| matches!(parse_line(line), Some((Some(_), _))))
        .count();

    if timed_lines > MAX_INSTRUMENTAL_TIMED_LINES {
        return false;
    }

    let lower = lyrics.to_lowercase();

    INSTRUMENTAL_MARKERS.iter().any(|m| lower.contains(m))
}

pub fn is_synced(lyrics: &str) -> bool {
    lyrics
        .lines()
        .any(|line| matches!(parse_line(line), Some((Some(_), _))))
}

pub(crate) fn time_tag_secs(line: &str) -> Option<f64> {
    match parse_line(line) {
        Some((Some(secs), _)) => Some(secs),
        _ => None,
    }
}

pub(crate) fn is_blank_timed_line(line: &str) -> bool {
    matches!(
        parse_line(line),
        Some((Some(_), text)) if strip_word_tags(text).trim().is_empty()
    )
}

fn keep_line(line: &str, first_time_tag_seen: &mut bool) -> bool {
    let trimmed = line.trim_start_matches('\u{feff}').trim();

    let lyric_text = match parse_line(trimmed) {
        Some((Some(secs), text)) => {
            if !*first_time_tag_seen {
                *first_time_tag_seen = true;
                if secs < TITLE_HEADER_MAX_SECS && is_title_header(text) {
                    return false;
                }
            }
            text
        }
        Some((None, _)) => {
            if is_droppable_metadata(trimmed) {
                return false;
            }
            return true;
        }
        None => trimmed,
    };

    !is_credit_line(lyric_text)
}

fn is_title_header(text: &str) -> bool {
    let plain = strip_word_tags(text);
    plain.contains(" - ") && plain.chars().any(|c| c.is_alphabetic())
}

fn is_droppable_metadata(line: &str) -> bool {
    let content = line
        .strip_prefix('[')
        .and_then(|rest| rest.split(']').next())
        .unwrap_or_default();

    match content.split_once(':') {
        Some((key, _)) => !KEEP_TAGS.contains(&key),
        None => true,
    }
}

fn is_credit_line(text: &str) -> bool {
    let plain = strip_word_tags(text);
    let plain = plain.trim_start();

    CREDIT_PREFIXES.iter().any(|prefix| {
        plain
            .get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
            && {
                let rest = plain[prefix.len()..].trim_start_matches(' ');
                rest.starts_with(':') || rest.starts_with('：')
            }
    })
}

fn strip_word_tags(text: &str) -> Cow<'_, str> {
    if !text.contains('<') {
        return Cow::Borrowed(text);
    }

    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(open) = rest.find('<') {
        let after = &rest[open + 1..];
        let is_tag = after.starts_with(|c: char| c.is_ascii_digit());

        if let Some(close) = is_tag.then(|| after.find('>')).flatten() {
            out.push_str(&rest[..open]);
            rest = &after[close + 1..];
        } else {
            out.push_str(&rest[..=open]);
            rest = &rest[open + 1..];
        }
    }

    out.push_str(rest);
    Cow::Owned(out)
}

fn parse_line(line: &str) -> Option<(Option<f64>, &str)> {
    let trimmed = line.trim_start_matches('\u{feff}').trim();

    let rest = trimmed.strip_prefix('[')?;
    let bracket_end = rest.find(']')?;

    let content = &rest[..bracket_end];
    let text = &rest[bracket_end + 1..];

    let time_tag_secs = content.split_once(':').and_then(|(left, right)| {
        if left.chars().all(|c| c.is_ascii_digit())
            && right.contains('.')
            && right.chars().all(|c| c.is_ascii_digit() || c == '.')
        {
            let mins = left.parse::<f64>().ok()?;
            let secs = right.parse::<f64>().ok()?;
            Some(mins * 60.0 + secs)
        } else {
            None
        }
    });

    Some((time_tag_secs, text))
}

#[cfg(test)]
mod tests {
    use super::is_instrumental;
    use super::is_synced;
    use super::sanitize;
    use super::strip_word_tags;

    mod strip_word_tags_tests {
        use super::strip_word_tags;
        use std::borrow::Cow;

        #[test]
        fn test_no_tags_borrows_input() {
            assert!(matches!(strip_word_tags("plain text"), Cow::Borrowed(_)));
        }

        #[test]
        fn test_strips_word_timing_tags() {
            assert_eq!(
                strip_word_tags("<00:00.00>Hello <00:00.50>world"),
                "Hello world"
            );
        }

        #[test]
        fn test_keeps_non_digit_angle_brackets() {
            assert_eq!(strip_word_tags("a <b> c"), "a <b> c");
        }

        #[test]
        fn test_unclosed_digit_bracket_is_kept() {
            assert_eq!(strip_word_tags("I <3 it"), "I <3 it");
        }

        #[test]
        fn test_tag_at_end_without_close_is_kept() {
            assert_eq!(strip_word_tags("done <12:34"), "done <12:34");
        }
    }

    mod sanitize_tests {
        use super::sanitize;

        #[test]
        fn test_metadata_tags_stripped() {
            let input = "[ar:Artist Name]\n[al:Album]\n[ti:Song Title]\n[00:10.00] Hello";
            assert_eq!(sanitize(input), "[00:10.00] Hello");
        }

        #[test]
        fn test_offset_tag_kept() {
            let input = "[offset:-500]\n[ar:Artist]\n[00:10.00] Hello";
            assert_eq!(sanitize(input), "[offset:-500]\n[00:10.00] Hello");
        }

        #[test]
        fn test_standalone_bracket_without_colon_stripped() {
            let input = "[SomethingWithoutColon]\n[00:10.00] Hello";
            assert_eq!(sanitize(input), "[00:10.00] Hello");
        }

        #[test]
        fn test_regular_lyric_lines_kept() {
            let input = "[00:10.00] Hello world\n[00:15.00] Foo bar";
            assert_eq!(
                sanitize(input),
                "[00:10.00] Hello world\n[00:15.00] Foo bar"
            );
        }

        #[test]
        fn test_empty_input() {
            assert_eq!(sanitize(""), "");
        }

        #[test]
        fn test_short_gap_blank_line_dropped() {
            let input = "[00:44.95]A song in every breath\n[00:47.60]\n[00:48.36]Sing me";
            assert_eq!(
                sanitize(input),
                "[00:44.95]A song in every breath\n[00:48.36]Sing me"
            );
        }

        #[test]
        fn test_long_gap_blank_line_kept() {
            let input = "[00:44.95]A song\n[00:47.60]\n[00:55.00]Sing me";
            assert_eq!(
                sanitize(input),
                "[00:44.95]A song\n[00:47.60]\n[00:55.00]Sing me"
            );
        }

        #[test]
        fn test_trailing_blank_line_dropped() {
            let input = "[00:44.95]Last line\n[00:47.60]";
            assert_eq!(sanitize(input), "[00:44.95]Last line");
        }

        #[test]
        fn test_first_time_tag_title_pattern_at_under_5s_is_filtered() {
            let input = "[00:01.00] Artist - Title\n[00:05.00] First verse";
            assert_eq!(sanitize(input), "[00:05.00] First verse");
        }

        #[test]
        fn test_first_time_tag_no_dash_is_kept() {
            let input = "[00:01.00] First verse";
            assert_eq!(sanitize(input), "[00:01.00] First verse");
        }

        #[test]
        fn test_first_time_tag_title_pattern_at_exactly_5s_is_kept() {
            let input = "[00:05.00] Artist - Title\n[00:10.00] Hello";
            assert_eq!(
                sanitize(input),
                "[00:05.00] Artist - Title\n[00:10.00] Hello"
            );
        }

        #[test]
        fn test_second_time_tag_title_pattern_is_kept() {
            let input = "[00:01.00] First verse\n[00:03.00] Artist - Title";
            assert_eq!(
                sanitize(input),
                "[00:01.00] First verse\n[00:03.00] Artist - Title"
            );
        }

        #[test]
        fn test_non_tagged_credit_line_filtered() {
            let input = "Lyrics by: Someone\n[00:05.00] Hello";
            assert_eq!(sanitize(input), "[00:05.00] Hello");
        }

        #[test]
        fn test_time_tagged_credit_line_filtered() {
            let input = "[00:01.00] Lyrics by: Someone\n[00:05.00] Hello";
            assert_eq!(sanitize(input), "[00:05.00] Hello");
        }

        #[test]
        fn test_credit_with_fullwidth_colon_filtered() {
            let input = "Written by\u{FF1A} Someone\n[00:05.00] Hello";
            assert_eq!(sanitize(input), "[00:05.00] Hello");
        }

        #[test]
        fn test_multiple_credits_filtered() {
            let input =
                "[00:01.00] Music by: Composer\n[00:02.00] Arranged by: Arranger\n[00:10.00] Verse";
            assert_eq!(sanitize(input), "[00:10.00] Verse");
        }

        #[test]
        fn test_credit_prefix_case_insensitive() {
            let input = "LYRICS BY: Someone\n[00:05.00] Hello";
            assert_eq!(sanitize(input), "[00:05.00] Hello");
        }

        #[test]
        fn test_chinese_credit_lines_filtered() {
            let input = "[00:00.000] 作词 : Freddie Mercury\n[00:00.000] 作曲 : Freddie Mercury\n[00:06.600]He's a fairy feller";
            assert_eq!(sanitize(input), "[00:06.600]He's a fairy feller");
        }

        #[test]
        fn test_chinese_credit_no_space_before_colon_filtered() {
            let input = "[00:00.000]作词:Freddie Mercury\n[00:01.000]作曲:Freddie Mercury\n[00:06.600]He's a fairy feller";
            assert_eq!(sanitize(input), "[00:06.600]He's a fairy feller");
        }

        #[test]
        fn test_chinese_arranger_credit_filtered() {
            let input = "[00:02.000]编曲 : Queen\n[00:06.600]He's a fairy feller";
            assert_eq!(sanitize(input), "[00:06.600]He's a fairy feller");
        }

        #[test]
        fn test_netease_credit_block_filtered() {
            // NetEase preamble example
            let input = concat!(
                "[00:00.000] 作词 : Freddie Mercury\n",
                "[00:00.000] 作曲 : Freddie Mercury\n",
                "[00:01.000]作曲 : Freddie Mercury\n",
                "[00:02.000]编曲 : Queen\n",
                "[00:06.600]He's a fairy feller\n",
                "[00:20.860]Ah ah the fairy folk have gathered\n",
                "[00:22.590]Round the new moon's shine",
            );
            assert_eq!(
                sanitize(input),
                "[00:06.600]He's a fairy feller\n[00:20.860]Ah ah the fairy folk have gathered\n[00:22.590]Round the new moon's shine"
            );
        }

        #[test]
        fn test_enhanced_lrc_title_line_filtered() {
            let input = concat!(
                "[00:00.00]<00:00.00>Artist <00:00.50>- <00:01.00>Title\n",
                "[00:06.00]<00:06.00>First <00:06.50>line"
            );
            assert_eq!(sanitize(input), "[00:06.00]<00:06.00>First <00:06.50>line");
        }

        #[test]
        fn test_enhanced_lrc_credit_line_filtered() {
            let input = concat!(
                "[00:01.00]<00:01.00>Composed <00:01.50>by<00:02.00>: <00:02.50>Someone\n",
                "[00:06.00]<00:06.00>Hello"
            );
            assert_eq!(sanitize(input), "[00:06.00]<00:06.00>Hello");
        }

        #[test]
        fn test_enhanced_lrc_word_tags_preserved_in_output() {
            let input = "[00:06.00]<00:06.00>Hello <00:06.50>world";
            assert_eq!(sanitize(input), "[00:06.00]<00:06.00>Hello <00:06.50>world");
        }

        #[test]
        fn test_space_before_colon_english_credit_filtered() {
            let input = "Written by : Someone\n[00:05.00] Hello";
            assert_eq!(sanitize(input), "[00:05.00] Hello");

            let input = concat!(
                "[ar:Artist]\n",
                "[al:Album]\n",
                "[offset:500]\n",
                "[00:00.50] Artist - Title\n",
                "[00:10.00] Lyrics by: Someone\n",
                "[00:15.00] Hello\n",
                "[00:20.00] World"
            );
            assert_eq!(
                sanitize(input),
                "[offset:500]\n[00:15.00] Hello\n[00:20.00] World"
            );
        }
    }

    mod is_instrumental_tests {
        use super::is_instrumental;

        #[test]
        fn test_detects_plain_instrumental() {
            assert!(is_instrumental("Instrumental"));
        }

        #[test]
        fn test_detects_chinese_instrumental() {
            assert!(is_instrumental("[00:01.00]纯音乐，请欣赏"));
        }

        #[test]
        fn test_detects_no_lyrics() {
            assert!(is_instrumental("[00:00.00]No Lyrics"));
        }

        #[test]
        fn test_metadata_lines_do_not_count() {
            let input = concat!(
                "[ar:test]\n",
                "[ti:test]\n",
                "[offset:0]\n",
                "[00:01.00]纯音乐，请欣赏\n"
            );

            assert!(is_instrumental(input));
        }

        #[test]
        fn test_more_than_three_timed_lines_is_not_instrumental() {
            let input = concat!(
                "[00:01.00]纯音乐，请欣赏\n",
                "[00:02.00]...\n",
                "[00:03.00]...\n",
                "[00:04.00]...\n"
            );

            assert!(!is_instrumental(input));
        }

        #[test]
        fn test_non_timed_lines_do_not_count() {
            let input = concat!("hello\n", "world\n", "[00:01.00]instrumental\n");

            assert!(is_instrumental(input));
        }

        #[test]
        fn test_bom_is_handled() {
            let input = "\u{feff}[00:01.00]纯音乐";

            assert!(is_instrumental(input));
        }

        #[test]
        fn test_regular_lyrics_not_detected() {
            let input = concat!("[00:01.00]Hello\n", "[00:05.00]World\n");

            assert!(!is_instrumental(input));
        }

        #[test]
        fn test_offset_tag_is_not_counted_as_timed_line() {
            let input = concat!("[offset:0]\n", "[00:01.00]instrumental\n");

            assert!(is_instrumental(input));
        }
    }

    mod is_synced_tests {
        use super::is_synced;

        #[test]
        fn test_real_synced_lrc_is_synced() {
            let input = concat!("[00:01.00]Hello\n", "[00:05.00]World\n");

            assert!(is_synced(input));
        }

        #[test]
        fn test_metadata_tags_before_timed_line_is_synced() {
            let input = concat!(
                "[ar:Artist]\n",
                "[al:Album]\n",
                "[ti:Title]\n",
                "[00:01.00]Hello\n"
            );

            assert!(is_synced(input));
        }

        #[test]
        fn test_plain_multiline_text_is_not_synced() {
            let input = "Hello\nWorld\n";

            assert!(!is_synced(input));
        }

        #[test]
        fn test_bracketed_non_time_label_is_not_synced() {
            let input = "[Verse 1]\nHello\nWorld\n";

            assert!(!is_synced(input));
        }

        #[test]
        fn test_empty_input_is_not_synced() {
            assert!(!is_synced(""));
        }

        #[test]
        fn test_bom_is_handled() {
            let input = "\u{feff}[00:01.00]Hello";

            assert!(is_synced(input));
        }
    }
}
