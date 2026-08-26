use std::time::Duration;

use nd_pdk::lyrics::TrackInfo;

pub trait TrackInfoExt {
    fn label(&self) -> String;
    fn has_artist(&self) -> bool;
    fn first_artist(&self) -> Option<&str>;
    fn all_artists(&self) -> String;
    fn duration(&self) -> Duration;
    fn clean_title(&self) -> String;
    fn matches_duration(&self, other: Duration, tolerance: Duration) -> bool;
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

    fn duration(&self) -> Duration {
        Duration::from_secs_f32(self.duration.max(0.0))
    }

    fn clean_title(&self) -> String {
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

        out.split_whitespace()
            .take_while(|word| !matches!(*word, "-" | "‐" | "‒" | "–" | "—" | "―"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn matches_duration(&self, other: Duration, tolerance: Duration) -> bool {
        other.abs_diff(self.duration()) <= tolerance
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
    fn check_clean_title(input: &str, expected: &str) {
        assert_eq!(track(input).clean_title(), expected, "title {input:?}");
    }

    #[test]
    fn a_clean_title_drops_bracketed_segments() {
        check_clean_title("Song (Live)", "Song");
        check_clean_title("Song [Remastered 2020]", "Song");
        check_clean_title("Song {Deluxe}", "Song");
        check_clean_title("Song (feat. Artist [Live])", "Song");
        check_clean_title("Song [Remix] (2020)", "Song");
    }

    #[test]
    fn a_clean_title_drops_dash_suffixes() {
        check_clean_title("Song - Remastered 2011", "Song");
        check_clean_title("Song - Live - 2011 Remaster", "Song");
        check_clean_title("Song \u{2013} Live", "Song");
        check_clean_title("Song \u{2014} Live", "Song");
        check_clean_title("Song (Live) - Remastered", "Song");
    }

    #[test]
    fn a_clean_title_keeps_dashes_inside_words() {
        check_clean_title("Song-Title", "Song-Title");
        check_clean_title("Song -Title", "Song -Title");
    }

    #[test]
    fn a_clean_title_collapses_whitespace() {
        check_clean_title("Song   (Live)   Version", "Song Version");
    }

    #[test]
    fn a_clean_title_leaves_plain_titles_untouched() {
        check_clean_title("Song Title", "Song Title");
        check_clean_title("", "");
    }
}
