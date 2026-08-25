use nd_pdk::lyrics::TrackInfo;

pub trait TrackInfoExt {
    fn label(&self) -> String;
    fn has_artist(&self) -> bool;
    fn first_artist(&self) -> Option<&str>;
    fn all_artists(&self) -> String;
    fn duration_secs(&self) -> i64;
    fn duration_ms(&self) -> u64;
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
}
