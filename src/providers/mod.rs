use crate::{
    LyricsKind,
    config::{LyricsMode, LyricsProviderId},
    providers::{lrclib::LrclibProvider, lyrics_ovh::LyricsOvhProvider},
    registry::ProviderRegistry,
};
use nd_pdk::lyrics::{Error, TrackInfo};

mod lrclib;
mod lyrics_ovh;

const USER_AGENT: &str =
    "navidrome-lrclib-plugin/4.0.0 (https://github.com/J0R6IT0/navidrome-lrclib-plugin)";

pub fn register_providers(registry: &mut ProviderRegistry) {
    registry.register(Box::new(LrclibProvider));
    registry.register(Box::new(LyricsOvhProvider));
}

pub trait LyricsProvider {
    fn id(&self) -> LyricsProviderId;
    fn fetch_lyrics(
        &self,
        track: &TrackInfo,
        lyrics_mode: LyricsMode,
    ) -> Result<Option<(String, LyricsKind)>, Error>;
}
