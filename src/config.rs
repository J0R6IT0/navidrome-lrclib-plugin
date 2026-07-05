use crate::types::LyricsKind;
use extism_pdk::warn;
use nd_pdk::{host::config, lyrics::Error as LyricsError};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet},
    fmt,
};

const DEFAULT_CACHE_TTL: i64 = 168;
const DEFAULT_NEGATIVE_CACHE_TTL: i64 = 24;
const MIN_CACHE_TTL: i64 = 1;
const MAX_CACHE_TTL: i64 = 1_000_000;

const DEFAULT_PLAIN_EXTENSION: &str = "txt";
const DEFAULT_INSTRUMENTAL_EXTENSION: &str = "txt";

const DEFAULT_DURATION_TOLERANCE_SECS: f32 = 3.0;
const MIN_DURATION_TOLERANCE_SECS: f32 = 1.0;
const MAX_DURATION_TOLERANCE_SECS: f32 = 3600.0;

const DEFAULT_INSTRUMENTAL_TEXT: &str = "Instrumental";
const DEFAULT_FOLDER_TEMPLATE: &str = "_lyrics/{type}/{track:album_artist} - {track:album}/{track:disc_number:2} - {track:track_number:2} {track:title}";

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ProviderParams(BTreeMap<String, String>);

impl ProviderParams {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProviderMode {
    #[default]
    Priority,
    Rotation,
}

impl ProviderMode {
    pub fn from_slug(slug: &str) -> Option<ProviderMode> {
        match slug.trim().to_ascii_lowercase().as_str() {
            "priority" => Some(ProviderMode::Priority),
            "rotation" => Some(ProviderMode::Rotation),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderEntry {
    pub name: String,
    pub params: ProviderParams,
}

impl ProviderEntry {
    pub fn cache_id(&self) -> String {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |bytes: &[u8]| {
            for byte in bytes {
                hash ^= *byte as u64;
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };

        mix(self.name.as_bytes());
        for (k, v) in &self.params.0 {
            mix(b"\0");
            mix(k.as_bytes());
            mix(b"=");
            mix(v.as_bytes());
        }

        format!("{hash:016x}")
    }

    pub fn display_name(&self) -> String {
        if self.params.is_empty() {
            return self.name.clone();
        }

        let joined = self
            .params
            .0
            .iter()
            .map(|(k, v)| {
                let v = if is_sensitive_key(k) { "***" } else { v };
                format!("{k}={v}")
            })
            .collect::<Vec<_>>()
            .join(", ");

        format!("{}({joined})", self.name)
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
    pub provider_mode: ProviderMode,
    pub write_to_specific_folder: bool,
    pub write_to_specific_folder_library_id: Option<i32>,
    pub write_to_specific_folder_template: String,
    pub strip_section_labels: bool,
    pub instrumental_text: String,
    pub duration_tolerance_secs: f32,
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
            provider_mode: ProviderMode::default(),
            write_to_specific_folder: false,
            write_to_specific_folder_library_id: None,
            write_to_specific_folder_template: DEFAULT_FOLDER_TEMPLATE.to_string(),
            strip_section_labels: false,
            instrumental_text: DEFAULT_INSTRUMENTAL_TEXT.to_string(),
            duration_tolerance_secs: DEFAULT_DURATION_TOLERANCE_SECS,
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
            provider_mode: resolve_provider_mode()?,
            write_to_specific_folder: get_bool("writeToSpecificFolder", false)?,
            write_to_specific_folder_library_id: get_optional_i32(
                "writeToSpecificFolderLibraryId",
            )?,
            write_to_specific_folder_template: get_string("writeToSpecificFolderTemplate")?
                .unwrap_or_else(|| DEFAULT_FOLDER_TEMPLATE.to_string()),
            strip_section_labels: get_bool("stripSectionLabels", false)?,
            instrumental_text: get_string("instrumentalText")?
                .unwrap_or_else(|| DEFAULT_INSTRUMENTAL_TEXT.to_string()),
            duration_tolerance_secs: resolve_duration_tolerance()?,
        })
    }

    pub fn resolve_order(&self) -> &[LyricsKind] {
        &self.lyrics_type_priority
    }

    pub fn wants(&self, kind: LyricsKind) -> bool {
        self.lyrics_type_priority.contains(&kind)
    }

    pub fn duration_tolerance_ms(&self) -> u64 {
        (self.duration_tolerance_secs * 1000.0).round() as u64
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
    Ok(clamp_cache_ttl(
        "cacheTtlHours",
        get_i64("cacheTtlHours", DEFAULT_CACHE_TTL)?,
    ))
}

fn resolve_negative_cache_ttl() -> Result<i64, LyricsError> {
    Ok(clamp_cache_ttl(
        "negativeCacheTtlHours",
        get_i64("negativeCacheTtlHours", DEFAULT_NEGATIVE_CACHE_TTL)?,
    ))
}

fn clamp_cache_ttl(key: &str, raw: i64) -> i64 {
    match classify_ttl(raw) {
        TtlClamp::InRange => raw,
        TtlClamp::BelowMin => {
            warn!("{key} {raw} is below minimum of {MIN_CACHE_TTL}h, clamping to {MIN_CACHE_TTL}h");
            MIN_CACHE_TTL
        }
        TtlClamp::AboveMax => {
            warn!("{key} {raw} exceeds maximum of {MAX_CACHE_TTL}h, clamping to {MAX_CACHE_TTL}h");
            MAX_CACHE_TTL
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum TtlClamp {
    InRange,
    BelowMin,
    AboveMax,
}

fn classify_ttl(raw: i64) -> TtlClamp {
    if raw < MIN_CACHE_TTL {
        TtlClamp::BelowMin
    } else if raw > MAX_CACHE_TTL {
        TtlClamp::AboveMax
    } else {
        TtlClamp::InRange
    }
}

fn resolve_duration_tolerance() -> Result<f32, LyricsError> {
    let raw = get_i64(
        "durationToleranceSeconds",
        DEFAULT_DURATION_TOLERANCE_SECS as i64,
    )?;

    let clamped = raw.clamp(
        MIN_DURATION_TOLERANCE_SECS as i64,
        MAX_DURATION_TOLERANCE_SECS as i64,
    );
    if clamped != raw {
        warn!("durationToleranceSeconds {raw} is out of range, clamping to {clamped}s");
    }

    Ok(clamped as f32)
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

fn resolve_provider_mode() -> Result<ProviderMode, LyricsError> {
    let mode = get_string("providerMode")?.and_then(|s| ProviderMode::from_slug(&s));

    match mode {
        Some(mode) => Ok(mode),
        None => Ok(ProviderMode::default()),
    }
}

fn parse_providers(raw: &str) -> Vec<ProviderEntry> {
    let rows: Vec<BTreeMap<String, Value>> = serde_json::from_str(raw).unwrap_or_default();

    let mut seen = HashSet::new();
    rows.into_iter()
        .filter_map(parse_provider_row)
        .filter(|entry| seen.insert(entry.clone()))
        .collect()
}

fn parse_provider_row(mut row: BTreeMap<String, Value>) -> Option<ProviderEntry> {
    let name = row
        .remove("provider")
        .as_ref()
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())?
        .to_string();

    let params = row
        .into_iter()
        .filter_map(|(key, value)| {
            let value = match value {
                Value::String(s) => s.trim().to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Number(n) => n.to_string(),
                _ => return None,
            };
            (!value.is_empty()).then_some((key, value))
        })
        .collect();

    Some(ProviderEntry {
        name,
        params: ProviderParams(params),
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
    let raw = config::get(key).map_err(|e| LyricsError::new(e.to_string()))?;
    let Some(s) = raw else {
        return Ok(default);
    };

    Ok(match parse_i64(&s) {
        I64Parse::Value(v) => v,
        I64Parse::Saturated(v) => {
            warn!("{key} value '{s}' exceeds 64-bit integer range, saturating to {v}");
            v
        }
        I64Parse::Invalid => {
            warn!("{key} value '{s}' is not a valid integer, using default {default}");
            default
        }
    })
}

#[derive(Debug, PartialEq, Eq)]
enum I64Parse {
    Value(i64),
    Saturated(i64),
    Invalid,
}

fn parse_i64(raw: &str) -> I64Parse {
    if let Ok(v) = raw.parse::<i64>() {
        return I64Parse::Value(v);
    }

    let trimmed = raw.trim();
    let (negative, digits) = match trimmed.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };

    if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
        I64Parse::Saturated(if negative { i64::MIN } else { i64::MAX })
    } else {
        I64Parse::Invalid
    }
}

fn get_optional_i32(key: &str) -> Result<Option<i32>, LyricsError> {
    config::get(key)
        .map_err(|e| LyricsError::new(e.to_string()))
        .map(|v| v.and_then(|s| s.parse().ok()))
}

fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    ["token", "secret", "password", "cookie", "auth"]
        .iter()
        .any(|needle| key.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, params: &[(&str, &str)]) -> ProviderEntry {
        ProviderEntry {
            name: name.to_string(),
            params: ProviderParams(
                params
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            ),
        }
    }

    #[test]
    fn test_parse_i64_normal() {
        assert_eq!(parse_i64("336"), I64Parse::Value(336));
        assert_eq!(parse_i64("-5"), I64Parse::Value(-5));
        assert_eq!(parse_i64(&i64::MAX.to_string()), I64Parse::Value(i64::MAX));
    }

    #[test]
    fn test_parse_i64_overflow_saturates() {
        assert_eq!(
            parse_i64("10000000000000000000000000"),
            I64Parse::Saturated(i64::MAX)
        );
        assert_eq!(
            parse_i64("-10000000000000000000000000"),
            I64Parse::Saturated(i64::MIN)
        );
    }

    #[test]
    fn test_parse_i64_garbage_is_invalid() {
        assert_eq!(parse_i64("abc"), I64Parse::Invalid);
        assert_eq!(parse_i64(""), I64Parse::Invalid);
        assert_eq!(parse_i64("12x"), I64Parse::Invalid);
    }

    #[test]
    fn test_classify_ttl_below_min() {
        assert_eq!(classify_ttl(0), TtlClamp::BelowMin);
        assert_eq!(classify_ttl(-100), TtlClamp::BelowMin);
    }

    #[test]
    fn test_classify_ttl_above_max() {
        assert_eq!(classify_ttl(i64::MAX), TtlClamp::AboveMax);
        assert_eq!(classify_ttl(MAX_CACHE_TTL + 1), TtlClamp::AboveMax);
    }

    #[test]
    fn test_classify_ttl_in_range() {
        assert_eq!(classify_ttl(336), TtlClamp::InRange);
        assert_eq!(classify_ttl(MIN_CACHE_TTL), TtlClamp::InRange);
        assert_eq!(classify_ttl(MAX_CACHE_TTL), TtlClamp::InRange);
    }

    #[test]
    fn test_max_ttl_seconds_fit_go_duration() {
        let max_seconds = MAX_CACHE_TTL.saturating_mul(3600);
        assert!(max_seconds.checked_mul(1_000_000_000).is_some());
    }

    #[test]
    fn test_provider_mode_from_slug() {
        assert_eq!(
            ProviderMode::from_slug("priority"),
            Some(ProviderMode::Priority)
        );
        assert_eq!(
            ProviderMode::from_slug(" Rotation "),
            Some(ProviderMode::Rotation)
        );
        assert_eq!(ProviderMode::from_slug("foo"), None);
    }

    #[test]
    fn test_provider_mode_default_is_priority() {
        assert_eq!(ProviderMode::default(), ProviderMode::Priority);
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
    fn test_cache_id_is_stable() {
        let e = entry("applemusic", &[("mediaUserToken", "abc")]);
        assert_eq!(e.cache_id(), e.cache_id());
    }

    #[test]
    fn test_cache_id_differs_by_name_and_params() {
        assert_ne!(
            entry("lrclib", &[]).cache_id(),
            entry("kugou", &[]).cache_id()
        );
        assert_ne!(
            entry("lrclib", &[("baseUrl", "http://a")]).cache_id(),
            entry("lrclib", &[("baseUrl", "http://b")]).cache_id()
        );
        assert_ne!(
            entry("lrclib", &[]).cache_id(),
            entry("lrclib", &[("baseUrl", "http://a")]).cache_id()
        );
    }

    #[test]
    fn test_display_name_with_param() {
        let e = entry("lrclib", &[("baseUrl", "http://localhost:7592")]);
        assert_eq!(e.display_name(), "lrclib(baseUrl=http://localhost:7592)");
    }

    #[test]
    fn test_display_name_redacts_sensitive_params() {
        let e = entry(
            "applemusic",
            &[("mediaUserToken", "abc"), ("storefront", "gb")],
        );
        assert_eq!(
            e.display_name(),
            "applemusic(mediaUserToken=***, storefront=gb)"
        );
    }

    #[test]
    fn test_display_name_without_param() {
        let e = entry("lrclib", &[]);
        assert_eq!(e.display_name(), "lrclib");
    }

    #[test]
    fn test_parse_providers_json_basic() {
        assert_eq!(
            parse_providers(r#"[{"provider":"lrclib"},{"provider":"lyrics.ovh"}]"#),
            vec![entry("lrclib", &[]), entry("lyrics.ovh", &[])]
        );
    }

    #[test]
    fn test_parse_providers_json_with_base_url() {
        assert_eq!(
            parse_providers(r#"[{"provider":"lrclib","baseUrl":"http://localhost:7592"}]"#),
            vec![entry("lrclib", &[("baseUrl", "http://localhost:7592")])]
        );
    }

    #[test]
    fn test_parse_providers_json_blank_base_url_is_none() {
        assert_eq!(
            parse_providers(r#"[{"provider":"lrclib","baseUrl":"   "}]"#),
            vec![entry("lrclib", &[])]
        );
    }

    #[test]
    fn test_parse_providers_json_named_params() {
        assert_eq!(
            parse_providers(
                r#"[{"provider":"applemusic","mediaUserToken":" abc ","storefront":"gb","baseUrl":""}]"#
            ),
            vec![entry(
                "applemusic",
                &[("mediaUserToken", "abc"), ("storefront", "gb")]
            )]
        );
    }

    #[test]
    fn test_parse_providers_json_bool_and_number_params() {
        assert_eq!(
            parse_providers(
                r#"[{"provider":"applemusic","mediaUserToken":"abc","includeTranslations":true,"storefront":""}]"#
            ),
            vec![entry(
                "applemusic",
                &[("includeTranslations", "true"), ("mediaUserToken", "abc")]
            )]
        );
    }

    #[test]
    fn test_parse_providers_json_skips_empty_provider_and_dedups() {
        assert_eq!(
            parse_providers(
                r#"[{"provider":""},{"provider":"kugou"},{"provider":"kugou"},{"provider":"netease"}]"#
            ),
            vec![entry("kugou", &[]), entry("netease", &[])]
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
