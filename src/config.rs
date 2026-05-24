use crate::types::LyricsType;
use extism_pdk::warn;
use nd_pdk::{host::config, lyrics::Error as LyricsError};
use std::collections::HashSet;

const DEFAULT_CACHE_TTL: i64 = 168;
const MIN_CACHE_TTL: i64 = 1;

const DEFAULT_PLAIN_EXTENSION: &str = "txt";
const DEFAULT_SYNCED_EXTENSION: &str = "lrc";
const DEFAULT_FOLDER_TEMPLATE: &str = "_lyrics/{type}/{track:album_artist} - {track:album}/{track:disc_number:2} - {track:track_number:2} {track:title}";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderEntry {
    pub name: String,
    pub param: Option<String>,
}

impl ProviderEntry {
    pub fn display_name(&self) -> String {
        match &self.param {
            Some(p) => format!("{}({})", self.name, p),
            None => self.name.clone(),
        }
    }
}

impl std::fmt::Display for ProviderEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

pub struct PluginConfig {
    pub lyrics_type_priority: Vec<LyricsType>,
    pub write_lyrics: bool,
    pub overwrite_lyrics: bool,
    pub plain_extension: String,
    pub synced_extension: String,
    pub enable_cache: bool,
    pub cache_ttl_hours: i64,
    pub providers: Vec<ProviderEntry>,
    pub write_to_specific_folder: bool,
    pub write_to_specific_folder_library_id: Option<i32>,
    pub write_to_specific_folder_template: String,
    pub strip_section_labels: bool,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            lyrics_type_priority: vec![],
            write_lyrics: false,
            overwrite_lyrics: false,
            plain_extension: DEFAULT_PLAIN_EXTENSION.to_string(),
            synced_extension: DEFAULT_SYNCED_EXTENSION.to_string(),
            enable_cache: true,
            cache_ttl_hours: DEFAULT_CACHE_TTL,
            providers: vec![],
            write_to_specific_folder: false,
            write_to_specific_folder_library_id: None,
            write_to_specific_folder_template: DEFAULT_FOLDER_TEMPLATE.to_string(),
            strip_section_labels: false,
        }
    }
}

impl PluginConfig {
    pub fn load() -> Result<Self, LyricsError> {
        Ok(Self {
            lyrics_type_priority: resolve_lyrics_type_priority()?,
            write_lyrics: get_bool("writeLyrics", false)?,
            overwrite_lyrics: get_bool("overwriteLyrics", false)?,
            plain_extension: resolve_extension("plainExtension", DEFAULT_PLAIN_EXTENSION)?,
            synced_extension: resolve_extension("syncedExtension", DEFAULT_SYNCED_EXTENSION)?,
            enable_cache: get_bool("enableCache", true)?,
            cache_ttl_hours: resolve_cache_ttl()?,
            providers: resolve_providers()?,
            write_to_specific_folder: get_bool("writeToSpecificFolder", false)?,
            write_to_specific_folder_library_id: get_optional_i32(
                "writeToSpecificFolderLibraryId",
            )?,
            write_to_specific_folder_template: get_string("writeToSpecificFolderTemplate")?
                .unwrap_or_else(|| DEFAULT_FOLDER_TEMPLATE.to_string()),
            strip_section_labels: get_bool("stripSectionLabels", false)?,
        })
    }

    pub fn resolve_order(&self) -> &[LyricsType] {
        &self.lyrics_type_priority
    }

    #[allow(dead_code)]
    pub fn wants_synced(&self) -> bool {
        self.lyrics_type_priority.contains(&LyricsType::Synced)
    }

    pub fn wants_plain(&self) -> bool {
        self.lyrics_type_priority.contains(&LyricsType::Plain)
    }
}

fn resolve_lyrics_type_priority() -> Result<Vec<LyricsType>, LyricsError> {
    let want_synced = get_bool("lyricsSynced", true)?;
    let want_plain = get_bool("lyricsPlain", true)?;
    let prefers_synced_first = get_string("lyricsPriority")?.as_deref() != Some("plain");

    let mut priority = Vec::new();

    if want_synced {
        priority.push(LyricsType::Synced);
    }
    if want_plain {
        priority.push(LyricsType::Plain);
    }

    if priority.is_empty() {
        warn!("no lyrics types selected, defaulting to synced + plain");
        priority = vec![LyricsType::Synced, LyricsType::Plain];
    }

    if priority.len() == 2 && !prefers_synced_first {
        priority.reverse();
    }

    Ok(priority)
}

fn resolve_extension(key: &str, default_value: &str) -> Result<String, LyricsError> {
    let extension = get_string(key)?
        .map(|s| normalize_extension(&s))
        .unwrap_or_else(|| default_value.to_string());

    if extension.is_empty() {
        warn!("{key} resolved to empty string, using '{default_value}'");
        return Ok(default_value.to_string());
    }

    Ok(extension)
}

fn resolve_cache_ttl() -> Result<i64, LyricsError> {
    let raw = get_i64("cacheTtlHours", DEFAULT_CACHE_TTL)?;

    if raw < MIN_CACHE_TTL {
        warn!(
            "cacheTtlHours {raw} is below minimum of {MIN_CACHE_TTL}, using {MIN_CACHE_TTL} instead"
        );
        return Ok(MIN_CACHE_TTL);
    }

    Ok(raw)
}

fn resolve_providers() -> Result<Vec<ProviderEntry>, LyricsError> {
    let providers = get_string("providers")?
        .map(|s| parse_providers(&s))
        .unwrap_or_default();

    if providers.is_empty() {
        warn!("no providers configured, no lyrics will be fetched");
    }

    Ok(providers)
}

