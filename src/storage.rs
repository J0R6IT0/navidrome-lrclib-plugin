use nd_pdk::{
    host::{kvstore, library},
    lyrics::Error as LyricsError,
};
use std::{
    fs::{self, create_dir_all},
    path::PathBuf,
};

#[derive(Clone, Copy)]
pub enum LyricsKind {
    Synchronized,
    Plain,
}

impl LyricsKind {
    fn subdir(self) -> &'static str {
        match self {
            LyricsKind::Synchronized => "synchronized",
            LyricsKind::Plain => "plain",
        }
    }
}

const KV_LIBRARY_KEY: &str = "lyrics_library_id";

pub struct LyricsStorage {
    lyrics_dir: PathBuf,
}

impl LyricsStorage {
    pub fn new() -> Result<Self, LyricsError> {
        let mount_point = resolve_library_mount()?;
        let lyrics_dir = PathBuf::from(mount_point).join("_lyrics");
        Ok(Self { lyrics_dir })
    }

    pub fn read(&self, track_id: &str, prefer_synced: bool) -> Result<Option<String>, LyricsError> {
        let candidates: &[LyricsKind] = if prefer_synced {
            &[LyricsKind::Synchronized, LyricsKind::Plain]
        } else {
            &[LyricsKind::Plain]
        };

        for kind in candidates {
            let path = self.lrc_path(track_id, *kind);
            if path.exists() {
                let text = fs::read_to_string(&path)
                    .map_err(|e| LyricsError::new(format!("failed to read lyrics cache: {e}")))?;
                return Ok(Some(text));
            }
        }

        Ok(None)
    }

    pub fn write(&self, track_id: &str, text: &str, kind: LyricsKind) -> Result<(), LyricsError> {
        let path = self.lrc_path(track_id, kind);

        create_dir_all(path.parent().unwrap())
            .map_err(|e| LyricsError::new(format!("failed to create lyrics directory: {e}")))?;

        fs::write(&path, text.as_bytes())
            .map_err(|e| LyricsError::new(format!("failed to write lyrics file: {e}")))?;

        Ok(())
    }

    fn lrc_path(&self, track_id: &str, kind: LyricsKind) -> PathBuf {
        self.lyrics_dir
            .join(kind.subdir())
            .join(format!("{}.lrc", track_id))
    }
}

fn resolve_library_mount() -> Result<String, LyricsError> {
    if let Ok(Some(bytes)) = kvstore::get(KV_LIBRARY_KEY) {
        if bytes.len() == 4 {
            let mut arr = [0u8; 4];
            arr.copy_from_slice(&bytes);

            let stored_id = i32::from_le_bytes(arr);

            if let Ok(Some(lib)) = library::get_library(stored_id)
                && !lib.mount_point.is_empty()
            {
                return Ok(lib.mount_point);
            }
        }
    }

    let libraries = library::get_all_libraries()
        .map_err(|e| LyricsError::new(format!("failed to list libraries: {e}")))?;

    if libraries.is_empty() {
        return Err(LyricsError::new("no libraries available"));
    }

    let chosen = &libraries[0];

    kvstore::set(KV_LIBRARY_KEY, chosen.id.to_le_bytes().to_vec())
        .map_err(|e| LyricsError::new(format!("failed to persist library ID: {e}")))?;

    Ok(chosen.mount_point.clone())
}
