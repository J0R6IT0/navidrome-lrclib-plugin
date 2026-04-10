mod config;
mod lrclib;
mod storage;

use nd_pdk::lyrics::{
    Error as LyricsError, GetLyricsRequest, GetLyricsResponse, Lyrics, LyricsText,
};

use config::PluginConfig;
use lrclib::fetch_lyrics_text;
use storage::LyricsStorage;

#[derive(Default)]
struct Plugin;

nd_pdk::register_lyrics!(Plugin);

impl Lyrics for Plugin {
    fn get_lyrics(&self, req: GetLyricsRequest) -> Result<GetLyricsResponse, LyricsError> {
        let track = req.track;
        let cfg = PluginConfig::load()?;

        let storage = cfg.write_lyrics.then(|| LyricsStorage::new()).transpose()?;

        if let Some(ref s) = storage {
            if let Some(cached) = s.read(&track.id, cfg.fetch_synced)? {
                return Ok(make_response(cached));
            }
        }

        let (text, kind) =
            fetch_lyrics_text(&track, &cfg)?.ok_or_else(|| LyricsError::new("no lyrics found"))?;

        if let Some(ref s) = storage {
            s.write(&track.id, &text, kind)?;
        }

        Ok(make_response(text))
    }
}

fn make_response(text: String) -> GetLyricsResponse {
    GetLyricsResponse {
        lyrics: vec![LyricsText {
            lang: "xxx".into(),
            text,
        }],
    }
}
