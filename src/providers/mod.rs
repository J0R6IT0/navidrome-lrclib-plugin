use crate::{
    config::PluginConfig,
    providers::{
        applemusic::AppleMusic, kugou::Kugou, lrclib::Lrclib, lyricsovh::LyricsOvh,
        netease::NetEase, qqmusic::QQMusic,
    },
    registry::ProviderRegistry,
    types::{Lyrics, LyricsKind},
};
use nd_pdk::lyrics::{Error, TrackInfo};

mod applemusic;
mod kugou;
mod lrclib;
mod lyricsovh;
mod netease;
mod qqmusic;

const USER_AGENT: &str = concat!(
    "navidrome-lyrics-plugin/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/J0R6IT0/navidrome-lyrics-plugin)"
);

const BROWSER_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 15.7; rv:150.0) Gecko/20100101 Firefox/150.0";

pub fn register_providers(registry: &mut ProviderRegistry) {
    registry.register("lrclib", Lrclib::create);
    registry.register("lyrics.ovh", LyricsOvh::create);
    registry.register("kugou", Kugou::create);
    registry.register("netease", NetEase::create);
    registry.register("qqmusic", QQMusic::create);
    registry.register("applemusic", AppleMusic::create);
}

pub trait LyricsProvider {
    fn supported_kinds(&self) -> &'static [LyricsKind];

    fn fetch_lyrics(&self, track: &TrackInfo, cfg: &PluginConfig) -> Result<Option<Lyrics>, Error>;
}
