pub fn sanitize(lrc: &str) -> String {
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

    let mut first_time_tag_processed = false;

    lrc.lines()
        .filter(|line| {
            let trimmed = line.trim_start_matches('\u{feff}').trim();
            let mut text = trimmed;

            if let Some((time_tag_secs, parsed_text)) = parse_line(trimmed) {
                text = parsed_text;

                if let Some(total_secs) = time_tag_secs {
                    if !first_time_tag_processed {
                        first_time_tag_processed = true;

                        let has_title_dash = text.contains(" - ");
                        let has_letters = text.chars().any(|c| c.is_alphabetic());

                        if total_secs < 5.0 && has_title_dash && has_letters {
                            return false;
                        }
                    }
                } else if let Some(rest) = trimmed.strip_prefix('[')
                    && let Some(bracket_end) = rest.find(']')
                {
                    let content = &rest[..bracket_end];

                    if let Some((key, _)) = content.split_once(':') {
                        if !KEEP_TAGS.contains(&key) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
            }

            let text = text.trim_start();

            for prefix in CREDIT_PREFIXES {
                if let Some(head) = text.get(..prefix.len()) {
                    let rest = &text[prefix.len()..];

                    let rest_trimmed = rest.trim_start_matches(' ');
                    if head.eq_ignore_ascii_case(prefix)
                        && (rest_trimmed.starts_with(':') || rest_trimmed.starts_with('：'))
                    {
                        return false;
                    }
                }
            }

            true
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn is_instrumental(lyrics: &str) -> bool {
    const INSTRUMENTAL_MARKERS: &[&str] = &["instrumental", "纯音乐", "no lyrics"];

    let timed_lines = lyrics
        .lines()
        .filter(|line| matches!(parse_line(line), Some((Some(_), _))))
        .count();

    if timed_lines > 3 {
        return false;
    }

    let lower = lyrics.to_lowercase();

    INSTRUMENTAL_MARKERS.iter().any(|m| lower.contains(m))
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
    use super::sanitize;

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
}
