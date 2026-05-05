use crate::{
    config::PluginConfig,
    providers::{kugou::Kugou, lrclib::Lrclib, lyricsovh::LyricsOvh},
    registry::ProviderRegistry,
    types::LyricsType,
};
use nd_pdk::lyrics::{Error, TrackInfo};

mod kugou;
mod lrclib;
mod lyricsovh;

const USER_AGENT: &str =
    "navidrome-lrclib-plugin/5.0.0 (https://github.com/J0R6IT0/navidrome-lyrics-plugin)";

pub fn register_providers(registry: &mut ProviderRegistry) {
    registry.register(Box::new(Lrclib));
    registry.register(Box::new(LyricsOvh));
    registry.register(Box::new(Kugou));
}

pub trait LyricsProvider {
    fn id(&self) -> &'static str;
    fn fetch_lyrics(
        &self,
        track: &TrackInfo,
        cfg: &PluginConfig,
    ) -> Result<Option<(String, LyricsType)>, Error>;
}
