## Navidrome LRCLIB Plugin

Simple Navidrome plugin to fetch lyrics from [lrclib.net](https://lrclib.net/).

Features:

- Choose between synced and plain lyrics
- Fallback to search endpoint when direct lookup fails
- (Planned) Inject "♪" symbol in long instrumental sections

## Instalation

Make sure your Navidrome version is greater than `v0.61.0`.

1. Download the latest `lrclib-lyrics.ndp` from the Releases page and place it in your plugins folder.
2. Add `lrclib-lyrics` to the `LyricsPriority` config option. See [here](https://www.navidrome.org/docs/usage/configuration/options/#:~:text=true-,LyricsPriority,-ND_LYRICSPRIORITY)
3. You may need to restart Navidrome for the plugin to be detected.
