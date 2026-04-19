## Navidrome LRCLIB Plugin

Simple Navidrome plugin to fetch lyrics from [lrclib.net](https://lrclib.net/).

> [!IMPORTANT]
> The Navidrome WebUI does not display lyrics from plugins at the moment, you need a third party client in order to see them.

## Features:

- Choose between synced and plain lyrics
- Fallback to search endpoint when direct lookup fails
- Cache lyrics in memory for a selected period of time
- Save lyrics as .lrc files.

## Installation

Make sure your Navidrome version is at least `v0.61.2`.

1. Download the latest `lrclib-lyrics.ndp` from the Releases page and place it in your plugins folder.
2. Add `lrclib-lyrics` to the `LyricsPriority` config option. See [here](https://www.navidrome.org/docs/usage/configuration/options/#:~:text=true-,LyricsPriority,-ND_LYRICSPRIORITY).

TIP: If you are using the "Write lyrics to .lrc files" option, you can do `".lrc,lrclib-lyrics,<others...>"` so Navidrome reads the files
directly when available.


3. You may need to restart Navidrome for the plugin to be detected.
