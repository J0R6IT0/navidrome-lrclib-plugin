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

    let mut path = resolve_track_path(track)?
        .ok_or_else(|| LyricsError::new("could not resolve track path to a valid local file"))?;

    path.set_extension(extension);

    if path.exists() && !overwrite {
        return Ok(());
    }

    fs::write(&path, text)
        .map_err(|e| LyricsError::new(format!("failed to write lyrics file: {e}")))?;

    Ok(())
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
