use regex::Regex;

pub mod censor;
pub mod lrc;
pub mod lyricsfile;
pub mod ttml;

pub fn strip_section_labels(lyrics: &str) -> String {
    let re = Regex::new(
        r"(?i)^(?:verse|pre[- ]?verse|chorus|pre[- ]?chorus|post[- ]?chorus|bridge|hook|refrain|intro|outro|coda|interlude|instrumental|breakdown|solo|drop|chant|skit|ad-?lib|overture|finale|couplet|coro|pre[- ]?coro|post[- ]?coro|verso|puente|refrán|interludio)\b"
    )
    .unwrap();

    let mut out: Vec<String> = Vec::new();

    let mut pending: Vec<String> = Vec::new();
    let mut run_has_label = false;

    for line in lyrics.lines() {
        let stripped = strip_labels_in_line(line, &re);
        let cleaned = stripped.replace("  ", " ");

        let cleaned = if line.trim_start().starts_with('[') {
            cleaned.trim().to_string()
        } else {
            cleaned.trim_end().to_string()
        };

        if cleaned.trim().is_empty() {
            continue;
        }

        if lrc::is_blank_timed_line(&cleaned) {
            run_has_label |= !lrc::is_blank_timed_line(line);
            pending.push(lrc::timestamp_only(&cleaned));
            continue;
        }

        if run_has_label {
            if gap_warrants_blank(&pending, &cleaned) {
                out.push(pending.remove(0));
            }
            pending.clear();
        } else {
            out.append(&mut pending);
        }
        run_has_label = false;

        out.push(cleaned);
    }

    if !run_has_label {
        out.append(&mut pending);
    }

    out.join("\n")
}

fn strip_labels_in_line(line: &str, re: &Regex) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;

    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find(']').map(|i| open + i) else {
            break;
        };

        let content = lrc::strip_word_tags(&rest[open + 1..close]);

        if re.is_match(content.trim()) {
            out.push_str(&rest[..open]);
        } else {
            out.push_str(&rest[..=close]);
        }

        rest = &rest[close + 1..];
    }

    out.push_str(rest);
    out
}

fn gap_warrants_blank(pending: &[String], next: &str) -> bool {
    let (Some(start), Some(end)) = (
        pending.first().and_then(|l| lrc::time_tag_secs(l)),
        lrc::time_tag_secs(next),
    ) else {
        return false;
    };

    end - start >= lrc::BLANK_GAP_MIN_SECS
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

        #[test]
        fn test_enhanced_lrc_label_line_removed() {
            let input = concat!(
                "[00:00.06]<00:00.06>[<00:00.13>Intro<00:00.20>]<00:00.27>\n",
                "[00:00.27]<00:00.27>Getting <00:00.72>late"
            );
            let expected = "[00:00.27]<00:00.27>Getting <00:00.72>late";
            assert_eq!(strip_section_labels(input), expected);
        }

        #[test]
        fn test_enhanced_lrc_word_tags_preserved() {
            let input = "[00:10.00]<00:10.00>Hello <00:10.50>world";
            assert_eq!(strip_section_labels(input), input);
        }

        #[test]
        fn test_lrc_label_and_following_blank_removed() {
            let input = "[00:00.00]Intro\n[00:02.00][Verse]\n[00:04.00]\n[00:06.00]First";
            let expected = "[00:00.00]Intro\n[00:06.00]First";
            assert_eq!(strip_section_labels(input), expected);
        }

        #[test]
        fn test_lrc_label_and_preceding_blank_removed() {
            let input = "[00:00.00]A\n[00:02.00]\n[00:04.00][Verse]\n[00:06.00]B";
            let expected = "[00:00.00]A\n[00:06.00]B";
            assert_eq!(strip_section_labels(input), expected);
        }

        #[test]
        fn test_lrc_label_and_blanks_both_sides_removed() {
            let input = "[00:00.00]A\n[00:02.00]\n[00:04.00][Verse]\n[00:06.00]\n[00:08.00]B";
            let expected = "[00:00.00]A\n[00:02.00]\n[00:08.00]B";
            assert_eq!(strip_section_labels(input), expected);
        }

        #[test]
        fn test_lrc_large_gap_keeps_blank() {
            let input = "[00:01.00]Foo\n[00:03.00]\n[00:03.00][Chorus]\n[00:10.00]Bar";
            let expected = "[00:01.00]Foo\n[00:03.00]\n[00:10.00]Bar";
            assert_eq!(strip_section_labels(input), expected);
        }

        #[test]
        fn test_lrc_small_gap_drops_blank() {
            let input = "[00:01.00]Foo\n[00:03.00]\n[00:03.00][Chorus]\n[00:04.00]Bar";
            let expected = "[00:01.00]Foo\n[00:04.00]Bar";
            assert_eq!(strip_section_labels(input), expected);
        }

        #[test]
        fn test_lrc_label_line_always_removed() {
            let input = "[00:00.00]Outro line\n[00:02.00][Outro]\n[00:04.00]End";
            let expected = "[00:00.00]Outro line\n[00:04.00]End";
            assert_eq!(strip_section_labels(input), expected);
        }

        #[test]
        fn test_lrc_leading_label_dropped() {
            let input = "[00:00.00][Verse]\n[00:02.00]Hi";
            let expected = "[00:02.00]Hi";
            assert_eq!(strip_section_labels(input), expected);
        }

        #[test]
        fn test_lrc_blank_without_adjacent_label_kept() {
            let input = "[00:00.00]A\n[00:02.00]\n[00:04.00]B";
            let expected = "[00:00.00]A\n[00:02.00]\n[00:04.00]B";
            assert_eq!(strip_section_labels(input), expected);
        }
    }
}
