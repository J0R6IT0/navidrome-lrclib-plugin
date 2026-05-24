use regex::Regex;

pub fn sanitize_lrc(lrc: &str) -> String {
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
    ];

    let mut first_time_tag_processed = false;

    lrc.lines()
        .filter(|line| {
            let trimmed = line.trim();
            let mut text = trimmed;

            if let Some(rest) = trimmed.strip_prefix('[')
                && let Some(bracket_end) = rest.find(']')
            {
                let content = &rest[..bracket_end];
                text = &rest[bracket_end + 1..];

                let time_tag_secs = content.split_once(':').and_then(|(left, right)| {
                    if left.chars().all(|c| c.is_ascii_digit())
                        && right.contains('.')
                        && right.chars().all(|c| c.is_ascii_digit() || c == '.')
                    {
                        let mins = left.parse::<f64>().unwrap_or(0.0);
                        let secs = right.parse::<f64>().unwrap_or(0.0);
                        Some(mins * 60.0 + secs)
                    } else {
                        None
                    }
                });

                if let Some(total_secs) = time_tag_secs {
                    if !first_time_tag_processed {
                        first_time_tag_processed = true;

                        let has_title_dash = text.contains(" - ");
                        let has_letters = text.chars().any(|c| c.is_alphabetic());

                        if total_secs < 5.0 && has_title_dash && has_letters {
                            return false;
                        }
                    }
                } else if let Some((key, _)) = content.split_once(':') {
                    if !KEEP_TAGS.contains(&key) {
                        return false;
                    }
                } else {
                    return false;
                }
            }

            let text = text.trim_start();
            for prefix in CREDIT_PREFIXES {
                if let Some(head) = text.get(..prefix.len()) {
                    let rest = &text[prefix.len()..];

                    if head.eq_ignore_ascii_case(prefix)
                        && (rest.starts_with(':') || rest.starts_with('：'))
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

pub fn strip_section_labels(lyrics: &str) -> String {
    let re = Regex::new(
         r"(?i)\[(?:verse|pre[- ]?verse|chorus|pre[- ]?chorus|post[- ]?chorus|bridge|hook|refrain|intro|outro|coda|interlude|instrumental|breakdown|solo|drop|chant|skit|ad-?lib|overture|finale|couplet)\b[^\]]*\]"
     ).unwrap();

    lyrics
        .lines()
        .map(|line| {
            let stripped = re.replace_all(line, "").to_string();

            let cleaned = stripped.replace("  ", " ");

            if line.trim_start().starts_with('[') {
                cleaned.trim().to_string()
            } else {
                cleaned.trim_end().to_string()
            }
        })
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::sanitize_lrc;
    use super::strip_section_labels;

    mod sanitize_lrc_tests {
        use super::sanitize_lrc;

        #[test]
        fn test_metadata_tags_stripped() {
            let input = "[ar:Artist Name]\n[al:Album]\n[ti:Song Title]\n[00:10.00] Hello";
            assert_eq!(sanitize_lrc(input), "[00:10.00] Hello");
        }

        #[test]
        fn test_offset_tag_kept() {
            let input = "[offset:-500]\n[ar:Artist]\n[00:10.00] Hello";
            assert_eq!(sanitize_lrc(input), "[offset:-500]\n[00:10.00] Hello");
        }

        #[test]
        fn test_standalone_bracket_without_colon_stripped() {
            let input = "[SomethingWithoutColon]\n[00:10.00] Hello";
            assert_eq!(sanitize_lrc(input), "[00:10.00] Hello");
        }

        #[test]
        fn test_regular_lyric_lines_kept() {
            let input = "[00:10.00] Hello world\n[00:15.00] Foo bar";
            assert_eq!(
                sanitize_lrc(input),
                "[00:10.00] Hello world\n[00:15.00] Foo bar"
            );
        }

        #[test]
        fn test_empty_input() {
            assert_eq!(sanitize_lrc(""), "");
        }

        #[test]
        fn test_first_time_tag_title_pattern_at_under_5s_is_filtered() {
            let input = "[00:01.00] Artist - Title\n[00:05.00] First verse";
            assert_eq!(sanitize_lrc(input), "[00:05.00] First verse");
        }

        #[test]
        fn test_first_time_tag_no_dash_is_kept() {
            let input = "[00:01.00] First verse";
            assert_eq!(sanitize_lrc(input), "[00:01.00] First verse");
        }

        #[test]
        fn test_first_time_tag_title_pattern_at_exactly_5s_is_kept() {
            let input = "[00:05.00] Artist - Title\n[00:10.00] Hello";
            assert_eq!(
                sanitize_lrc(input),
                "[00:05.00] Artist - Title\n[00:10.00] Hello"
            );
        }

        #[test]
        fn test_second_time_tag_title_pattern_is_kept() {
            let input = "[00:01.00] First verse\n[00:03.00] Artist - Title";
            assert_eq!(
                sanitize_lrc(input),
                "[00:01.00] First verse\n[00:03.00] Artist - Title"
            );
        }

        #[test]
        fn test_non_tagged_credit_line_filtered() {
            let input = "Lyrics by: Someone\n[00:05.00] Hello";
            assert_eq!(sanitize_lrc(input), "[00:05.00] Hello");
        }

        #[test]
        fn test_time_tagged_credit_line_filtered() {
            let input = "[00:01.00] Lyrics by: Someone\n[00:05.00] Hello";
            assert_eq!(sanitize_lrc(input), "[00:05.00] Hello");
        }

        #[test]
        fn test_credit_with_fullwidth_colon_filtered() {
            let input = "Written by\u{FF1A} Someone\n[00:05.00] Hello";
            assert_eq!(sanitize_lrc(input), "[00:05.00] Hello");
        }

        #[test]
        fn test_multiple_credits_filtered() {
            let input =
                "[00:01.00] Music by: Composer\n[00:02.00] Arranged by: Arranger\n[00:10.00] Verse";
            assert_eq!(sanitize_lrc(input), "[00:10.00] Verse");
        }

        #[test]
        fn test_credit_prefix_case_insensitive() {
            let input = "LYRICS BY: Someone\n[00:05.00] Hello";
            assert_eq!(sanitize_lrc(input), "[00:05.00] Hello");
        }

        #[test]
        fn test_mixed_metadata_credits_and_lyrics() {
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
                sanitize_lrc(input),
                "[offset:500]\n[00:15.00] Hello\n[00:20.00] World"
            );
        }
    }

    mod strip_section_labels_tests {
        use super::strip_section_labels;

        #[test]
        fn test_basic_removal() {
            let input = "[Verse 1]\nHello there\n[Chorus]\nWe will rock you";
            let expected = "Hello there\nWe will rock you";
            assert_eq!(strip_section_labels(input), expected);
        }

        #[test]
        fn test_variants_and_suffixes() {
            let input =
                "[Verse 2]\n[Pre-Chorus: Lead]\n[Hook - Artist]\n[Chorus 3x]\n[Outro (Fade)]";
            let expected = "";
            assert_eq!(strip_section_labels(input), expected);
        }

        #[test]
        fn test_lrc_format_preserves_timestamps() {
            let input = "[00:10.00][Verse 1]First line\n[00:15.00][Chorus]Second line";
            let expected = "[00:10.00]First line\n[00:15.00]Second line";
            assert_eq!(strip_section_labels(input), expected);
        }

        #[test]
        fn test_case_insensitivity() {
            let input = "[VERSE 1]\nHello\n[chorus]\nRock you";
            let expected = "Hello\nRock you";
            assert_eq!(strip_section_labels(input), expected);
        }

        #[test]
        fn test_word_boundary_safety() {
            let input = "[00:10.00]Breaking the [Chains]\n[Outrageous] behavior";
            let expected = "[00:10.00]Breaking the [Chains]\n[Outrageous] behavior";
            assert_eq!(strip_section_labels(input), expected);
        }

        #[test]
        fn test_mid_line_labels() {
            let input = "[Chorus] We will [Chorus] rock you";
            let expected = "We will rock you";
            assert_eq!(strip_section_labels(input), expected);
        }

        #[test]
        fn test_hyphenated_variants() {
            let input =
                "[Pre-Chorus]\nLet's go\n[Pre Chorus]\nLet's go again\n[Prechorus]\nOne more time";
            let expected = "Let's go\nLet's go again\nOne more time";
            assert_eq!(strip_section_labels(input), expected);
        }

        #[test]
        fn test_ad_lib_variant() {
            let input = "[Ad-lib] Ooh yeah\n[Adlib] Aah";
            let expected = "Ooh yeah\nAah";
            assert_eq!(strip_section_labels(input), expected);
        }
    }
}
