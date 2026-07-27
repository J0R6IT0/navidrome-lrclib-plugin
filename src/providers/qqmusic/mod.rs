//! QQ Music provider, modelled on the QQ Music Lite Android client as
//! implemented by https://github.com/chenmozhijin/LDDC (LDDC/core/api/lyrics/qm.py).

use crate::{
    config::{PluginConfig, ProviderParams},
    ext::TrackInfoExt,
    format::lrc,
    providers::LyricsProvider,
    types::{Lyrics, LyricsKind},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use extism_pdk::{info, warn};
use nd_pdk::{
    host::http::{self, HTTPRequest, HTTPResponse},
    lyrics::{Error, TrackInfo},
};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

mod qrc;

const ENDPOINT: &str = "https://u.y.qq.com/cgi-bin/musicu.fcg";
const CLIENT_USER_AGENT: &str = "okhttp/3.14.9";

const SEARCH_MODULE: &str = "music.search.SearchCgiService";
const SEARCH_METHOD: &str = "DoSearchForQQMusicLite";
const LYRIC_MODULE: &str = "music.musichallSong.PlayLyricInfo";
const LYRIC_METHOD: &str = "GetPlayLyricInfo";

/// Client identity block sent with every request. `ct` 11 plus the
/// `qqmusiclight` app id is what marks this as the Android Lite client.
#[derive(Serialize)]
struct Common {
    ct: u32,
    cv: &'static str,
    v: &'static str,
    os_ver: &'static str,
    phonetype: &'static str,
    rom: &'static str,
    #[serde(rename = "tmeAppID")]
    tme_app_id: &'static str,
    nettype: &'static str,
    udid: &'static str,
}

impl Common {
    fn new() -> Self {
        Self {
            ct: 11,
            cv: "1003006",
            v: "1003006",
            os_ver: "15",
            phonetype: "24122RKC7C",
            rom: "Redmi/miro/miro:15/AE3A.240806.005/OS2.0.105.0.VOMCNXM:user/release-keys",
            tme_app_id: "qqmusiclight",
            nettype: "NETWORK_WIFI",
            udid: "0",
        }
    }
}

#[derive(Serialize)]
struct SearchParam<'a> {
    search_id: String,
    remoteplace: &'static str,
    query: &'a str,
    search_type: u32,
    num_per_page: u32,
    page_num: u32,
    highlight: u32,
    nqc_flag: u32,
    page_id: u32,
    grp: u32,
}

/// The lyric endpoint identifies the track by numeric song id, and cross-checks
/// it against the base64-encoded title/artist/album and the duration in seconds.
#[derive(Serialize)]
struct LyricParam {
    #[serde(rename = "albumName")]
    album_name: String,
    crypt: u32,
    ct: u32,
    cv: u32,
    interval: u64,
    lrc_t: u32,
    qrc: u32,
    qrc_t: u32,
    roma: u32,
    roma_t: u32,
    #[serde(rename = "singerName")]
    singer_name: String,
    #[serde(rename = "songID")]
    song_id: u64,
    #[serde(rename = "songName")]
    song_name: String,
    trans: u32,
    trans_t: u32,
    #[serde(rename = "type")]
    type_: u32,
}

struct Song {
    id: u64,
    title: String,
    artist: String,
    album: String,
    interval_secs: u64,
}

pub struct QQMusic;

impl QQMusic {
    pub fn create(_params: &ProviderParams) -> Box<dyn LyricsProvider> {
        Box::new(Self)
    }
}

impl LyricsProvider for QQMusic {
    fn supported_kinds(&self) -> &'static [LyricsKind] {
        &[LyricsKind::Lrc, LyricsKind::Elrc]
    }

    fn fetch_lyrics(&self, track: &TrackInfo, cfg: &PluginConfig) -> Result<Option<Lyrics>, Error> {
        let first_artist = track
            .first_artist()
            .ok_or_else(|| Error::new("missing artist"))?;

        let query = format!("{} {first_artist}", track.title);
        let target_ms = (track.duration * 1000.0).round() as u64;

        let song = match find_song(&query, target_ms, cfg.duration_tolerance_ms())? {
            Some(s) => s,
            None => return Ok(None),
        };

        let content = match fetch_lyric_content(&song)? {
            Some(c) => c,
            None => return Ok(None),
        };

        let is_qrc = qrc::is_qrc(&content);

        if !is_qrc && lrc::is_instrumental(&content) {
            info!("qqmusic: track is instrumental");
            return Ok(Some(Lyrics::Instrumental));
        }

        for &kind in cfg.resolve_order() {
            match kind {
                LyricsKind::Elrc if is_qrc => {
                    let elrc = qrc::to_enhanced_lrc(&content);
                    if !elrc.trim().is_empty() {
                        return Ok(Some(Lyrics::Elrc(elrc)));
                    }
                }
                LyricsKind::Lrc => {
                    let plain = if is_qrc {
                        qrc::to_lrc(&content)
                    } else {
                        content.clone()
                    };

                    if !plain.trim().is_empty() {
                        return Ok(Some(Lyrics::Lrc(plain)));
                    }
                }
                _ => {}
            }
        }

        Ok(None)
    }
}

