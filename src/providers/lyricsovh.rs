use crate::{
    config::{PluginConfig, ProviderParams},
    ext::TrackInfoExt,
    providers::{LyricsProvider, ProviderResult, http::Http},
    types::{Lyrics, LyricsKind},
};
use nd_pdk::lyrics::TrackInfo;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Deserialize;

const DEFAULT_BASE_URL: &str = "https://api.lyrics.ovh";

#[derive(Debug, Deserialize)]
struct ApiResponse {
    lyrics: String,
}

pub struct LyricsOvh {
    base_url: String,
}

impl LyricsOvh {
    pub fn create(params: &ProviderParams) -> Box<dyn LyricsProvider> {
        Box::new(Self {
            base_url: params
                .get("baseUrl")
                .unwrap_or(DEFAULT_BASE_URL)
                .to_string(),
        })
    }
}

impl LyricsProvider for LyricsOvh {
    fn supported_kinds(&self) -> &'static [LyricsKind] {
        &[LyricsKind::Plain]
    }

    fn log_params(&self) -> Vec<(&'static str, String)> {
        vec![("baseUrl", self.base_url.clone())]
    }

    fn fetch_lyrics(
        &self,
        track: &TrackInfo,
        _cfg: &PluginConfig,
    ) -> ProviderResult<Option<Lyrics>> {
        let first_artist = track.first_artist().unwrap();
        let url = search_url(&self.base_url, first_artist, &track.title);
        let response = Http::get(url).send()?;

        match response.status {
            200 => {
                let body: ApiResponse = response.json("lyrics")?;
                if body.lyrics.trim().is_empty() {
                    return Ok(None);
                }

                Ok(Some(Lyrics::Plain(body.lyrics)))
            }
            404 => Ok(None),
            _ => Err(response.unexpected_status("lyrics.ovh")),
        }
    }
}

fn search_url(base_url: &str, artist: &str, title: &str) -> String {
    let encode = |s: &str| utf8_percent_encode(s, NON_ALPHANUMERIC).to_string();
    format!("{base_url}/v1/{}/{}", encode(artist), encode(title))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn check_url(base_url: &str, artist: &str, title: &str, expected: &str) {
        assert_eq!(
            search_url(base_url, artist, title),
            expected,
            "{artist:?} - {title:?} on {base_url}"
        );
    }

    #[test]
    fn a_search_names_the_artist_and_the_title_in_the_path() {
        check_url(
            DEFAULT_BASE_URL,
            "The Beatles",
            "Hey Jude",
            "https://api.lyrics.ovh/v1/The%20Beatles/Hey%20Jude",
        );
    }

    #[test]
    fn a_name_can_never_open_a_path_segment_of_its_own() {
        check_url(
            DEFAULT_BASE_URL,
            "AC/DC",
            "Back in Black",
            "https://api.lyrics.ovh/v1/AC%2FDC/Back%20in%20Black",
        );
        check_url(
            DEFAULT_BASE_URL,
            "../../etc",
            "passwd",
            "https://api.lyrics.ovh/v1/%2E%2E%2F%2E%2E%2Fetc/passwd",
        );
    }

    #[test]
    fn a_name_outside_ascii_travels_as_utf8() {
        check_url(
            DEFAULT_BASE_URL,
            "Björk",
            "Hyperballad",
            "https://api.lyrics.ovh/v1/Bj%C3%B6rk/Hyperballad",
        );
    }

    #[test]
    fn a_self_hosted_mirror_keeps_the_same_path() {
        check_url(
            "http://localhost:8080",
            "Artist",
            "Title",
            "http://localhost:8080/v1/Artist/Title",
        );
    }
}
