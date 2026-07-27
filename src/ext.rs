use nd_pdk::lyrics::TrackInfo;

pub trait TrackInfoExt {
    fn label(&self) -> String;
    fn first_artist(&self) -> Option<&str>;
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

    fn first_artist(&self) -> Option<&str> {
        self.artists.first().map(|a| a.name.as_str())
    }
}