fn find_song(query: &str, target_ms: u64, tolerance_ms: u64) -> Result<Option<Song>, Error> {
    let param = SearchParam {
        search_id: random_search_id(),
        remoteplace: "search.android.keyboard",
        query,
        search_type: 0,
        num_per_page: 5,
        page_num: 1,
        highlight: 0,
        nqc_flag: 0,
        page_id: 1,
        grp: 1,
    };

    let data = api_request(SEARCH_MODULE, SEARCH_METHOD, &param)?;

    let items = data
        .pointer("/body/item_song")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    if items.is_empty() {
        info!("qqmusic: search returned no results for '{query}'");
        return Ok(None);
    }

    let matched = items.iter().find(|item| {
        item["interval"]
            .as_u64()
            .map(|secs| (secs * 1000).abs_diff(target_ms) <= tolerance_ms)
            .unwrap_or(false)
    });

    Ok(matched.and_then(parse_song))
}

fn parse_song(item: &Value) -> Option<Song> {
    let artist = item["singer"]
        .as_array()
        .map(|singers| {
            singers
                .iter()
                .filter_map(|s| s["name"].as_str())
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_default();

    Some(Song {
        id: item["id"].as_u64()?,
        title: item["title"].as_str().unwrap_or_default().to_string(),
        artist,
        album: item
            .pointer("/album/name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        interval_secs: item["interval"].as_u64().unwrap_or(0),
    })
}

fn fetch_lyric_content(song: &Song) -> Result<Option<String>, Error> {
    let param = LyricParam {
        album_name: STANDARD.encode(&song.album),
        crypt: 1,
        ct: 19,
        cv: 2111,
        interval: song.interval_secs,
        lrc_t: 0,
        qrc: 1,
        qrc_t: 0,
        roma: 0,
        roma_t: 0,
        singer_name: STANDARD.encode(&song.artist),
        song_id: song.id,
        song_name: STANDARD.encode(&song.title),
        trans: 0,
        trans_t: 0,
        type_: 0,
    };

    let data = api_request(LYRIC_MODULE, LYRIC_METHOD, &param)?;

    let Some(encrypted) = data["lyric"].as_str().filter(|s| !s.is_empty()) else {
        info!("qqmusic: song {} has no lyrics attached", song.id);
        return Ok(None);
    };

    let xml = match qrc::decrypt(encrypted) {
        Ok(xml) => xml,
        Err(e) => {
            warn!(
                "qqmusic: failed to decrypt lyrics for song {} ({} hex chars): {e}",
                song.id,
                encrypted.len()
            );
            return Ok(None);
        }
    };

    let content = qrc::extract_lyric_content(&xml);

    if content.is_none() {
        warn!(
            "qqmusic: decrypted payload for song {} carries no LyricContent",
            song.id
        );
    }

    Ok(content)
}

fn api_request<P: Serialize>(module: &str, method: &str, param: &P) -> Result<Value, Error> {
    let body = serde_json::to_vec(&serde_json::json!({
        "comm": Common::new(),
        "request": {
            "module": module,
            "method": method,
            "param": param,
        },
    }))
    .map_err(|e| Error::new(format!("qqmusic: failed to encode request: {e}")))?;

    let response = send_request(body)?;

    if response.status_code != 200 {
        return Err(Error::new(format!(
            "qqmusic: API returned status {}",
            response.status_code
        )));
    }

    let parsed: Value = serde_json::from_slice(&response.body)
        .map_err(|e| Error::new(format!("qqmusic: failed to parse API response: {e}")))?;

    // The gateway reports a transport-level code at the top and a per-module code
    // inside the echoed `request` object. Both must be zero.
    check_code(&parsed, module, method, "gateway")?;
    let result = &parsed["request"];
    check_code(result, module, method, "module")?;

    Ok(result
        .get("data")
        .cloned()
        .unwrap_or_else(|| result.clone()))
}

fn check_code(value: &Value, module: &str, method: &str, level: &str) -> Result<(), Error> {
    match value["code"].as_i64() {
        Some(0) => Ok(()),
        // 2001 means the endpoint rejected our client identity, either the `comm` block
        // or the headers no longer match the client it expects.
        Some(2001) => Err(Error::new(format!(
            "qqmusic: {module}.{method} rejected the client identity ({level} error code 2001), the 'comm' block or request headers are stale"
        ))),
        Some(code) => Err(Error::new(format!(
            "qqmusic: {module}.{method} returned {level} error code {code}"
        ))),
        None => Err(Error::new(format!(
            "qqmusic: {module}.{method} response missing {level} 'code' field"
        ))),
    }
}

fn random_search_id() -> String {
    let mut bytes = [0u8; 8];
    let rnd = if getrandom::getrandom(&mut bytes).is_ok() {
        u64::from_le_bytes(bytes)
    } else {
        0x1234_5678_9abc_def0
    };

    let prefix = (rnd % 20) + 1;
    let middle = (rnd >> 8) % 4_194_304;
    let low = (rnd >> 32) % 86_400_000;

    (prefix * 18_014_398_509_481_984 + middle * 4_294_967_296 + low).to_string()
}

fn send_request(body: Vec<u8>) -> Result<HTTPResponse, Error> {
    let mut headers = HashMap::new();
    headers.insert("Content-Type".into(), "application/json".into());
    headers.insert("Cookie".into(), "tmeLoginType=-1;".into());
    headers.insert("User-Agent".into(), CLIENT_USER_AGENT.into());

    http::send(HTTPRequest {
        url: ENDPOINT.into(),
        method: "POST".into(),
        headers,
        no_follow_redirects: false,
        body,
        timeout_ms: 15_000,
    })
    .map_err(|e| Error::new(format!("qqmusic: HTTP request failed: {e}")))?
    .ok_or_else(|| Error::new("qqmusic: received empty HTTP response"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_common_identifies_the_android_lite_client() {
        let json = serde_json::to_string(&Common::new()).unwrap();
        assert!(json.contains(r#""ct":11"#));
        assert!(json.contains(r#""tmeAppID":"qqmusiclight""#));
        assert!(json.contains(r#""cv":"1003006""#));
    }

    #[test]
    fn test_random_search_id_is_numeric() {
        let id = random_search_id();
        assert!(id.chars().all(|c| c.is_ascii_digit()));
        assert!(id.parse::<u64>().is_ok());
    }

    #[test]
    fn test_parse_song_joins_singers_and_reads_album() {
        let item = serde_json::json!({
            "id": 7239095,
            "mid": "004D8vna0lUZbr",
            "title": "Bohemian Rhapsody",
            "singer": [{"name": "Queen"}, {"name": "David Bowie"}, {"name": ""}],
            "album": {"name": "A Night At The Opera"},
            "interval": 354,
        });

        let song = parse_song(&item).unwrap();
        assert_eq!(song.id, 7239095);
        assert_eq!(song.title, "Bohemian Rhapsody");
        assert_eq!(song.artist, "Queen/David Bowie");
        assert_eq!(song.album, "A Night At The Opera");
        assert_eq!(song.interval_secs, 354);
    }

    #[test]
    fn test_parse_song_without_id_is_skipped() {
        let item = serde_json::json!({ "title": "No id", "interval": 100 });
        assert!(parse_song(&item).is_none());
    }

    #[test]
    fn test_lyric_param_base64_encodes_names() {
        let json = serde_json::to_string(&LyricParam {
            album_name: STANDARD.encode("A Night At The Opera"),
            crypt: 1,
            ct: 19,
            cv: 2111,
            interval: 354,
            lrc_t: 0,
            qrc: 1,
            qrc_t: 0,
            roma: 0,
            roma_t: 0,
            singer_name: STANDARD.encode("Queen"),
            song_id: 7239095,
            song_name: STANDARD.encode("Bohemian Rhapsody"),
            trans: 0,
            trans_t: 0,
            type_: 0,
        })
        .unwrap();

        assert!(json.contains(r#""singerName":"UXVlZW4=""#));
        assert!(json.contains(r#""songID":7239095"#));
        assert!(json.contains(r#""type":0"#));
    }
}
