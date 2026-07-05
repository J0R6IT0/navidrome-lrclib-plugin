//! QQ Music provider ported from
//! https://github.com/ibratabian17/lyricsplus/blob/cookie/src/shared/services/qq.service.js

use crate::{
    config::{PluginConfig, ProviderParams},
    format::lrc,
    providers::{BROWSER_USER_AGENT, LyricsProvider},
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
use sha1::{Digest, Sha1};
use std::collections::HashMap;

mod qrc;

const ENDPOINT: &str = "https://u.y.qq.com/cgi-bin/musics.fcg";
const VERSION_CODE: u64 = 13_020_508;

const SEARCH_MODULE: &str = "music.search.SearchCgiService";
const SEARCH_METHOD: &str = "DoSearchForQQMusicMobile";
const LYRIC_MODULE: &str = "music.musichallSong.PlayLyricInfo";
const LYRIC_METHOD: &str = "GetPlayLyricInfo";

#[derive(Serialize)]
struct Common {
    wid: String,
    cv: u64,
    v: u64,
    #[serde(rename = "QIMEI36")]
    qimei36: &'static str,
    ct: &'static str,
    #[serde(rename = "tmeAppID")]
    tme_app_id: &'static str,
    format: &'static str,
    #[serde(rename = "inCharset")]
    in_charset: &'static str,
    #[serde(rename = "outCharset")]
    out_charset: &'static str,
    uid: &'static str,
}

impl Common {
    fn new() -> Self {
        Self {
            wid: random_guid(),
            cv: VERSION_CODE,
            v: VERSION_CODE,
            qimei36: "8888888888888888",
            ct: "11",
            tme_app_id: "qqmusic",
            format: "json",
            in_charset: "utf-8",
            out_charset: "utf-8",
            uid: "3931641530",
        }
    }
}

#[derive(Serialize)]
struct SearchParam<'a> {
    searchid: &'static str,
    query: &'a str,
    search_type: u32,
    num_per_page: u32,
    page_num: u32,
    highlight: u32,
    grp: u32,
}

