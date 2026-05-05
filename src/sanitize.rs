pub fn sanitize_lrc(lrc: &str) -> String {
    const KEEP_TAGS: &[&str] = &["offset"];

    const CREDIT_PREFIXES: &[&str] = &[
        "Lyrics by",
        "Composed by",
        "Produced by",
        "Published by",
        "Vocals by",
        "Background Vocals by",
        "Additional Vocal by",
        "Mixing Engineer",
        "Mastered by",
        "Executive Producer",
        "Vocal Engineer",
        "Vocals Produced by",
        "Recorded at",
        "Repertoire Owner",
        "Written by",
        "Arranged by",
        "Music by",
        "Words by",
        "Lyrics",
        "Composer",
        "Lyricist",
        "Arranger",
        "Translator",
        "Adapted by",
    ];

    lrc.lines()
        .filter(|line| {
            let trimmed = line.trim();

            let text = if let Some(rest) = trimmed.strip_prefix('[') {
                if let Some(bracket_end) = rest.find(']') {
                    let content = &rest[..bracket_end];
                    if let Some(colon_pos) = content.find(':') {
                        let key = &content[..colon_pos];
                        if key.chars().all(|c| c.is_ascii_alphabetic()) && !KEEP_TAGS.contains(&key)
                        {
                            return false;
                        }
                    }
                    &rest[bracket_end + 1..]
                } else {
                    trimmed
                }
            } else {
                trimmed
            };

            for prefix in CREDIT_PREFIXES {
                if let Some(rest) = text.strip_prefix(prefix)
                    && (rest.starts_with(':') || rest.starts_with('：'))
                {
                    return false;
                }
            }

            true
        })
        .collect::<Vec<_>>()
        .join("\n")
}
