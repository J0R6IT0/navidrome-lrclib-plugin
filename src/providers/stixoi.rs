use crate::{
    config::{PluginConfig, ProviderParams},
    ext::TrackInfoExt,
    providers::{LyricsProvider, ProviderResult, error::ProviderError, http::Http},
    types::{Lyrics, LyricsKind},
};
use nd_pdk::lyrics::TrackInfo;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashSet;

const BASE_URL: &str = "https://stixoi.info";

// How many search results to fetch and inspect before giving up.
const MAX_PROBE: usize = 8;

#[derive(Debug, Default, Deserialize)]
struct SongPage {
    #[serde(default)]
    title: String,
    #[serde(default)]
    lyrics: String,
    #[serde(default)]
    singers: Vec<String>,
    #[serde(default)]
    composers: Vec<String>,
    #[serde(default)]
    lyricists: Vec<String>,
}

impl SongPage {
    fn credits(&self) -> Vec<&str> {
        self.singers
            .iter()
            .chain(&self.composers)
            .chain(&self.lyricists)
            .map(String::as_str)
            .collect()
    }
}

pub struct Stixoi;

impl Stixoi {
    pub fn create(_params: &ProviderParams) -> Box<dyn LyricsProvider> {
        Box::new(Self)
    }
}

impl LyricsProvider for Stixoi {
    fn supported_kinds(&self) -> &'static [LyricsKind] {
        &[LyricsKind::Plain]
    }

    fn fetch_lyrics(
        &self,
        track: &TrackInfo,
        _cfg: &PluginConfig,
    ) -> ProviderResult<Option<Lyrics>> {
        let title = strip_parens(&track.title);
        if title.is_empty() {
            return Ok(None);
        }

        let artist = track.first_artist().unwrap_or_default();

        let combined = if artist.is_empty() {
            title.clone()
        } else {
            format!("{title} {artist}")
        };

        if let Some(lyrics) = self.find_lyrics(&combined, &title, artist)? {
            return Ok(Some(Lyrics::Plain(lyrics)));
        }

        if combined != title
            && let Some(lyrics) = self.find_lyrics(&title, &title, artist)?
        {
            return Ok(Some(Lyrics::Plain(lyrics)));
        }

        Ok(None)
    }
}

impl Stixoi {
    fn find_lyrics(
        &self,
        query: &str,
        title: &str,
        artist: &str,
    ) -> ProviderResult<Option<String>> {
        let ids = self.search_ids(query)?;

        let mut fallback: Option<String> = None;

        for id in ids.iter().take(MAX_PROBE) {
            let song = match self.fetch_song(id)? {
                Some(s) => s,
                None => continue,
            };

            if song.lyrics.trim().is_empty() || !title_equal(title, &song.title) {
                continue;
            }

            if artist_matches(artist, &song.credits()) {
                return Ok(Some(song.lyrics));
            }

            if fallback.is_none() {
                fallback = Some(song.lyrics);
            }
        }

        Ok(fallback)
    }

    fn search_ids(&self, query: &str) -> ProviderResult<Vec<String>> {
        let response = self
            .request(format!("{BASE_URL}/search"))
            .param("q", query)
            .param("scope", "songs")
            .send()?;

        if response.status != 200 {
            return Err(response.unexpected_status("the search"));
        }

        let body = response.text();

        let re = Regex::new(r#""href":"/songs/(\d+)""#)
            .map_err(|e| ProviderError::other(format!("invalid song link regex: {e}")))?;

        let mut ids = Vec::new();
        for cap in re.captures_iter(&body) {
            if let Some(m) = cap.get(1) {
                let id = m.as_str().to_string();
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }

        Ok(ids)
    }

    fn fetch_song(&self, id: &str) -> ProviderResult<Option<SongPage>> {
        let response = self.request(format!("{BASE_URL}/songs/{id}")).send()?;

        match response.status {
            200 => {}
            404 => return Ok(None),
            _ => return Err(response.unexpected_status("the song page")),
        }

        let body = response.text();
        let anchor = format!(r#""song":{{"id":{id}"#);

        let Some(obj) = extract_json_object(&body, &anchor) else {
            return Ok(None);
        };

        serde_json::from_str(obj)
            .map(Some)
            .map_err(|e| ProviderError::other(format!("failed to parse the song payload: {e}")))
    }

    fn request(&self, url: String) -> Http {
        Http::get(url).browser().header("RSC", "1")
    }
}

fn extract_json_object<'a>(haystack: &'a str, anchor: &str) -> Option<&'a str> {
    let anchor_at = haystack.find(anchor)?;
    let start = anchor_at + haystack[anchor_at..].find('{')?;

    let bytes = haystack.as_bytes();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, &b) in bytes[start..].iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }

        match b {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&haystack[start..start + offset + 1]);
                }
            }
            _ => {}
        }
    }

    None
}

