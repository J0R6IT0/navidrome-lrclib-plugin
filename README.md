# Navidrome Lyrics Plugin

A Navidrome plugin to fetch lyrics from multiple sources.

> [!IMPORTANT]
> The Navidrome WebUI does not display lyrics from plugins at the moment, you need a third party client in order to see them.

## Features

- Support for multiple providers (see below).
- Choose to fetch synced lyrics, plain or both, with configurable priority.
- Sanitizes lyrics and optionally strips section labels ([Verse], [Chorus], etc.)
- In-memory caching of lyrics for a configurable duration.
- Option to write lyrics to files, next to tracks or in custom paths.

## Installation

Make sure your Navidrome version is at least `v0.61.2`.

1. Download the latest `lyrics.ndp` from the Releases page and place it in your plugins folder.
2. Add `nd-lyrics` to the `LyricsPriority` config option. See [here](https://www.navidrome.org/docs/usage/configuration/options/#:~:text=true-,LyricsPriority,-ND_LYRICSPRIORITY).

TIP: If you are using the "Write lyrics to .lrc files" option, you can do `".lrc,nd-lyrics,<others...>"` so Navidrome reads the files
directly when available.

3. You may need to restart Navidrome for the plugin to be detected.

## Providers

| Provider   | Mode           |
| ---------- | -------------- |
| lrclib     | plain + synced |
| lyrics.ovh | plain          |
| kugou      | synced         |

## Path variables
