## Navidrome LRCLIB Plugin

Simple Navidrome plugin to fetch lyrics from [lrclib.net](https://lrclib.net/).

> [!IMPORTANT]
> The Navidrome WebUI does not display lyrics from plugins at the moment, you need a third party client in order to see them.

Features:

- Choose between synced and plain lyrics
- Fallback to search endpoint when direct lookup fails
- Cache lyrics in memory for a selected period of time
- Save lyrics as .lrc files.\*

\*Due to API limitations, .lrc files are written inside a folder called `_lyrics` at the root of the selected library.

## Instalation

Make sure your Navidrome version is greater than `v0.61.0`.

1. Download the latest `lrclib-lyrics.ndp` from the Releases page and place it in your plugins folder.
2. Add `lrclib-lyrics` to the `LyricsPriority` config option. See [here](https://www.navidrome.org/docs/usage/configuration/options/#:~:text=true-,LyricsPriority,-ND_LYRICSPRIORITY).
3. You may need to restart Navidrome for the plugin to be detected.
