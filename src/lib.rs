use nd_pdk::{
    host::{
        config,
        http::{self, HTTPRequest, HTTPResponse},
    },
    lyrics::{Error as LyricsError, GetLyricsRequest, GetLyricsResponse, Lyrics, LyricsText},
};
use serde::Deserialize;
use std::collections::HashMap;

const USER_AGENT: &str =
    "navidrome-lrclib-plugin/1.1.1 (https://github.com/J0R6IT0/navidrome-lrclib-plugin)";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LyricsRecord {
    synced_lyrics: Option<String>,
    plain_lyrics: Option<String>,
    duration: f32,
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

        let fetch_synced = config::get("fetchSyncedLyrics")
            .map_err(|e| LyricsError::new(e.to_string()))?
            .unwrap_or_else(|| "true".into())
            == "true";

        let use_search_fallback = config::get("useSearchFallback")
            .map_err(|e| LyricsError::new(e.to_string()))?
            .unwrap_or_else(|| "true".into())
            == "true";

        if let Some(record) = fetch_lyrics(&all_artists, title, Some(album), &duration)? {
            if let Some(text) = pick_text(record, fetch_synced) {
                return Ok(lyrics_response(text));
            }
        }

        if use_search_fallback {
            let query = format!("{} {}", first_artist, title);
            if let Some(record) = search_lyrics(&query, track.duration)? {
                if let Some(text) = pick_text(record, fetch_synced) {
                    return Ok(lyrics_response(text));
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
) -> Result<Option<LyricsRecord>, LyricsError> {
    let mut params = vec![
        ("artist_name", artist),
        ("track_name", title),
        ("duration", duration),
    ];
    if let Some(album) = album {
        params.push(("album_name", album));
    }

    let query = serde_urlencoded::to_string(params).map_err(|e| LyricsError::new(e.to_string()))?;

    let response = send_request(&format!("https://lrclib.net/api/get?{}", query))?;

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

fn search_lyrics(q: &str, duration: f32) -> Result<Option<LyricsRecord>, LyricsError> {
    let query =
        serde_urlencoded::to_string([("q", q)]).map_err(|e| LyricsError::new(e.to_string()))?;

    let response = send_request(&format!("https://lrclib.net/api/search?{}", query))?;

    if response.status_code != 200 {
        return Err(LyricsError::new(format!(
            "lrclib search returned status {}",
            response.status_code
        )));
    }

    let records: Vec<LyricsRecord> =
        serde_json::from_slice(&response.body).map_err(|e| LyricsError::new(e.to_string()))?;

    Ok(records
        .into_iter()
        .find(|r| (r.duration - duration).abs() <= 2.))
}

fn send_request(url: &str) -> Result<HTTPResponse, LyricsError> {
    let mut headers = HashMap::new();
    headers.insert("User-Agent".into(), USER_AGENT.into());

    http::send(HTTPRequest {
        url: url.into(),
        method: "GET".into(),
        headers,
        no_follow_redirects: false,
        body: Vec::new(),
        timeout_ms: 10_000,
    })
    .map_err(|e| LyricsError::new(e.to_string()))?
    .ok_or_else(|| LyricsError::new("Empty HTTP response"))
}

fn pick_text(record: LyricsRecord, use_synced: bool) -> Option<String> {
    let synced = record.synced_lyrics.filter(|s| !s.is_empty());
    let plain = record.plain_lyrics.filter(|s| !s.is_empty());

    if use_synced { synced.or(plain) } else { plain }
}

fn lyrics_response(text: String) -> GetLyricsResponse {
    GetLyricsResponse {
        lyrics: vec![LyricsText {
            lang: "xxx".into(),
            text,
        }],
    }
}
