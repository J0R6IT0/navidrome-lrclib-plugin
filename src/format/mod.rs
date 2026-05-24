use regex::Regex;

pub mod lrc;

pub fn strip_section_labels(lyrics: &str) -> String {
    let re = Regex::new(
        r"(?i)\[(?:verse|pre[- ]?verse|chorus|pre[- ]?chorus|post[- ]?chorus|bridge|hook|refrain|intro|outro|coda|interlude|instrumental|breakdown|solo|drop|chant|skit|ad-?lib|overture|finale|couplet)\b[^\]]*\]"
    )
    .unwrap();

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
