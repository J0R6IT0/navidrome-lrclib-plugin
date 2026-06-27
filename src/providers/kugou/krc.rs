use flate2::read::ZlibDecoder;
use regex::Regex;
use std::io::Read;

/// XOR key used to obfuscate KuGou KRC payloads.
const KRC_KEY: [u8; 16] = [
    0x40, 0x47, 0x61, 0x77, 0x5e, 0x32, 0x74, 0x47, 0x51, 0x36, 0x31, 0x2d, 0xce, 0xd2, 0x6e, 0x69,
];

const MAGIC: &[u8] = b"krc1";

pub fn to_enhanced_lrc(decoded: &[u8]) -> Result<String, String> {
    let krc = decrypt(decoded)?;
    Ok(convert(&krc))
}

fn decrypt(decoded: &[u8]) -> Result<String, String> {
    if decoded.len() <= MAGIC.len() || &decoded[..MAGIC.len()] != MAGIC {
        return Err("missing krc1 magic header".to_string());
    }

    let body: Vec<u8> = decoded[MAGIC.len()..]
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ KRC_KEY[i % KRC_KEY.len()])
        .collect();

    let mut decoder = ZlibDecoder::new(&body[..]);
    let mut bytes = Vec::new();
    decoder
        .read_to_end(&mut bytes)
        .map_err(|e| format!("krc inflate failed: {e}"))?;

    String::from_utf8(bytes).map_err(|e| format!("krc is not valid UTF-8: {e}"))
}

fn convert(krc: &str) -> String {
    let line_re = Regex::new(r"^\[(\d+),\d+\](.*)$").unwrap();
    let word_re = Regex::new(r"<(\d+),(\d+),-?\d+>([^<]*)").unwrap();

    let mut out = Vec::new();

    for line in krc.lines() {
        let Some(caps) = line_re.captures(line.trim()) else {
            continue;
        };

        let start: u64 = caps[1].parse().unwrap_or(0);
        let body = &caps[2];

        let mut rendered = format!("[{}]", format_timestamp(start));
        let mut last_word_end: Option<u64> = None;

        for word in word_re.captures_iter(body) {
            let offset: u64 = word[1].parse().unwrap_or(0);
            let duration: u64 = word[2].parse().unwrap_or(0);
            let text = &word[3];
            rendered.push_str(&format!("<{}>{}", format_timestamp(start + offset), text));
            last_word_end = Some(offset + duration);
        }

        if let Some(end) = last_word_end {
            rendered.push_str(&format!("<{}>", format_timestamp(start + end)));
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
        assert_eq!(format_timestamp(5890), "00:05.89");
        assert_eq!(format_timestamp(65_000), "01:05.00");
    }

    #[test]
    fn test_convert_word_timing() {
        let krc = "[0,5890]<0,736,0>foo<736,736,0>bar<1472,736,0>baz";
        assert_eq!(
            convert(krc),
            "[00:00.00]<00:00.00>foo<00:00.74>bar<00:01.47>baz<00:02.21>"
        );
    }

    #[test]
    fn test_convert_multiple_lines_and_offsets() {
        let krc = "[5890,5900]<0,1180,0>foo<1180,1180,0>：\n[11790,5900]<0,1180,0>bar";
        assert_eq!(
            convert(krc),
            "[00:05.89]<00:05.89>foo<00:07.07>：<00:08.25>\n[00:11.79]<00:11.79>bar<00:12.97>"
        );
    }

    #[test]
    fn test_convert_closes_last_word() {
        let krc = "[0,1000]<0,500,0>hi";
        assert_eq!(convert(krc), "[00:00.00]<00:00.00>hi<00:00.50>");
    }

    #[test]
    fn test_convert_skips_metadata_lines() {
        let krc = "[ti:Song]\n[ar:Artist]\n[offset:0]\n[0,1000]<0,500,0>hi";
        assert_eq!(convert(krc), "[00:00.00]<00:00.00>hi<00:00.50>");
    }

    #[test]
    fn test_convert_empty() {
        assert_eq!(convert(""), "");
    }

    #[test]
    fn test_decrypt_rejects_non_krc() {
        assert!(to_enhanced_lrc(b"not a krc payload").is_err());
    }
}
