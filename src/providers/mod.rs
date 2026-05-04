use crate::{
    LyricsKind,
    config::LyricsMode,
    providers::{lrclib::Lrclib, lyricsovh::LyricsOvh},
    registry::ProviderRegistry,
};
use nd_pdk::lyrics::{Error, TrackInfo};

mod lrclib;
mod lyricsovh;

const USER_AGENT: &str =
    "navidrome-lrclib-plugin/5.0.0 (https://github.com/J0R6IT0/navidrome-lrclib-plugin)";

pub fn register_providers(registry: &mut ProviderRegistry) {
    registry.register(Box::new(Lrclib));
    registry.register(Box::new(LyricsOvh));
}

pub trait LyricsProvider {
    fn id(&self) -> &'static str;
    fn fetch_lyrics(
        &self,
        track: &TrackInfo,
        lyrics_mode: LyricsMode,
    ) -> Result<Option<(String, LyricsKind)>, Error>;
}
