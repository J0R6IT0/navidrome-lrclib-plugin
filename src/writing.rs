use crate::{
    config::PluginConfig,
    types::{Lyrics, LyricsKind},
};
use nd_pdk::{
    host::library,
    lyrics::{Error as LyricsError, TrackInfo},
};
use std::{fs, path::PathBuf};

pub fn write(track: &TrackInfo, lyrics: &Lyrics, cfg: &PluginConfig) -> Result<(), LyricsError> {
    let path = if cfg.write_to_specific_folder {
        resolve_custom_path(track, lyrics.kind(), cfg)?
    } else {
        resolve_sidecar_path(track, lyrics.kind(), cfg)?
    };

    if path.exists() && !cfg.overwrite_lyrics {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| LyricsError::new(format!("failed to create lyrics directory: {e}")))?;
    }

    fs::write(&path, lyrics.text(cfg).as_bytes())
        .map_err(|e| LyricsError::new(format!("failed to write lyrics file: {e}")))?;

    Ok(())
}

fn resolve_custom_path(
    track: &TrackInfo,
    kind: LyricsKind,
    cfg: &PluginConfig,
) -> Result<PathBuf, LyricsError> {
    let library_id = cfg.write_to_specific_folder_library_id.ok_or_else(|| {
        LyricsError::new("a library ID is required when write to custom path is enabled")
    })?;

    let lib = library::get_library(library_id)
        .map_err(|e| LyricsError::new(format!("failed to query library {library_id}: {e}")))?
        .ok_or_else(|| LyricsError::new(format!("library with ID {library_id} not found")))?;

    let ext = match kind {
        LyricsKind::Synced => &cfg.synced_extension,
        LyricsKind::Plain => &cfg.plain_extension,
        LyricsKind::Instrumental => &cfg.instrumental_extension,
    };

    let relative_path = process_template(&cfg.write_to_specific_folder_template, track, kind, ext);

    Ok(PathBuf::from(lib.mount_point).join(relative_path))
}

fn resolve_sidecar_path(
    track: &TrackInfo,
    kind: LyricsKind,
    cfg: &PluginConfig,
) -> Result<PathBuf, LyricsError> {
    if track.path.is_empty() {
        return Err(LyricsError::new("track path is empty"));
    }

    let mut path = resolve_track_path(track)?
        .ok_or_else(|| LyricsError::new("could not resolve track path to a valid local file"))?;

    let ext = match kind {
        LyricsKind::Synced => &cfg.synced_extension,
        LyricsKind::Plain => &cfg.plain_extension,
        LyricsKind::Instrumental => &cfg.instrumental_extension,
    };

    path.set_extension(ext);
    Ok(path)
}

