use crate::format::lrc::format_timestamp;

const AUDIBLE_PAUSE_MS: i64 = 150;
const IMPLAUSIBLE_WORD_MS: i64 = 40;

pub struct Word {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

impl Word {
    fn new(text: impl Into<String>, start_ms: i64, end_ms: i64) -> Self {
        Self {
            text: text.into(),
            start_ms,
            end_ms,
        }
    }

    fn duration(&self) -> i64 {
        self.end_ms - self.start_ms
    }

    fn is_separator(&self) -> bool {
        self.text.trim().is_empty()
    }
}

fn holds_pause(word_ms: i64, gap_ms: i64) -> bool {
    gap_ms >= AUDIBLE_PAUSE_MS && word_ms >= IMPLAUSIBLE_WORD_MS
}

pub fn render_line(line_start_ms: i64, words: &[Word]) -> Option<String> {
    let tokens = split_pauses(&fold_separators(words));
    let end = tokens.last()?.end_ms;

    let mut buf = format!("[{}]", format_timestamp(line_start_ms));
    for token in &tokens {
        buf.push_str(&format!(
            "<{}>{}",
            format_timestamp(token.start_ms),
            token.text
        ));
    }
    buf.push_str(&format!("<{}>", format_timestamp(end)));
    Some(buf)
}

/// Folds whitespace-only tokens into the word before them unless they are
/// holding a real pause.
fn fold_separators(words: &[Word]) -> Vec<Word> {
    let mut out: Vec<Word> = Vec::with_capacity(words.len());
    for word in words {
        let fold = word.is_separator()
            && out
                .last()
                .is_some_and(|prev| !holds_pause(prev.duration(), word.duration()));

        if fold && let Some(prev) = out.last_mut() {
            prev.text.push_str(&word.text);
            prev.end_ms = word.end_ms;
            continue;
        }
        out.push(Word::new(word.text.clone(), word.start_ms, word.end_ms));
    }
    out
}

/// Splits a word's trailing whitespace onto the pause that follows it.
fn split_pauses(words: &[Word]) -> Vec<Word> {
    let mut out: Vec<Word> = Vec::with_capacity(words.len());
    for (i, word) in words.iter().enumerate() {
        let gap = words.get(i + 1).map(|next| next.start_ms - word.end_ms);
        let spoken = word.text.trim_end();

        if let Some(next_start) = words.get(i + 1).map(|next| next.start_ms)
            && holds_pause(word.duration(), gap.unwrap_or(0))
            && !spoken.is_empty()
            && spoken.len() < word.text.len()
        {
            out.push(Word::new(spoken, word.start_ms, word.end_ms));
            out.push(Word::new(
                &word.text[spoken.len()..],
                word.end_ms,
                next_start,
            ));
            continue;
        }
        out.push(Word::new(word.text.clone(), word.start_ms, word.end_ms));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, start_ms: i64, end_ms: i64) -> Word {
        Word::new(text, start_ms, end_ms)
    }

    #[test]
    fn test_render_line_closes_on_the_last_word() {
        let words = vec![word("hello ", 1000, 2000), word("world", 2000, 3000)];
        assert_eq!(
            render_line(1000, &words).unwrap(),
            "[00:01.00]<00:01.00>hello <00:02.00>world<00:03.00>"
        );
    }

    #[test]
    fn test_render_line_is_none_without_words() {
        assert!(render_line(1000, &[]).is_none());
    }

    #[test]
    fn test_pause_moves_onto_the_trailing_space() {
        let words = vec![
            word("guy ", 129_364, 129_828),
            word("duh", 134_956, 135_316),
        ];
        assert_eq!(
            render_line(129_364, &words).unwrap(),
            "[02:09.36]<02:09.36>guy<02:09.83> <02:14.96>duh<02:15.32>"
        );
    }

    #[test]
    fn test_short_gaps_stay_with_the_word() {
        let words = vec![word("guy ", 1000, 1400), word("duh", 1500, 1800)];
        assert_eq!(
            render_line(1000, &words).unwrap(),
            "[00:01.00]<00:01.00>guy <00:01.50>duh<00:01.80>"
        );
    }

    #[test]
    fn test_gap_after_an_implausibly_short_word_is_reclaimed() {
        let words = vec![word("guy ", 1000, 1020), word("duh", 1800, 2000)];
        assert_eq!(
            render_line(1000, &words).unwrap(),
            "[00:01.00]<00:01.00>guy <00:01.80>duh<00:02.00>"
        );
    }

    #[test]
    fn test_gap_without_trailing_whitespace_is_left_alone() {
        let words = vec![word("難", 1000, 1400), word("忘", 2000, 2400)];
        assert_eq!(
            render_line(1000, &words).unwrap(),
            "[00:01.00]<00:01.00>難<00:02.00>忘<00:02.40>"
        );
    }

    #[test]
    fn test_mistimed_separators_are_folded_into_their_word() {
        let words = vec![
            word("Looks", 42_350, 42_362),
            word(" ", 42_362, 42_726),
            word("like", 42_726, 42_746),
            word(" ", 42_746, 42_969),
            word("blue", 42_969, 42_983),
        ];
        assert_eq!(
            render_line(42_350, &words).unwrap(),
            "[00:42.35]<00:42.35>Looks <00:42.73>like <00:42.97>blue<00:42.98>"
        );
    }

    #[test]
    fn test_well_timed_separators_keep_their_own_segment() {
        let words = vec![
            word("here", 20_490, 20_870),
            word(" ", 20_870, 21_020),
            word("before", 21_020, 21_845),
        ];
        assert_eq!(
            render_line(20_490, &words).unwrap(),
            "[00:20.49]<00:20.49>here<00:20.87> <00:21.02>before<00:21.85>"
        );
    }

    #[test]
    fn test_a_leading_separator_has_no_word_to_fold_into() {
        let words = vec![word(" ", 1000, 1100), word("hi", 1100, 2000)];
        assert_eq!(
            render_line(1000, &words).unwrap(),
            "[00:01.00]<00:01.00> <00:01.10>hi<00:02.00>"
        );
    }
}