fn strip_parens(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut depth = 0u32;

    for c in title.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }

    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fold(c: char) -> char {
    match c {
        'ά' => 'α',
        'έ' => 'ε',
        'ή' => 'η',
        'ί' | 'ϊ' | 'ΐ' => 'ι',
        'ό' => 'ο',
        'ύ' | 'ϋ' | 'ΰ' => 'υ',
        'ώ' => 'ω',
        'ς' => 'σ',

        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        other => other,
    }
}

fn normalize(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(fold)
        .filter(|c| c.is_alphanumeric())
        .collect()
}

fn tokens(s: &str) -> Vec<String> {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(fold)
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

fn title_equal(a: &str, b: &str) -> bool {
    let a = normalize(a);
    !a.is_empty() && a == normalize(b)
}

fn artist_matches(artist: &str, credits: &[&str]) -> bool {
    let want = tokens(artist);
    if want.is_empty() {
        return false;
    }

    credits.iter().any(|name| {
        let have: HashSet<String> = tokens(name).into_iter().collect();
        want.iter().all(|t| have.contains(t))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_parens() {
        assert_eq!(strip_parens("Αγάπη (Live)"), "Αγάπη");
        assert_eq!(strip_parens("Song [Remix] (2020)"), "Song");
        assert_eq!(strip_parens("  spaced   out  "), "spaced out");
        assert_eq!(strip_parens("No parens"), "No parens");
    }

    #[test]
    fn test_normalize_greek_accents() {
        assert_eq!(normalize("Αγάπη"), normalize("ΑΓΑΠΗ"));
        assert_eq!(normalize("Της δικαιοσύνης!"), "τησδικαιοσυνησ");
    }

    #[test]
    fn test_title_equal_ignores_accents_and_punctuation() {
        assert!(title_equal("Αγάπη", "ΑΓΑΠΗ"));
        assert!(title_equal("Της δικαιοσύνης, ήλιε", "Της δικαιοσυνης ηλιε"));
        assert!(!title_equal("Αγάπη", "Αγάπη μου"));
        assert!(!title_equal("", "anything"));
    }

    #[test]
    fn test_artist_matches_surname_first() {
        let credits = vec!["Νταλάρας Γιώργος"];
        assert!(artist_matches("Γιώργος Νταλάρας", &credits));
        assert!(artist_matches("Νταλάρας", &credits));
        assert!(!artist_matches("Μητροπάνος", &credits));
        assert!(!artist_matches("", &credits));
    }

    #[test]
    fn test_artist_matches_across_credit_kinds() {
        let credits = vec!["Theodorakis Mikis", "Elytis Odysseas"];
        assert!(artist_matches("Mikis Theodorakis", &credits));
        assert!(artist_matches("Odysseas Elytis", &credits));
    }

    #[test]
    fn test_extract_json_object_simple() {
        let s = r#"prefix "song":{"id":70,"title":"A","nested":{"x":1}} suffix"#;
        let obj = extract_json_object(s, r#""song":{"id":70"#).unwrap();
        assert_eq!(obj, r#"{"id":70,"title":"A","nested":{"x":1}}"#);
    }

    #[test]
    fn test_extract_json_object_braces_in_string() {
        let s = r#""song":{"id":1,"lyrics":"a {b} \"c\" }","ok":true}"#;
        let obj = extract_json_object(s, r#""song":{"id":1"#).unwrap();
        assert_eq!(obj, r#"{"id":1,"lyrics":"a {b} \"c\" }","ok":true}"#);
    }

    #[test]
    fn test_extract_json_object_missing_anchor() {
        assert_eq!(
            extract_json_object("nothing here", r#""song":{"id":9"#),
            None
        );
    }

    #[test]
    fn test_song_page_parses_and_collects_credits() {
        let json = r#"{"id":70,"title":"Αγάπη","lyrics":"line one\nline two",
            "lyricists":["Τριπολίτης Κώστας"],"composers":["Θεοδωράκης Μίκης"],
            "singers":["Νταλάρας Γιώργος"],"albums":["X (1981)"],"versions":[]}"#;
        let song: SongPage = serde_json::from_str(json).unwrap();
        assert_eq!(song.title, "Αγάπη");
        assert_eq!(song.lyrics, "line one\nline two");
        assert_eq!(
            song.credits(),
            vec!["Νταλάρας Γιώργος", "Θεοδωράκης Μίκης", "Τριπολίτης Κώστας"]
        );
    }
}
