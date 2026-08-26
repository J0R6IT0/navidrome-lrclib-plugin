use nd_pdk::lyrics::TrackInfo;

pub trait TrackInfoExt {
    fn label(&self) -> String;
    fn has_artist(&self) -> bool;
    fn first_artist(&self) -> Option<&str>;
    fn all_artists(&self) -> String;
    fn duration_secs(&self) -> i64;
    fn duration_ms(&self) -> u64;
    fn title_without_parens(&self) -> String;
}

impl TrackInfoExt for TrackInfo {
    fn label(&self) -> String {
        match (self.artist.trim(), self.title.trim()) {
            ("", "") => self.id.clone(),
            ("", title) => title.to_string(),
            (artist, "") => artist.to_string(),
            (artist, title) => format!("{artist} - {title}"),
        }
    }

    fn has_artist(&self) -> bool {
        !self.artists.is_empty()
    }

    fn first_artist(&self) -> Option<&str> {
        self.artists.first().map(|a| a.name.as_str())
    }

    fn all_artists(&self) -> String {
        self.artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn duration_secs(&self) -> i64 {
        self.duration.round().max(0.0) as i64
    }

    fn duration_ms(&self) -> u64 {
        (self.duration * 1000.0).round().max(0.0) as u64
    }

    fn title_without_parens(&self) -> String {
        let mut out = String::with_capacity(self.title.len());
        let mut depth = 0u32;

        for c in self.title.chars() {
            match c {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth = depth.saturating_sub(1),
                _ if depth == 0 => out.push(c),
                _ => {}
            }
        }

        out.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(title: &str) -> TrackInfo {
        TrackInfo {
            title: title.to_string(),
            ..Default::default()
        }
    }

    #[track_caller]
    fn check_title_without_parens(input: &str, expected: &str) {
        assert_eq!(track(input).title_without_parens(), expected);
    }

    #[test]
    fn strip_parens_removes_bracketed_segments() {
        check_title_without_parens("Song (Live)", "Song");
        check_title_without_parens("Song [Remastered 2020]", "Song");
        check_title_without_parens("Song {Deluxe}", "Song");
        check_title_without_parens("Song (feat. Artist [Live])", "Song");
        check_title_without_parens("Song [Remix] (2020)", "Song");
    }

    #[test]
    fn strip_parens_removes_whitespace() {
        check_title_without_parens("Song   (Live)   Version", "Song Version");
    }

    #[test]
    fn strip_parens_leaves_plain_titles_untouched() {
        check_title_without_parens("Plain Title", "Plain Title");
        check_title_without_parens("", "");
    }
}
