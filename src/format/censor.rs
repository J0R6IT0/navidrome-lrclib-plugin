pub fn is_censored(text: &str) -> bool {
    text.split_whitespace().any(is_masked)
}

fn is_masked(token: &str) -> bool {
    if !token.contains('*') {
        return false;
    }

    let inner = token.trim_matches('*');
    let wrapped = token.starts_with('*') && token.ends_with('*');

    !(wrapped && !inner.contains('*') && inner.chars().any(char::is_alphabetic))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_text_is_not_censored() {
        assert!(!is_censored("Just a smile and the rain is gone"));
    }

    #[test]
    fn test_empty_is_not_censored() {
        assert!(!is_censored(""));
    }

    /// Extracted from real QQ Music payloads.
    #[test]
    fn test_real_qqmusic_masks() {
        for token in [
            "f**k",
            "b**ch",
            "ni**a",
            "sh*t",
            "a**",
            "a*s",
            "p***y",
            "motherf**kers",
            "mother****in",
            "k**l",
            "g*n",
            "w**d",
            "d**n",
            "Paink**lers",
            "Freaky-a*s",
            "Wet-***",
            "****in'",
            "****ing",
        ] {
            assert!(is_censored(&format!("some {token} here")), "{token}");
        }
    }

    #[test]
    fn test_standalone_asterisk_runs() {
        for run in ["**", "***", "****", "*****", "*******", "*************"] {
            assert!(is_censored(&format!("he's {run}")), "{run}");
        }
    }

    #[test]
    fn test_asterisk_wrapped_stage_directions_are_not_censored() {
        assert!(!is_censored("*sigh* I don't know"));
        assert!(!is_censored("she said *laughs* nothing"));
    }

    #[test]
    fn test_wrapped_but_internally_masked_still_counts() {
        assert!(is_censored("*f**k*"));
    }

    #[test]
    fn test_detects_mask_inside_lrc_timestamps() {
        assert!(is_censored("[00:58.11]B**ch be humble hol' up"));
        assert!(!is_censored("[00:58.11]Be humble hol' up"));
    }

    #[test]
    fn test_trailing_punctuation_around_mask() {
        assert!(is_censored("(f**k)"));
        assert!(is_censored("f**k,"));
        assert!(is_censored("\"f**k\""));
    }
}
