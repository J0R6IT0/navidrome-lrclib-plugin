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

    lrc.lines()
        .filter(|line| {
            let trimmed = line.trim();
            let mut text = trimmed;

            if let Some(rest) = trimmed.strip_prefix('[')
                && let Some(bracket_end) = rest.find(']')
            {
                let content = &rest[..bracket_end];
                text = &rest[bracket_end + 1..];

                let is_time_tag = content.split_once(':').is_some_and(|(left, right)| {
                    left.chars().all(|c| c.is_ascii_digit())
                        && right.contains('.')
                        && right.chars().all(|c| c.is_ascii_digit() || c == '.')
                });

                if is_time_tag {
                } else if let Some((key, _)) = content.split_once(':') {
                    if !KEEP_TAGS.contains(&key) {
                        return false;
                    }
                } else {
                    return false;
                }
            }

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
    use super::strip_section_labels;

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
