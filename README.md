# Navidrome Lyrics Plugin

A Navidrome plugin to fetch lyrics from multiple sources. Formerly Navidrome LRCLIB Plugin.

> [!IMPORTANT]
> The Navidrome WebUI does not display lyrics from plugins at the moment, you need a third party client in order to see them.

## Features

- Support for multiple providers (see below).
- Choose which lyrics types to fetch (LRC, enhanced LRC, TTML, SRT, YAML lyrics file, plain), with configurable priority.
- Sanitizes lyrics and optionally strips section labels ([Verse], [Chorus], etc.)
- Caching of lyrics for a configurable duration.
- Option to write lyrics to files, next to tracks or in custom paths.
- And much more!

## Installation

Make sure your Navidrome version is at least `v0.63.0`.

1. Download the latest `nd-lyrics.ndp` from the Releases page and place it in your plugins folder.
2. Add `nd-lyrics` to the `LyricsPriority` config option. See [here](https://www.navidrome.org/docs/usage/configuration/options/#:~:text=true-,LyricsPriority,-ND_LYRICSPRIORITY).

TIP: If you are using the "Write lyrics files" option, you can do `".ttml,.yaml,.yml,.elrc,.lrc,.srt,.txt,embedded,nd-lyrics"` so Navidrome reads the files
directly when available. This will only work if "Write to custom path" is disabled.

3. You may need to restart Navidrome for the plugin to be detected. Don't forget to enable the plugin and configure it to your liking.

## Providers

At this time, the following providers are available. Please report any issues you encounter while using them.

| Provider    | Type                 | Translations | Romanization | Notes                           |
| ----------- | -------------------- | :----------: | :----------: | ------------------------------- |
| LRCLIB      | plain,lrc,lyricsfile |      ✗       |      ✗       | Supports custom base URL        |
| lyrics.ovh  | plain                |      ✗       |      ✗       | Supports custom base URL        |
| lrcmux      | plain,lrc,elrc       |      ✗       |      ✗       | Supports custom base URL        |
| KuGou       | lrc,elrc             |      ✗       |      ✗       |                                 |
| NetEase     | lrc,elrc             |      ✗       |      ✗       |                                 |
| QQ Music    | lrc,elrc             |      ✗       |      ✗       |                                 |
| Apple Music | ttml                 |      ✓       |      ✓       | Requires an active subscription |
| stixoi.info | plain                |      ✗       |      ✗       | Greek lyrics archive            |

## Path variables

Custom paths to write lyrics files to can be composed using path variables.

Consider the following example:

```
_lyrics/{type}/{track:album}/{track:track_number:2} - {track:title}
```

This will be transfored into something like this:

```
<selected_library_root>/_lyrics/lrc/The Razors Edge/01 - Thunderstruck.lrc
```

Note that the extension is appended automatically based on the configuration and lyrics type.

| Variable             | Description                                                                                |
| -------------------- | ------------------------------------------------------------------------------------------ |
| {type}               | The type of lyrics ("plain", "lrc", "elrc", "ttml", "srt", "lyricsfile" or "instrumental") |
| {track:id}           | The ID of the track                                                                        |
| {track:title}        | The title of the track                                                                     |
| {track:album}        | The name of the album this track belongs to                                                |
| {track:artist}       | The artist of the track                                                                    |
| {track:album_artist} | The artist of the album this track belongs to                                              |
| {track:track_number} | The number of track in the album\*                                                         |
| {track:disc_number}  | The number of the disc in the album\*                                                      |

\* {track:track_number} and {track:disc_number} accept a padding argument to fill with 0s:

`{track:track_number:2}` will ensure that there are at least 2 digits. For example, track_number `1` will become `01`.
