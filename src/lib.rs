use nd_pdk::lyrics::{
    Error as LyricsError, GetLyricsRequest, GetLyricsResponse, Lyrics as LyricsPlugin,
};

mod cache;
mod config;
mod engine;
mod fetch;
mod format;
mod providers;
mod registry;
mod selection;
mod types;
mod writing;

#[derive(Default)]
struct Plugin;

nd_pdk::register_lyrics!(Plugin);

impl LyricsPlugin for Plugin {
    fn get_lyrics(&self, req: GetLyricsRequest) -> Result<GetLyricsResponse, LyricsError> {
        engine::get_lyrics(req.track)
    }
}