fn parse_providers(raw: &str) -> Vec<ProviderEntry> {
    let mut seen = HashSet::new();
    raw.split(',')
        .filter_map(parse_provider_entry)
        .filter(|entry| seen.insert(entry.clone()))
        .collect()
}

fn parse_provider_entry(raw: &str) -> Option<ProviderEntry> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(paren_pos) = trimmed.find('(')
        && trimmed.ends_with(')')
    {
        let name = trimmed[..paren_pos].trim();
        if name.is_empty() {
            return None;
        }
        let param = trimmed[paren_pos + 1..trimmed.len() - 1].trim();
        let param = if param.is_empty() {
            None
        } else {
            Some(param.to_string())
        };
        return Some(ProviderEntry {
            name: name.to_string(),
            param,
        });
    }

    Some(ProviderEntry {
        name: trimmed.to_string(),
        param: None,
    })
}

fn normalize_extension(ext: &str) -> String {
    ext.trim().trim_start_matches('.').to_string()
}

fn get_string(key: &str) -> Result<Option<String>, LyricsError> {
    config::get(key).map_err(|e| LyricsError::new(e.to_string()))
}

fn get_bool(key: &str, default: bool) -> Result<bool, LyricsError> {
    config::get(key)
        .map_err(|e| LyricsError::new(e.to_string()))
        .map(|v| v.map(|s| s == "true").unwrap_or(default))
}

fn get_i64(key: &str, default: i64) -> Result<i64, LyricsError> {
    config::get(key)
        .map_err(|e| LyricsError::new(e.to_string()))
        .map(|v| v.and_then(|s| s.parse().ok()).unwrap_or(default))
}

fn get_optional_i32(key: &str) -> Result<Option<i32>, LyricsError> {
    config::get(key)
        .map_err(|e| LyricsError::new(e.to_string()))
        .map(|v| v.and_then(|s| s.parse().ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, param: Option<&str>) -> ProviderEntry {
        ProviderEntry {
            name: name.to_string(),
            param: param.map(|s| s.to_string()),
        }
    }

    #[test]
    fn test_normalize_extension() {
        assert_eq!(normalize_extension("lrc"), "lrc");
        assert_eq!(normalize_extension(".lrc"), "lrc");
        assert_eq!(normalize_extension("...lrc"), "lrc");
        assert_eq!(normalize_extension("  .txt  "), "txt");
        assert_eq!(normalize_extension("."), "");
    }

    #[test]
    fn test_parse_providers_basic() {
        assert_eq!(
            parse_providers("lrclib,lyrics.ovh"),
            vec![entry("lrclib", None), entry("lyrics.ovh", None)]
        );
    }

    #[test]
    fn test_parse_providers_single() {
        assert_eq!(parse_providers("lrclib"), vec![entry("lrclib", None)]);
    }

    #[test]
    fn test_parse_providers_whitespace() {
        assert_eq!(
            parse_providers(" lrclib , lyrics.ovh "),
            vec![entry("lrclib", None), entry("lyrics.ovh", None)]
        );
    }

    #[test]
    fn test_parse_providers_empty_entries() {
        assert_eq!(
            parse_providers("lrclib,,lyrics.ovh"),
            vec![entry("lrclib", None), entry("lyrics.ovh", None)]
        );
    }

    #[test]
    fn test_parse_providers_empty_string() {
        assert_eq!(parse_providers(""), Vec::<ProviderEntry>::new());
    }

    #[test]
    fn test_parse_providers_just_comma() {
        assert_eq!(parse_providers(","), Vec::<ProviderEntry>::new());
    }

    #[test]
    fn test_parse_providers_parameterized() {
        assert_eq!(
            parse_providers("lrclib(http://localhost:7592)"),
            vec![entry("lrclib", Some("http://localhost:7592"))]
        );
    }

    #[test]
    fn test_parse_providers_mixed_parameterized_and_plain() {
        assert_eq!(
            parse_providers("lrclib(http://localhost:7592),lrclib,lyrics.ovh"),
            vec![
                entry("lrclib", Some("http://localhost:7592")),
                entry("lrclib", None),
                entry("lyrics.ovh", None),
            ]
        );
    }

    #[test]
    fn test_parse_providers_dedup_exact_duplicates() {
        assert_eq!(
            parse_providers("lrclib,lyrics.ovh,lrclib"),
            vec![entry("lrclib", None), entry("lyrics.ovh", None)]
        );
    }

    #[test]
    fn test_parse_providers_parameterized_not_deduped_with_plain() {
        assert_eq!(
            parse_providers("lrclib(http://localhost:7592),lrclib"),
            vec![
                entry("lrclib", Some("http://localhost:7592")),
                entry("lrclib", None),
            ]
        );
    }

    #[test]
    fn test_parse_providers_dedup_parameterized_duplicates() {
        assert_eq!(
            parse_providers("lrclib(http://x),lyrics.ovh,lrclib(http://x)"),
            vec![entry("lrclib", Some("http://x")), entry("lyrics.ovh", None),]
        );
    }

    #[test]
    fn test_parse_providers_empty_param_treated_as_none() {
        assert_eq!(parse_providers("lrclib()"), vec![entry("lrclib", None)]);
    }

    #[test]
    fn test_display_name_with_param() {
        let e = entry("lrclib", Some("http://localhost:7592"));
        assert_eq!(e.display_name(), "lrclib(http://localhost:7592)");
    }

    #[test]
    fn test_display_name_without_param() {
        let e = entry("lrclib", None);
        assert_eq!(e.display_name(), "lrclib");
    }
}
