use extism_pdk::warn;
use nd_pdk::{
    host::library,
    lyrics::{Error as LyricsError, TrackInfo},
};
use std::{
    fs::{self},
    path::PathBuf,
};

pub fn write(track: &TrackInfo, text: &str, update: bool) -> Result<(), LyricsError> {
    if let Some(mut path) = resolve_track_path(track)? {
        path.set_extension("lrc");

        if path.exists() && !update {
            return Ok(());
        }

        fs::write(&path, text.as_bytes())
            .map_err(|e| LyricsError::new(format!("failed to write lyrics file: {e}")))?;
    } else {
        warn!("could not resolve track path!")
    }

    Ok(())
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
