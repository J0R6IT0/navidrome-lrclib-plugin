use crate::types::SyncLevel;

pub fn sync_level(doc: &str) -> SyncLevel {
    let mut level = SyncLevel::Plain;
    let mut in_lines = false;
    let mut expecting_word = false;

    for line in doc.lines() {
        if is_top_level_key(line) {
            in_lines = line.starts_with("lines:");
            expecting_word = false;
            continue;
        }

        if !in_lines {
            continue;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if expecting_word {
            if trimmed.starts_with('-') {
                return SyncLevel::Word;
            }
            expecting_word = false;
        }

        match words_value(trimmed) {
            Some("") => expecting_word = true,
            Some("[]") => {}
            Some(_) => return SyncLevel::Word,
            None => {
                if trimmed.trim_start_matches("- ").starts_with("start_ms:") {
                    level = SyncLevel::Line;
                }
            }
        }
    }

    level
}

fn is_top_level_key(line: &str) -> bool {
    !line.starts_with([' ', '\t', '-']) && !line.trim().is_empty()
}

fn words_value(trimmed: &str) -> Option<&str> {
    trimmed
        .trim_start_matches("- ")
        .strip_prefix("words:")
        .map(str::trim)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn check(doc: &str, expected: SyncLevel) {
        assert_eq!(sync_level(doc), expected, "{doc}");
    }

    #[test]
    fn no_timed_lines_is_plain() {
        check(
            concat!(
                "version: '1.0'\n",
                "metadata:\n",
                "  title: Some Song\n",
                "plain: |-\n",
                "  First line\n",
                "  Second line\n",
            ),
            SyncLevel::Plain,
        );
        check(
            concat!(
                "version: '1.0'\n",
                "metadata:\n",
                "  title: Creep\n",
                "lines: []\n",
                "plain: |-\n",
                "  When you were here before\n",
            ),
            SyncLevel::Plain,
        );
        check(
            "lines:\n- text: First line\n- text: Second line\n",
            SyncLevel::Plain,
        );
        check("", SyncLevel::Plain);
    }

    #[test]
    fn lines_with_a_start_are_line_synced() {
        check(
            concat!(
                "version: '1.0'\n",
                "metadata:\n",
                "  title: Some Song\n",
                "lines:\n",
                "- text: First line\n",
                "  start_ms: 8340\n",
                "  end_ms: 12080\n",
                "plain: |-\n",
                "  First line\n",
            ),
            SyncLevel::Line,
        );
    }

    #[test]
    fn lines_with_words_are_word_synced() {
        check(
            concat!(
                "version: \"1.0\"\n",
                "lines:\n",
                "  - text: Just a smile\n",
                "    words:\n",
                "      - text: \"Just \"\n",
                "        start_ms: 6070\n",
                "    start_ms: 6070\n",
                "    end_ms: 10880\n",
                "plain: |-\n",
                "  Just a smile\n",
            ),
            SyncLevel::Word,
        );
        check(
            "lines:\n  - text: Hi\n    words: [{text: Hi, start_ms: 10}]\n",
            SyncLevel::Word,
        );
    }

    #[test]
    fn an_empty_words_list_is_only_line_synced() {
        check(
            concat!(
                "lines:\n",
                "  - text: \"\"\n",
                "    words: []\n",
                "    start_ms: 117410\n",
            ),
            SyncLevel::Line,
        );
    }

    #[test]
    fn a_single_line_with_words_is_word_synced() {
        check(
            concat!(
                "lines:\n",
                "  - text: \"\"\n",
                "    words: []\n",
                "    start_ms: 0\n",
                "  - text: Hello\n",
                "    words:\n",
                "      - text: Hello\n",
                "        start_ms: 100\n",
                "    start_ms: 100\n",
            ),
            SyncLevel::Word,
        );
    }
}