fn resolve_track_path(track: &TrackInfo) -> Result<Option<PathBuf>, LyricsError> {
    let libraries = library::get_all_libraries()
        .map_err(|e| LyricsError::new(format!("failed to query libraries: {e}")))?;

    for lib in libraries {
        let path = PathBuf::from(lib.mount_point).join(&track.path);
        if path.exists() {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

fn process_template(template: &str, track: &TrackInfo, kind: LyricsKind, ext: &str) -> PathBuf {
    let type_str = match kind {
        LyricsKind::Synced => "synced",
        LyricsKind::Plain => "plain",
        LyricsKind::Instrumental => "instrumental",
    };

    let mut path = PathBuf::new();

    for component in template.split(&['/', '\\']) {
        if component.is_empty() {
            continue;
        }

        let mut substituted = component
            .replace("{type}", type_str)
            .replace("{track:id}", &track.id)
            .replace("{track:title}", &track.title)
            .replace("{track:album}", &track.album)
            .replace("{track:artist}", &track.artist)
            .replace("{track:album_artist}", &track.album_artist);

        substituted = replace_padded_variable(
            &substituted,
            "track:track_number",
            &track.track_number.to_string(),
        );
        substituted = replace_padded_variable(
            &substituted,
            "track:disc_number",
            &track.disc_number.to_string(),
        );

        let sanitized = sanitize_path_segment(&substituted);

        if !sanitized.is_empty() {
            path.push(sanitized);
        }
    }

    if template.ends_with('/') || template.ends_with('\\') {
        path.push("unknown");
    }

    if path.as_os_str().is_empty() {
        path.push("unknown");
    }

    let path_str = path.as_os_str().to_string_lossy().to_string();

    if path_str.ends_with(&format!(".{ext}")) {
        return PathBuf::from(path_str);
    }

    let clean_path = path_str.trim_end_matches('.');

    if clean_path.is_empty() {
        PathBuf::from(format!("unknown.{ext}"))
    } else {
        PathBuf::from(format!("{clean_path}.{ext}"))
    }
}

fn replace_padded_variable(template: &str, var_name: &str, value: &str) -> String {
    let mut result = template.to_string();
    let search_prefix = format!("{{{var_name}");

    let mut start = 0;
    while let Some(pos) = result[start..].find(&search_prefix) {
        let abs_pos = start + pos;
        let remaining = &result[abs_pos..];

        if let Some(end_offset) = remaining.find('}') {
            let var_end = abs_pos + end_offset + 1;
            let full_var = &result[abs_pos..var_end];

            let inner = &full_var[1..full_var.len() - 1];
            let width = if let Some(padding_str) = inner.strip_prefix(&format!("{var_name}:")) {
                padding_str.parse::<usize>().unwrap_or(0)
            } else {
                0
            };

            let formatted_value = if width > 0 {
                format!("{:0>width$}", value, width = width)
            } else {
                value.to_string()
            };

            result.replace_range(abs_pos..var_end, &formatted_value);
            start = abs_pos + formatted_value.len();
        } else {
            break;
        }
    }
    result
}

fn sanitize_path_segment(name: &str) -> String {
    let mut sanitized = String::with_capacity(name.len());
    let mut last_underscore = false;

    for c in name.chars() {
        let is_invalid =
            c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|');

        if is_invalid {
            if !last_underscore {
                sanitized.push('_');
                last_underscore = true;
            }
        } else {
            sanitized.push(c);
            last_underscore = c == '_';
        }
    }

    let trimmed = sanitized
        .trim_matches(|c: char| c.is_whitespace() || c == '.')
        .to_string();

    if trimmed.is_empty() || trimmed.chars().all(|c| c == '_') {
        "unknown".to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_track() -> TrackInfo {
        TrackInfo {
            id: "123".to_string(),
            title: "Test Song".to_string(),
            album: "Test Album".to_string(),
            artist: "Test Artist".to_string(),
            album_artist: "Test Album Artist".to_string(),
            track_number: 1,
            disc_number: 1,
            ..Default::default()
        }
    }

    #[test]
    fn test_replace_padded_variable_no_padding() {
        assert_eq!(
            replace_padded_variable("{track:track_number}", "track:track_number", "5"),
            "5"
        );
    }

    #[test]
    fn test_replace_padded_variable_with_padding() {
        assert_eq!(
            replace_padded_variable("{track:track_number:2}", "track:track_number", "5"),
            "05"
        );
        assert_eq!(
            replace_padded_variable("{track:track_number:3}", "track:track_number", "12"),
            "012"
        );
    }

    #[test]
    fn test_replace_padded_variable_invalid_padding_fallback() {
        assert_eq!(
            replace_padded_variable("{track:track_number:abc}", "track:track_number", "5"),
            "5"
        );
    }

    #[test]
    fn test_replace_padded_variable_multiple_occurrences() {
        let template = "{track:track_number:2}_disc{track:disc_number:2}";
        assert_eq!(
            replace_padded_variable(template, "track:track_number", "7"),
            "07_disc{track:disc_number:2}"
        );
    }

    #[test]
    fn test_sanitize_path_segment_standard() {
        assert_eq!(sanitize_path_segment("Hello World"), "Hello World");
    }

    #[test]
    fn test_sanitize_path_segment_reserved_chars() {
        assert_eq!(sanitize_path_segment("AC/DC"), "AC_DC");
        assert_eq!(sanitize_path_segment("A:B|C?D*E"), "A_B_C_D_E");
        assert_eq!(sanitize_path_segment("<Track>"), "_Track_");
    }

    #[test]
    fn test_sanitize_path_segment_collapse_underscores() {
        assert_eq!(sanitize_path_segment("Song ??? Title"), "Song _ Title");
        assert_eq!(sanitize_path_segment("Song//Title"), "Song_Title");
    }

    #[test]
    fn test_sanitize_path_segment_trimming() {
        assert_eq!(sanitize_path_segment("  *test*  "), "_test_");
        assert_eq!(sanitize_path_segment("...file..."), "file");
        assert_eq!(sanitize_path_segment("_song_"), "_song_");
        assert_eq!(sanitize_path_segment(" . _ mixed _ . "), "_ mixed _");
    }

    #[test]
    fn test_sanitize_path_segment_empty_and_fallback() {
        assert_eq!(sanitize_path_segment(""), "unknown");
        assert_eq!(sanitize_path_segment("???"), "unknown");
        assert_eq!(sanitize_path_segment("..."), "unknown");
        assert_eq!(sanitize_path_segment("   "), "unknown");
        assert_eq!(sanitize_path_segment("___"), "unknown");
        assert_eq!(sanitize_path_segment(". _ ."), "unknown");
    }

    #[test]
    fn test_sanitize_path_segment_preserves_leading_trailing_underscores() {
        assert_eq!(sanitize_path_segment("_intro_"), "_intro_");
        assert_eq!(sanitize_path_segment("__init__"), "__init__");
        assert_eq!(
            sanitize_path_segment(" _spaces_and_underscores_ "),
            "_spaces_and_underscores_"
        );
    }

    #[test]
    fn test_process_template_auto_appends_extension() {
        let track = default_track();

        let template = "lyrics/{type}/{track:artist}/{track:title}";
        let path = process_template(template, &track, LyricsKind::Synced, "lrc");
        assert_eq!(
            path,
            PathBuf::from("lyrics/synced/Test Artist/Test Song.lrc")
        );
    }

    #[test]
    fn test_process_template_preserves_secondary_extensions() {
        let track = default_track();

        let template_backup = "lyrics/{track:title}.backup";
        let path = process_template(template_backup, &track, LyricsKind::Synced, "lrc");
        assert_eq!(path, PathBuf::from("lyrics/Test Song.backup.lrc"));

        let template_multi = "lyrics/{track:title}.backup.txt";
        let path = process_template(template_multi, &track, LyricsKind::Synced, "lrc");
        assert_eq!(path, PathBuf::from("lyrics/Test Song.backup.txt.lrc"));
    }

    #[test]
    fn test_process_template_appends_to_hardcoded_txt() {
        let track = default_track();

        let template = "lyrics/{track:title}.txt";
        let path = process_template(template, &track, LyricsKind::Synced, "lrc");
        assert_eq!(path, PathBuf::from("lyrics/Test Song.txt.lrc"));
    }

    #[test]
    fn test_process_template_handles_exact_hardcoded_ext() {
        let track = default_track();

        let template = "lyrics/{track:title}.lrc";
        let path = process_template(template, &track, LyricsKind::Synced, "lrc");
        assert_eq!(path, PathBuf::from("lyrics/Test Song.lrc"));
    }

    #[test]
    fn test_process_template_handles_multiple_slashes() {
        let track = default_track();

        let template = "lyrics///{type}//{track:artist}/";
        let path = process_template(template, &track, LyricsKind::Synced, "lrc");
        assert_eq!(path, PathBuf::from("lyrics/synced/Test Artist/unknown.lrc"));
    }

    #[test]
    fn test_process_template_handles_only_slashes() {
        let track = default_track();

        let template = "///";
        let path = process_template(template, &track, LyricsKind::Synced, "lrc");
        assert_eq!(path, PathBuf::from("unknown.lrc"));
    }

    #[test]
    fn test_process_template_handles_empty_variables() {
        let track = TrackInfo {
            title: String::new(),
            artist: String::new(),
            ..default_track()
        };

        let template = "{track:artist}/{track:title}";
        let path = process_template(template, &track, LyricsKind::Plain, "txt");
        assert_eq!(path, PathBuf::from("unknown/unknown.txt"));
    }

    #[test]
    fn test_process_template_handles_variables_with_only_invalid_chars() {
        let track = TrackInfo {
            title: "???".to_string(),
            artist: "***".to_string(),
            ..default_track()
        };

        let template = "{track:artist}/{track:title}";
        let path = process_template(template, &track, LyricsKind::Plain, "txt");
        assert_eq!(path, PathBuf::from("unknown/unknown.txt"));
    }

    #[test]
    fn test_process_template_handles_dots_in_directory_names() {
        let track = TrackInfo {
            artist: "U2".to_string(),
            album: "Achtung.Baby".to_string(),
            ..default_track()
        };

        let template = "{track:artist}/{track:album}/{track:title}";
        let path = process_template(template, &track, LyricsKind::Synced, "lrc");
        assert_eq!(path, PathBuf::from("U2/Achtung.Baby/Test Song.lrc"));
    }

    #[test]
    fn test_process_template_handles_leading_trailing_dots_in_metadata() {
        let track = TrackInfo {
            title: "  ..Hidden Song..  ".to_string(),
            artist: ".The Artist.".to_string(),
            ..default_track()
        };

        let template = "{track:artist}/{track:title}";
        let path = process_template(template, &track, LyricsKind::Plain, "txt");
        assert_eq!(path, PathBuf::from("The Artist/Hidden Song.txt"));
    }

    #[test]
    fn test_process_template_prevents_directory_traversal_via_metadata() {
        let track = TrackInfo {
            title: "../../../etc/passwd".to_string(),
            ..default_track()
        };

        let template = "lyrics/{track:title}";
        let path = process_template(template, &track, LyricsKind::Plain, "txt");

        assert_eq!(path, PathBuf::from("lyrics/_.._.._etc_passwd.txt"));
    }

    #[test]
    fn test_process_template_prevents_directory_traversal_via_template() {
        let track = default_track();

        let template = "../../{track:title}";
        let path = process_template(template, &track, LyricsKind::Plain, "txt");

        assert_eq!(path, PathBuf::from("unknown/unknown/Test Song.txt"));
    }

    #[test]
    fn test_process_template_handles_unicode_and_emojis() {
        let track = TrackInfo {
            title: "大丈夫 🎵".to_string(),
            artist: "Björk".to_string(),
            ..default_track()
        };

        let template = "lyrics/{track:artist}/{track:title}";
        let path = process_template(template, &track, LyricsKind::Synced, "lrc");

        assert_eq!(path, PathBuf::from("lyrics/Björk/大丈夫 🎵.lrc"));
    }

    #[test]
    fn test_process_template_handles_typo_in_variable_name() {
        let track = default_track();

        let template = "lyrics/{track:titel}";
        let path = process_template(template, &track, LyricsKind::Plain, "txt");

        assert_eq!(path, PathBuf::from("lyrics/{track_titel}.txt"));
    }

    #[test]
    fn test_process_template_handles_spaces_around_variables() {
        let track = default_track();

        let template = "lyrics/{ track:title }";
        let path = process_template(template, &track, LyricsKind::Plain, "txt");

        assert_eq!(path, PathBuf::from("lyrics/{ track_title }.txt"));
    }

    #[test]
    fn test_process_template_handles_mixed_slashes() {
        let track = default_track();

        let template = r"lyrics\{type}/{track:artist}\{track:title}";
        let path = process_template(template, &track, LyricsKind::Synced, "lrc");

        assert_eq!(
            path,
            PathBuf::from("lyrics/synced/Test Artist/Test Song.lrc")
        );
    }

    #[test]
    fn test_process_template_with_padded_numbers() {
        let track = TrackInfo {
            track_number: 5,
            disc_number: 1,
            ..default_track()
        };

        let template_unpadded =
            "lyrics/Disc {track:disc_number}/{track:track_number} - {track:title}";
        let path_unpadded = process_template(template_unpadded, &track, LyricsKind::Plain, "txt");
        assert_eq!(
            path_unpadded,
            PathBuf::from("lyrics/Disc 1/5 - Test Song.txt")
        );

        let template_padded =
            "lyrics/Disc {track:disc_number:2}/{track:track_number:2} - {track:title}";
        let path_padded = process_template(template_padded, &track, LyricsKind::Plain, "txt");
        assert_eq!(
            path_padded,
            PathBuf::from("lyrics/Disc 01/05 - Test Song.txt")
        );
    }
}
