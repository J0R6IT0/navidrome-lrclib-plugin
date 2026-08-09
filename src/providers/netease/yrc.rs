use crate::format::elrc;
use regex::Regex;

/// YRC lines look like:
///   `[lineStart,lineDuration](wordStart,wordDuration,0)word(wordStart,...)word`
/// where every timestamp is in absolute milliseconds. Header/metadata lines are
/// JSON objects (`{"t":...}`) and are skipped.
pub fn to_enhanced_lrc(yrc: &str) -> String {
    let line_re = Regex::new(r"^\[(\d+),\d+\](.*)$").unwrap();
    let word_re = Regex::new(r"\((\d+),(\d+),-?\d+\)([^(]*)").unwrap();

    let mut out = Vec::new();

    for line in yrc.lines() {
        let Some(caps) = line_re.captures(line.trim()) else {
            continue;
        };

        let start: i64 = caps[1].parse().unwrap_or(0);
        let words: Vec<elrc::Word> = word_re
            .captures_iter(&caps[2])
            .map(|word| {
                let word_start: i64 = word[1].parse().unwrap_or(0);
                let duration: i64 = word[2].parse().unwrap_or(0);
                elrc::Word {
                    text: word[3].to_string(),
                    start_ms: word_start,
                    end_ms: word_start + duration,
                }
            })
            .collect();

        if let Some(rendered) = elrc::render_line(start, &words) {
            out.push(rendered);
        }
    }

    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_word_timing_absolute() {
        let yrc = "[0,2210](0,736,0)foo(736,736,0)bar(1472,736,0)baz";
        assert_eq!(
            to_enhanced_lrc(yrc),
            "[00:00.00]<00:00.00>foo<00:00.74>bar<00:01.47>baz<00:02.21>"
        );
    }

    #[test]
    fn test_convert_multiple_lines() {
        let yrc = "[4890,6250](4890,270,0)foo(5160,270,0)：\n[11140,3260](11140,270,0)bar";
        assert_eq!(
            to_enhanced_lrc(yrc),
            "[00:04.89]<00:04.89>foo<00:05.16>：<00:05.43>\n[00:11.14]<00:11.14>bar<00:11.41>"
        );
    }

    #[test]
    fn test_convert_closes_last_word() {
        let yrc = "[0,1000](0,500,0)hi";
        assert_eq!(to_enhanced_lrc(yrc), "[00:00.00]<00:00.00>hi<00:00.50>");
    }

    #[test]
    fn test_skips_json_metadata_lines() {
        let yrc = "{\"t\":0,\"c\":[{\"tx\":\"foo: \"}]}\n[0,1000](0,500,0)hi";
        assert_eq!(to_enhanced_lrc(yrc), "[00:00.00]<00:00.00>hi<00:00.50>");
    }

    #[test]
    fn test_keeps_fullwidth_parens_in_text() {
        let yrc = "[0,1000](0,500,0)（foo）";
        assert_eq!(
            to_enhanced_lrc(yrc),
            "[00:00.00]<00:00.00>（foo）<00:00.50>"
        );
    }

    #[test]
    fn test_convert_empty() {
        assert_eq!(to_enhanced_lrc(""), "");
    }
}
