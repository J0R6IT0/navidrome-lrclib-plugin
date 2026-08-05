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

    const PLAIN: &str = concat!(
        "version: '1.0'\n",
        "metadata:\n",
        "  title: Some Song\n",
        "plain: |-\n",
        "  First line\n",
        "  Second line\n",
    );

    const LINE: &str = concat!(
        "version: '1.0'\n",
        "metadata:\n",
        "  title: Some Song\n",
        "lines:\n",
        "- text: First line\n",
        "  start_ms: 8340\n",
        "  end_ms: 12080\n",
        "plain: |-\n",
        "  First line\n",
    );

    const WORD: &str = concat!(
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
    );

    #[test]
    fn test_plain_without_lines() {
        assert_eq!(sync_level(PLAIN), SyncLevel::Plain);
    }

    #[test]
    fn test_empty_lines_list_is_plain() {
        let doc = concat!(
            "version: '1.0'\n",
            "metadata:\n",
            "  title: Creep\n",
            "lines: []\n",
            "plain: |-\n",
            "  When you were here before\n",
        );
        assert_eq!(sync_level(doc), SyncLevel::Plain);
    }

    #[test]
    fn test_line_timed() {
        assert_eq!(sync_level(LINE), SyncLevel::Line);
    }

    #[test]
    fn test_word_timed() {
        assert_eq!(sync_level(WORD), SyncLevel::Word);
    }

    #[test]
    fn test_empty_words_list_is_line() {
        let doc = concat!(
            "lines:\n",
            "  - text: \"\"\n",
            "    words: []\n",
            "    start_ms: 117410\n",
        );
        assert_eq!(sync_level(doc), SyncLevel::Line);
    }

    #[test]
    fn test_one_word_timed_line_wins() {
        let doc = concat!(
            "lines:\n",
            "  - text: \"\"\n",
            "    words: []\n",
            "    start_ms: 0\n",
            "  - text: Hello\n",
            "    words:\n",
            "      - text: Hello\n",
            "        start_ms: 100\n",
            "    start_ms: 100\n",
        );
        assert_eq!(sync_level(doc), SyncLevel::Word);
    }

    #[test]
    fn test_flow_style_words_list() {
        let doc = "lines:\n  - text: Hi\n    words: [{text: Hi, start_ms: 10}]\n";
        assert_eq!(sync_level(doc), SyncLevel::Word);
    }

    #[test]
    fn test_untimed_lines_are_plain() {
        let doc = "lines:\n- text: First line\n- text: Second line\n";
        assert_eq!(sync_level(doc), SyncLevel::Plain);
    }

    #[test]
    fn test_plain_block_content_is_not_scanned() {
        let doc = concat!(
            "version: '1.0'\n",
            "plain: |-\n",
            "  words:\n",
            "  - start_ms: not really yaml\n",
        );
        assert_eq!(sync_level(doc), SyncLevel::Plain);
    }

    #[test]
    fn test_empty_document_is_plain() {
        assert_eq!(sync_level(""), SyncLevel::Plain);
    }
}
