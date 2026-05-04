use nd_pdk::{
    host::library,
    lyrics::{Error as LyricsError, TrackInfo},
};
use std::{fs, path::PathBuf};

pub fn write(
    track: &TrackInfo,
    text: &str,
    extension: &str,
    overwrite: bool,
) -> Result<(), LyricsError> {
    if track.path.is_empty() {
        return Err(LyricsError::new("track path is empty"));
    }

    if let Some(mut path) = resolve_track_path(track)? {
        path.set_extension(extension);

        if path.exists() && !overwrite {
            return Ok(());
        }

        fs::write(&path, text.as_bytes())
            .map_err(|e| LyricsError::new(format!("failed to write lyrics file: {e}")))?;

        Ok(())
    } else {
        Err(LyricsError::new("could not resolve track path!"))
    }
}

fn resolve_track_path(track: &TrackInfo) -> Result<Option<PathBuf>, LyricsError> {
    let libraries = library::get_all_libraries()
        .map_err(|e| LyricsError::new(format!("failed to list libraries: {e}")))?;

    for library in libraries {
        let mount_point = library.mount_point;
        let full_path = PathBuf::from(mount_point).join(&track.path);
        if fs::metadata(&full_path).is_ok() {
            return Ok(Some(full_path));
        }
    }

    Ok(None)
}
