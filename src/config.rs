use crate::types::LyricsKind;
use extism_pdk::warn;
use nd_pdk::{host::config, lyrics::Error as LyricsError};
use serde::Deserialize;
use std::{collections::HashSet, fmt};

const DEFAULT_CACHE_TTL: i64 = 168;
const DEFAULT_NEGATIVE_CACHE_TTL: i64 = 24;
const MIN_CACHE_TTL: i64 = 1;

const DEFAULT_PLAIN_EXTENSION: &str = "txt";
const DEFAULT_INSTRUMENTAL_EXTENSION: &str = "txt";

const DEFAULT_INSTRUMENTAL_TEXT: &str = "Instrumental";
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

impl fmt::Display for ProviderEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

pub struct PluginConfig {
    pub lyrics_type_priority: Vec<LyricsKind>,
    pub write_lyrics: bool,
    pub overwrite_lyrics: bool,
    pub plain_extension: String,
    pub instrumental_extension: String,
    pub enable_cache: bool,
    pub cache_ttl_hours: i64,
    pub negative_cache: bool,
    pub negative_cache_ttl_hours: i64,
    pub providers: Vec<ProviderEntry>,
    pub write_to_specific_folder: bool,
    pub write_to_specific_folder_library_id: Option<i32>,
    pub write_to_specific_folder_template: String,
    pub strip_section_labels: bool,
    pub instrumental_text: String,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            lyrics_type_priority: vec![],
            write_lyrics: false,
            overwrite_lyrics: false,
            plain_extension: DEFAULT_PLAIN_EXTENSION.to_string(),
            instrumental_extension: DEFAULT_INSTRUMENTAL_EXTENSION.to_string(),
            enable_cache: true,
            cache_ttl_hours: DEFAULT_CACHE_TTL,
            negative_cache: true,
            negative_cache_ttl_hours: DEFAULT_NEGATIVE_CACHE_TTL,
            providers: vec![],
            write_to_specific_folder: false,
            write_to_specific_folder_library_id: None,
            write_to_specific_folder_template: DEFAULT_FOLDER_TEMPLATE.to_string(),
            strip_section_labels: false,
            instrumental_text: DEFAULT_INSTRUMENTAL_TEXT.to_string(),
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
            instrumental_extension: resolve_extension(
                "instrumentalExtension",
                DEFAULT_INSTRUMENTAL_EXTENSION,
            )?,
            enable_cache: get_bool("enableCache", true)?,
            cache_ttl_hours: resolve_cache_ttl()?,
            negative_cache: get_bool("negativeCache", true)?,
            negative_cache_ttl_hours: resolve_negative_cache_ttl()?,
            providers: resolve_providers()?,
            write_to_specific_folder: get_bool("writeToSpecificFolder", false)?,
            write_to_specific_folder_library_id: get_optional_i32(
                "writeToSpecificFolderLibraryId",
            )?,
            write_to_specific_folder_template: get_string("writeToSpecificFolderTemplate")?
                .unwrap_or_else(|| DEFAULT_FOLDER_TEMPLATE.to_string()),
            strip_section_labels: get_bool("stripSectionLabels", false)?,
            instrumental_text: get_string("instrumentalText")?
                .unwrap_or_else(|| DEFAULT_INSTRUMENTAL_TEXT.to_string()),
        })
    }

    pub fn resolve_order(&self) -> &[LyricsKind] {
        &self.lyrics_type_priority
    }

    pub fn wants(&self, kind: LyricsKind) -> bool {
        self.lyrics_type_priority.contains(&kind)
    }

    pub fn wants_lrc(&self) -> bool {
        self.wants(LyricsKind::Lrc)
    }

    pub fn wants_plain(&self) -> bool {
        self.wants(LyricsKind::Plain)
    }

    pub fn extension_for(&self, kind: LyricsKind) -> &str {
        match kind {
            LyricsKind::Plain => self.plain_extension.as_str(),
            LyricsKind::Instrumental => self.instrumental_extension.as_str(),
            LyricsKind::Lrc => "lrc",
            LyricsKind::Elrc => "elrc",
            LyricsKind::Ttml => "ttml",
            LyricsKind::Srt => "srt",
            LyricsKind::Lyricsfile => "yml",
        }
    }
}

fn resolve_lyrics_type_priority() -> Result<Vec<LyricsKind>, LyricsError> {
    let order = match get_string("lyricsFormats")? {
        Some(raw) => parse_lyrics_formats(&raw),
        None => Vec::new(),
    };

    if order.is_empty() {
        warn!("no lyrics formats enabled, defaulting to lrc + plain");
        return Ok(vec![LyricsKind::Lrc, LyricsKind::Plain]);
    }

    Ok(order)
}

