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

    #[track_caller]
    fn check(text: &str, expected: bool) {
        assert_eq!(is_censored(text), expected, "{text}");
    }

    #[track_caller]
    fn check_tokens(tokens: &[&str], expected: bool) {
        for token in tokens {
            check(token, expected);
            check(&format!("some {token} here"), expected);
        }
    }

    #[test]
    fn plain_lyrics_are_not_censored() {
        check("Just a smile and the rain is gone", false);
        check("[00:58.11]Be humble hol' up", false);
        check("", false);
    }

    /// Masks extracted from real QQ Music payloads.
    #[test]
    fn masked_words_are_censored() {
        check_tokens(
            &[
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
            ],
            true,
        );
    }

    #[test]
    fn a_bare_run_of_asterisks_is_censored() {
        check_tokens(&["**", "***", "****", "*****", "*******"], true);
    }

    #[test]
    fn asterisks_around_a_word_are_a_stage_direction() {
        check("*sigh* I don't know", false);
        check("she said *laughs* nothing", false);
    }

    #[test]
    fn a_stage_direction_can_still_be_censored() {
        check("*f**k*", true);
    }

    #[test]
    fn punctuation_does_not_affect_detection() {
        check_tokens(&["(f**k)", "f**k,", "\"f**k\"", "[00:58.11]B**ch"], true);
    }
}
