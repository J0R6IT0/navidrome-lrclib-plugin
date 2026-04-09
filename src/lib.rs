use nd_pdk::{
    host::{
        config,
        http::{self, HTTPRequest},
    },
    lyrics::{Error as LyricsError, GetLyricsRequest, GetLyricsResponse, Lyrics, LyricsText},
};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    synced_lyrics: String,
    plain_lyrics: String,
}

#[derive(Default)]
struct Plugin;

nd_pdk::register_lyrics!(Plugin);

impl Lyrics for Plugin {
    fn get_lyrics(&self, req: GetLyricsRequest) -> Result<GetLyricsResponse, LyricsError> {
        let track = req.track;

        let first_artist = track
            .artists
            .first()
            .ok_or_else(|| LyricsError::new("Missing artist"))?
            .name
            .as_str();

        let all_artists = track
            .artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let title = track.title.as_str();
        let album = track.album.as_str();
        let duration = track.duration.round().to_string();

        let use_synced = config::get("useSyncedLyrics")
            .map_err(|e| LyricsError::new(e.to_string()))?
            .unwrap_or_else(|| "true".into())
            == "true";

        let attempts: &[(&str, bool)] = &[
            (&all_artists, true),
            (first_artist, true),
            (&all_artists, false),
            (first_artist, false),
        ];

        for (artist, include_album) in attempts {
            let album_param = include_album.then_some(album);
            if let Some(response) = fetch_lyrics(artist, title, album_param, &duration)? {
                if let Some(text) = pick_text(response, use_synced) {
                    return Ok(GetLyricsResponse {
                        lyrics: vec![LyricsText {
                            lang: "xxx".into(),
                            text,
                        }],
                    });
                }
            }
        }

        Err(LyricsError::new("No lyrics found"))
    }
}

fn fetch_lyrics(
    artist: &str,
    title: &str,
    album: Option<&str>,
    duration: &str,
) -> Result<Option<Response>, LyricsError> {
    let mut params = vec![
        ("artist_name", artist),
        ("track_name", title),
        ("duration", duration),
    ];
    if let Some(album) = album {
        params.push(("album_name", album));
    }

    let query = serde_urlencoded::to_string(params).map_err(|e| LyricsError::new(e.to_string()))?;

    let request = HTTPRequest {
        url: format!("https://lrclib.net/api/get?{}", query),
        method: "GET".into(),
        headers: HashMap::new(),
        no_follow_redirects: false,
        body: Vec::new(),
        timeout_ms: 10_000,
    };

    let response = http::send(request)
        .map_err(|e| LyricsError::new(e.to_string()))?
        .ok_or_else(|| LyricsError::new("Empty HTTP response"))?;

    match response.status_code {
        200 => {
            let parsed = serde_json::from_slice(&response.body)
                .map_err(|e| LyricsError::new(e.to_string()))?;
            Ok(Some(parsed))
        }
        404 => Ok(None),
        code => Err(LyricsError::new(format!("lrclib returned status {}", code))),
    }
}

fn pick_text(response: Response, use_synced: bool) -> Option<String> {
    let text = if use_synced && !response.synced_lyrics.is_empty() {
        response.synced_lyrics
    } else {
        response.plain_lyrics
    };
    (!text.is_empty()).then_some(text)
}