fn parse_lyrics_formats(raw: &str) -> Vec<LyricsKind> {
    let mut order: Vec<LyricsKind> = Vec::new();
    for slug in raw.split(',') {
        if let Some(kind) = LyricsKind::from_slug(slug)
            && kind != LyricsKind::Instrumental
            && !order.contains(&kind)
        {
            order.push(kind);
        }
    }

    order
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

fn resolve_negative_cache_ttl() -> Result<i64, LyricsError> {
    let raw = get_i64("negativeCacheTtlHours", DEFAULT_NEGATIVE_CACHE_TTL)?;

    if raw < MIN_CACHE_TTL {
        warn!(
            "negativeCacheTtlHours {raw} is below minimum of {MIN_CACHE_TTL}, using {MIN_CACHE_TTL} instead"
        );
        return Ok(MIN_CACHE_TTL);
    }

    Ok(raw)
}

fn resolve_providers() -> Result<Vec<ProviderEntry>, LyricsError> {
    let providers = get_string("providersList")?
        .map(|s| parse_providers(&s))
        .unwrap_or_default();

    if providers.is_empty() {
        warn!("no providers configured, no lyrics will be fetched");
    }

    Ok(providers)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderRow {
    #[serde(default)]
    provider: String,
    #[serde(default)]
    base_url: String,
}

fn parse_providers(raw: &str) -> Vec<ProviderEntry> {
    let rows: Vec<ProviderRow> = serde_json::from_str(raw).unwrap_or_default();

    let mut seen = HashSet::new();
    rows.into_iter()
        .filter_map(|row| {
            let name = row.provider.trim();
            if name.is_empty() {
                return None;
            }
            let param = row.base_url.trim();
            Some(ProviderEntry {
                name: name.to_string(),
                param: (!param.is_empty()).then(|| param.to_string()),
            })
        })
        .filter(|entry| seen.insert(entry.clone()))
        .collect()
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
    fn test_parse_lyrics_formats_ordered() {
        assert_eq!(
            parse_lyrics_formats("ttml,lyricsfile,elrc,lrc,srt,plain"),
            vec![
                LyricsKind::Ttml,
                LyricsKind::Lyricsfile,
                LyricsKind::Elrc,
                LyricsKind::Lrc,
                LyricsKind::Srt,
                LyricsKind::Plain,
            ]
        );
    }

    #[test]
    fn test_parse_lyrics_formats_whitespace_and_case() {
        assert_eq!(
            parse_lyrics_formats(" LRC , Plain "),
            vec![LyricsKind::Lrc, LyricsKind::Plain]
        );
    }

    #[test]
    fn test_parse_lyrics_formats_dedup_and_skips_unknown() {
        assert_eq!(parse_lyrics_formats("lrc,bogus,lrc"), vec![LyricsKind::Lrc]);
    }

    #[test]
    fn test_parse_lyrics_formats_excludes_instrumental() {
        assert_eq!(
            parse_lyrics_formats("instrumental,plain"),
            vec![LyricsKind::Plain]
        );
    }

    #[test]
    fn test_parse_lyrics_formats_empty() {
        assert_eq!(parse_lyrics_formats(""), Vec::<LyricsKind>::new());
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
    fn test_display_name_with_param() {
        let e = entry("lrclib", Some("http://localhost:7592"));
        assert_eq!(e.display_name(), "lrclib(http://localhost:7592)");
    }

    #[test]
    fn test_display_name_without_param() {
        let e = entry("lrclib", None);
        assert_eq!(e.display_name(), "lrclib");
    }

    #[test]
    fn test_parse_providers_json_basic() {
        assert_eq!(
            parse_providers(r#"[{"provider":"lrclib"},{"provider":"lyrics.ovh"}]"#),
            vec![entry("lrclib", None), entry("lyrics.ovh", None)]
        );
    }

    #[test]
    fn test_parse_providers_json_with_base_url() {
        assert_eq!(
            parse_providers(r#"[{"provider":"lrclib","baseUrl":"http://localhost:7592"}]"#),
            vec![entry("lrclib", Some("http://localhost:7592"))]
        );
    }

    #[test]
    fn test_parse_providers_json_blank_base_url_is_none() {
        assert_eq!(
            parse_providers(r#"[{"provider":"lrclib","baseUrl":"   "}]"#),
            vec![entry("lrclib", None)]
        );
    }

    #[test]
    fn test_parse_providers_json_skips_empty_provider_and_dedups() {
        assert_eq!(
            parse_providers(
                r#"[{"provider":""},{"provider":"kugou"},{"provider":"kugou"},{"provider":"netease"}]"#
            ),
            vec![entry("kugou", None), entry("netease", None)]
        );
    }

    #[test]
    fn test_parse_providers_json_invalid_is_empty() {
        assert_eq!(parse_providers("not json"), Vec::<ProviderEntry>::new());
    }

    #[test]
    fn test_parse_providers_json_empty_array() {
        assert_eq!(parse_providers("[]"), Vec::<ProviderEntry>::new());
    }
}