#[derive(Serialize)]
struct LyricParam<'a> {
    crypt: u32,
    ct: u32,
    cv: u64,
    lrc_t: u32,
    qrc: u32,
    qrc_t: u32,
    roma: u32,
    roma_t: u32,
    trans: u32,
    trans_t: u32,
    #[serde(rename = "type")]
    type_: u32,
    #[serde(rename = "songMid")]
    song_mid: &'a str,
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
            .artists
            .first()
            .ok_or_else(|| Error::new("missing artist"))?
            .name
            .as_str();

        let query = format!("{} {first_artist}", track.title);
        let target_ms = (track.duration * 1000.0).round() as u64;

        let mid = match find_song(&query, target_ms, cfg.duration_tolerance_ms())? {
            Some(m) => m,
            None => return Ok(None),
        };

        let content = match fetch_lyric_content(&mid)? {
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

fn find_song(query: &str, target_ms: u64, tolerance_ms: u64) -> Result<Option<String>, Error> {
    let param = SearchParam {
        searchid: "12345678901234567",
        query,
        search_type: 0,
        num_per_page: 5,
        page_num: 1,
        highlight: 1,
        grp: 1,
    };

    let data = api_request(SEARCH_MODULE, SEARCH_METHOD, &param)?;

    let items = data
        .pointer("/body/item_song")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let song = items.iter().find(|item| {
        item["interval"]
            .as_u64()
            .map(|secs| (secs * 1000).abs_diff(target_ms) <= tolerance_ms)
            .unwrap_or(false)
    });

    Ok(song.and_then(|s| s["mid"].as_str()).map(|s| s.to_string()))
}

fn fetch_lyric_content(mid: &str) -> Result<Option<String>, Error> {
    let param = LyricParam {
        crypt: 1,
        ct: 11,
        cv: VERSION_CODE,
        lrc_t: 0,
        qrc: 1,
        qrc_t: 0,
        roma: 0,
        roma_t: 0,
        trans: 0,
        trans_t: 0,
        type_: 1,
        song_mid: mid,
    };

    let data = api_request(LYRIC_MODULE, LYRIC_METHOD, &param)?;

    let Some(encrypted) = data["lyric"].as_str().filter(|s| !s.is_empty()) else {
        return Ok(None);
    };

    let xml = match qrc::decrypt(encrypted) {
        Ok(xml) => xml,
        Err(e) => {
            warn!("qqmusic: failed to decrypt lyrics: {e}");
            return Ok(None);
        }
    };

    Ok(qrc::extract_lyric_content(&xml))
}

fn api_request<P: Serialize>(module: &str, method: &str, param: &P) -> Result<Value, Error> {
    let param_json = serde_json::to_string(param)
        .map_err(|e| Error::new(format!("qqmusic: failed to encode params: {e}")))?;
    let common_json = serde_json::to_string(&Common::new())
        .map_err(|e| Error::new(format!("qqmusic: failed to encode common params: {e}")))?;

    let body = format!(
        r#"{{"comm":{common_json},"{module}.{method}":{{"module":"{module}","method":"{method}","param":{param_json}}}}}"#
    );

    let signature = sign(&body);
    let url = format!("{ENDPOINT}?sign={signature}");

    let response = send_request(&url, body.into_bytes())?;

    if response.status_code != 200 {
        return Err(Error::new(format!(
            "qqmusic: API returned status {}",
            response.status_code
        )));
    }

    let parsed: Value = serde_json::from_slice(&response.body)
        .map_err(|e| Error::new(format!("qqmusic: failed to parse API response: {e}")))?;

    let result = &parsed[format!("{module}.{method}")];

    match result["code"].as_i64() {
        Some(0) => {}
        Some(code) => return Err(Error::new(format!("qqmusic: API error code {code}"))),
        None => return Err(Error::new("qqmusic: invalid API response structure")),
    }

    Ok(result
        .get("data")
        .cloned()
        .unwrap_or_else(|| result.clone()))
}

fn random_guid() -> String {
    let mut bytes = [0u8; 16];
    if getrandom::getrandom(&mut bytes).is_err() {
        return "0123456789ABCDEF0123456789ABCDEF".to_string();
    }

    let mut guid = String::with_capacity(32);
    for byte in bytes {
        guid.push_str(&format!("{byte:02X}"));
    }
    guid
}

fn sign(body: &str) -> String {
    let hash = Sha1::digest(body.as_bytes());
    let hex: Vec<u8> = format!("{hash:X}").into_bytes();

    let part1: String = [23, 14, 6, 36, 16, 7, 19]
        .iter()
        .map(|&i| hex[i] as char)
        .collect();
    let part2: String = [16, 1, 32, 12, 19, 27, 8, 5]
        .iter()
        .map(|&i| hex[i] as char)
        .collect();

    const SCRAMBLE: [u8; 20] = [
        89, 39, 179, 150, 218, 82, 58, 252, 177, 52, 186, 123, 120, 64, 242, 133, 143, 161, 121,
        179,
    ];
    let part3: Vec<u8> = (0..20).map(|i| SCRAMBLE[i] ^ hash[i]).collect();

    let mut b64 = STANDARD.encode(part3);
    b64.retain(|c| c != '/' && c != '+' && c != '=');

    format!("zzc{part1}{b64}{part2}").to_lowercase()
}

fn send_request(url: &str, body: Vec<u8>) -> Result<HTTPResponse, Error> {
    let mut headers = HashMap::new();
    headers.insert("Content-Type".into(), "application/json".into());
    headers.insert("Referer".into(), "https://y.qq.com/".into());
    headers.insert("Origin".into(), "https://y.qq.com".into());
    headers.insert("User-Agent".into(), BROWSER_USER_AGENT.into());

    http::send(HTTPRequest {
        url: url.into(),
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
    fn test_sign_is_deterministic_and_lowercase() {
        let body = r#"{"comm":{"wid":"x"},"a.b":{"module":"a","method":"b","param":{}}}"#;
        let s = sign(body);
        assert!(s.starts_with("zzc"));
        assert_eq!(s, s.to_lowercase());
        assert_eq!(s, sign(body));
        assert!(!s.contains('/') && !s.contains('+') && !s.contains('='));
    }

    #[test]
    fn test_random_guid_format() {
        let guid = random_guid();
        assert_eq!(guid.len(), 32);
        assert!(guid.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(guid, guid.to_uppercase());
    }

    #[test]
    fn test_search_param_field_order() {
        let json = serde_json::to_string(&SearchParam {
            searchid: "1",
            query: "q",
            search_type: 0,
            num_per_page: 5,
            page_num: 1,
            highlight: 1,
            grp: 1,
        })
        .unwrap();
        assert_eq!(
            json,
            r#"{"searchid":"1","query":"q","search_type":0,"num_per_page":5,"page_num":1,"highlight":1,"grp":1}"#
        );
    }

    #[test]
    fn test_common_field_order_and_renames() {
        let json = serde_json::to_string(&Common::new()).unwrap();
        assert!(json.starts_with(r#"{"wid":"#));
        assert!(json.contains(r#""QIMEI36":"#));
        assert!(json.contains(r#""tmeAppID":"qqmusic""#));
        assert!(json.contains(r#""inCharset":"utf-8""#));
    }
}
