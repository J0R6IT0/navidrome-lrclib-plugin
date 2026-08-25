use crate::{
    config::{PluginConfig, ProviderParams},
    ext::TrackInfoExt,
    format::lrc,
    providers::{LyricsProvider, ProviderResult, error::ProviderError, http::Http},
    types::{Lyrics, LyricsKind},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use extism_pdk::{info, warn};
use nd_pdk::lyrics::TrackInfo;
use serde::Deserialize;

mod krc;

const SONG_SEARCH_URL: &str = "http://mobilecdn.kugou.com/api/v3/search/song";
const LYRICS_SEARCH_URL: &str = "http://lyrics.kugou.com/search";
const DOWNLOAD_URL: &str = "https://lyrics.kugou.com/download";

#[derive(Debug, Deserialize)]
struct SongSearchResponse {
    data: SongSearchData,
}

#[derive(Debug, Deserialize)]
struct SongSearchData {
    #[serde(default)]
    info: Vec<SongInfo>,
}

#[derive(Debug, Deserialize)]
struct SongInfo {
    hash: String,
    duration: Option<u64>,
    trans_param: Option<TransParam>,
}

#[derive(Debug, Deserialize)]
struct TransParam {
    #[serde(default)]
    language: String,
}

#[derive(Debug, Deserialize)]
struct LyricsSearchResponse {
    status: u32,
    #[serde(default)]
    candidates: Vec<Candidate>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    id: String,
    accesskey: String,
    #[serde(default)]
    krctype: i64,
}

#[derive(Debug, Deserialize)]
struct DownloadResponse {
    content: String,
}

pub struct Kugou;

impl Kugou {
    pub fn create(_params: &ProviderParams) -> Box<dyn LyricsProvider> {
        Box::new(Self)
    }
}

impl LyricsProvider for Kugou {
    fn supported_kinds(&self) -> &'static [LyricsKind] {
        &[LyricsKind::Lrc, LyricsKind::Elrc]
    }

    fn fetch_lyrics(
        &self,
        track: &TrackInfo,
        cfg: &PluginConfig,
    ) -> ProviderResult<Option<Lyrics>> {
        let keyword = format!("{} {}", track.title, track.first_artist().unwrap());
        let target_ms = track.duration_ms();

        let song = match find_song(&keyword, target_ms, cfg.duration_tolerance_ms())? {
            Some(s) => s,
            None => return Ok(None),
        };

        if song
            .trans_param
            .as_ref()
            .is_some_and(|p| p.language == "纯音乐")
        {
            info!("kugou: track is instrumental");
            return Ok(Some(Lyrics::Instrumental));
        }

        let candidate = match find_candidate(&song.hash, song.duration)? {
            Some(c) => c,
            None => return Ok(None),
        };

        for &kind in &cfg.lyrics_type_priority {
            match kind {
                LyricsKind::Elrc if candidate.krctype != 0 => {
                    let bytes = download_raw(&candidate.id, &candidate.accesskey, "krc")?;
                    match krc::to_enhanced_lrc(&bytes) {
                        Ok(elrc) if !elrc.trim().is_empty() => {
                            return Ok(Some(Lyrics::Elrc(elrc)));
                        }
                        Ok(_) => {}
                        Err(e) => warn!("kugou: failed to decode krc lyrics: {e}"),
                    }
                }
                LyricsKind::Lrc => {
                    let bytes = download_raw(&candidate.id, &candidate.accesskey, "lrc")?;
                    let lrc = String::from_utf8(bytes).map_err(|e| {
                        ProviderError::other(format!("lyrics content is not valid UTF-8: {e}"))
                    })?;

                    if lrc::is_instrumental(&lrc) {
                        info!("kugou: track is instrumental");
                        return Ok(Some(Lyrics::Instrumental));
                    }
                    if !lrc.trim().is_empty() {
                        return Ok(Some(Lyrics::Lrc(lrc)));
                    }
                }
                _ => {}
            }
        }

        Ok(None)
    }
}

fn find_song(keyword: &str, target_ms: u64, tolerance_ms: u64) -> ProviderResult<Option<SongInfo>> {
    let parsed: SongSearchResponse = get_json(
        Http::get(SONG_SEARCH_URL)
            .param("format", "json")
            .param("keyword", keyword)
            .param("page", "1")
            .param("pagesize", "10")
            .param("showtype", "1"),
        "the song search",
    )?;

    Ok(parsed.data.info.into_iter().find(|s| {
        s.duration
            .map(|secs| (secs * 1000).abs_diff(target_ms) <= tolerance_ms)
            .unwrap_or(false)
    }))
}

fn find_candidate(hash: &str, duration: Option<u64>) -> ProviderResult<Option<Candidate>> {
    let parsed: LyricsSearchResponse = get_json(
        Http::get(LYRICS_SEARCH_URL)
            .param("ver", "1")
            .param("man", "yes")
            .param("client", "mobi")
            .param("keyword", "")
            .param("duration", duration.unwrap_or(0).to_string())
            .param("hash", hash)
            .param("album_audio_id", ""),
        "the lyrics search",
    )?;

    if parsed.status != 200 {
        return Ok(None);
    }

    Ok(parsed.candidates.into_iter().next())
}

fn download_raw(id: &str, accesskey: &str, fmt: &str) -> ProviderResult<Vec<u8>> {
    let parsed: DownloadResponse = get_json(
        Http::get(DOWNLOAD_URL)
            .param("ver", "1")
            .param("client", "pc")
            .param("id", id)
            .param("accesskey", accesskey)
            .param("fmt", fmt)
            .param("charset", "utf8"),
        "the download",
    )?;

    STANDARD
        .decode(&parsed.content)
        .map_err(|e| ProviderError::other(format!("failed to decode lyrics content: {e}")))
}

fn get_json<T: serde::de::DeserializeOwned>(request: Http, what: &str) -> ProviderResult<T> {
    let response = request.browser().send()?;

    match response.status {
        200 => response.json(what),
        _ => Err(response.unexpected_status(what)),
    }
}
