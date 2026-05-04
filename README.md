# Navidrome LRCLIB Lyrics Plugin

A Navidrome plugin to fetch lyrics from [lrclib.net](https://lrclib.net/).

> [!IMPORTANT]
> The Navidrome WebUI does not display lyrics from plugins at the moment, you need a third party client in order to see them.

## Features

- Multiple lyrics modes: Plain, synchronized, or both combined.
- Flexible search system that tries both direct lookup and search.
- Fallback to [lyrics.ovh](https://lyrics.ovh/) if no lyrics are found.
- In-memory caching of lyrics for a configurable duration.
- Option to save lyrics as `.lrc` files.

## Available providers

| Provider   | Mode           |
|------------|----------------|
| lrclib     | plain + synced |
| lyrics.ovh | plain          |

## Installation

Make sure your Navidrome version is at least `v0.61.2`.

1. Download the latest `lrclib-lyrics.ndp` from the Releases page and place it in your plugins folder.
2. Add `lrclib-lyrics` to the `LyricsPriority` config option. See [here](https://www.navidrome.org/docs/usage/configuration/options/#:~:text=true-,LyricsPriority,-ND_LYRICSPRIORITY).

TIP: If you are using the "Write lyrics to .lrc files" option, you can do `".lrc,lrclib-lyrics,<others...>"` so Navidrome reads the files
directly when available.

3. You may need to restart Navidrome for the plugin to be detected.
