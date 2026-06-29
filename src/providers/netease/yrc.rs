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

        let start: u64 = caps[1].parse().unwrap_or(0);
        let body = &caps[2];

        let mut rendered = format!("[{}]", format_timestamp(start));
        let mut last_word_end: Option<u64> = None;

        for word in word_re.captures_iter(body) {
            let word_start: u64 = word[1].parse().unwrap_or(0);
            let duration: u64 = word[2].parse().unwrap_or(0);
            let text = &word[3];
            rendered.push_str(&format!("<{}>{}", format_timestamp(word_start), text));
            last_word_end = Some(word_start + duration);
        }

        if let Some(end) = last_word_end {
            rendered.push_str(&format!("<{}>", format_timestamp(end)));
            out.push(rendered);
        }
    }

    out.join("\n")
}

fn format_timestamp(ms: u64) -> String {
    let cs = (ms + 5) / 10;
    let hundredths = cs % 100;
    let total_secs = cs / 100;
    let secs = total_secs % 60;
    let mins = total_secs / 60;
    format!("{mins:02}:{secs:02}.{hundredths:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_timestamp() {
        assert_eq!(format_timestamp(0), "00:00.00");
        assert_eq!(format_timestamp(736), "00:00.74");
        assert_eq!(format_timestamp(65_000), "01:05.00");
    }

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
