use crate::{
    config::PluginConfig,
    providers::{kugou::Kugou, lrclib::Lrclib, lyricsovh::LyricsOvh},
    registry::ProviderRegistry,
    types::Lyrics,
};
use nd_pdk::lyrics::{Error, TrackInfo};

mod kugou;
mod lrclib;
mod lyricsovh;

const USER_AGENT: &str = concat!(
    "navidrome-lyrics-plugin/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/J0R6IT0/navidrome-lyrics-plugin)"
);

const FIREFOX_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 15.7; rv:150.0) Gecko/20100101 Firefox/150.0";

pub fn register_providers(registry: &mut ProviderRegistry) {
    registry.register("lrclib", Lrclib::create);
    registry.register("lyrics.ovh", LyricsOvh::create);
    registry.register("kugou", Kugou::create);
}

pub trait LyricsProvider {
    fn fetch_lyrics(&self, track: &TrackInfo, cfg: &PluginConfig) -> Result<Option<Lyrics>, Error>;
}
