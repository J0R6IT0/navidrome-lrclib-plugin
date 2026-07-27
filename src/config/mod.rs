use crate::types::LyricsKind;
use extism_pdk::warn;
use host::{get_bool, get_f64, get_optional_i32, get_string};
use nd_pdk::lyrics::Error;

mod host;
mod providers;
mod ttl;

pub use providers::{ProviderEntry, ProviderMode, ProviderParams};
pub use ttl::TypeCacheTtls;

const DEFAULT_LYRICS_FORMATS: [LyricsKind; 2] = [LyricsKind::Lrc, LyricsKind::Plain];

const DEFAULT_PLAIN_EXTENSION: &str = "txt";
const DEFAULT_INSTRUMENTAL_EXTENSION: &str = "txt";

const DEFAULT_DURATION_TOLERANCE_SECS: f32 = 3.0;
const MIN_DURATION_TOLERANCE_SECS: f32 = 1.0;
const MAX_DURATION_TOLERANCE_SECS: f32 = 3600.0;

const DEFAULT_INSTRUMENTAL_TEXT: &str = "Instrumental";
const DEFAULT_FOLDER_TEMPLATE: &str = "_lyrics/{type}/{track:album_artist} - {track:album}/{track:disc_number:2} - {track:track_number:2} {track:title}";

type Result<T> = std::result::Result<T, Error>;

pub struct PluginConfig {
    pub lyrics_type_priority: Vec<LyricsKind>,
    pub write_lyrics: bool,
    pub overwrite_lyrics: bool,
    pub plain_extension: String,
    pub instrumental_extension: String,
    pub enable_cache: bool,
    pub per_type_cache_ttl: bool,
    pub cache_ttl_hours: i64,
    pub type_cache_ttl_hours: TypeCacheTtls,
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
            lyrics_type_priority: DEFAULT_LYRICS_FORMATS.to_vec(),
            write_lyrics: false,
            overwrite_lyrics: false,
            plain_extension: DEFAULT_PLAIN_EXTENSION.to_string(),
            instrumental_extension: DEFAULT_INSTRUMENTAL_EXTENSION.to_string(),
            enable_cache: true,
            per_type_cache_ttl: false,
            cache_ttl_hours: ttl::DEFAULT_CACHE_TTL,
            type_cache_ttl_hours: TypeCacheTtls::default(),
            negative_cache: true,
            negative_cache_ttl_hours: ttl::DEFAULT_NEGATIVE_CACHE_TTL,
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
    pub fn load() -> Result<Self> {
        let per_type_cache_ttl = get_bool("perTypeCacheTtl", false)?;

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
            per_type_cache_ttl,
            cache_ttl_hours: ttl::resolve_global()?,
            type_cache_ttl_hours: if per_type_cache_ttl {
                // This costs several host round-trips, so only call it
                // when per-type mode is actually enabled.
                ttl::resolve_per_type()?
            } else {
                TypeCacheTtls::default()
            },
            negative_cache: get_bool("negativeCache", true)?,
            negative_cache_ttl_hours: ttl::resolve_negative()?,
            providers: providers::resolve_list()?,
            provider_mode: providers::resolve_mode()?,
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

    pub fn wants(&self, kind: LyricsKind) -> bool {
        self.lyrics_type_priority.contains(&kind)
    }

    pub fn cache_ttl_hours_for(&self, kind: LyricsKind) -> i64 {
        if self.per_type_cache_ttl {
            self.type_cache_ttl_hours.get(kind)
        } else {
            self.cache_ttl_hours
        }
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

fn resolve_lyrics_type_priority() -> Result<Vec<LyricsKind>> {
    let order = match get_string("lyricsFormats")? {
        Some(raw) => parse_lyrics_formats(&raw),
        None => Vec::new(),
    };

    if order.is_empty() {
        let fallback = DEFAULT_LYRICS_FORMATS.map(|kind| kind.slug()).join(" + ");
        warn!("no lyrics formats enabled, defaulting to {fallback}");
        return Ok(DEFAULT_LYRICS_FORMATS.to_vec());
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

fn resolve_extension(key: &str, default_value: &str) -> Result<String> {
    let extension = get_string(key)?
        .map(|s| normalize_extension(&s))
        .unwrap_or_else(|| default_value.to_string());

    if extension.is_empty() {
        warn!("{key} resolved to empty string, using '{default_value}'");
        return Ok(default_value.to_string());
    }

    Ok(extension)
}

fn normalize_extension(ext: &str) -> String {
    ext.trim()
        .trim_start_matches('.')
        .chars()
        .filter(|c| {
            !c.is_control() && !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
        .collect()
}

fn resolve_duration_tolerance() -> Result<f32> {
    let raw = get_f64(
        "durationToleranceSeconds",
        DEFAULT_DURATION_TOLERANCE_SECS as f64,
    )? as f32;

    let clamped = raw.clamp(MIN_DURATION_TOLERANCE_SECS, MAX_DURATION_TOLERANCE_SECS);
    if clamped != raw {
        warn!("durationToleranceSeconds {raw} is out of range, clamping to {clamped}s");
    }

    Ok(clamped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_matches_the_load_time_fallback() {
        let cfg = PluginConfig::default();

        assert_eq!(cfg.lyrics_type_priority, DEFAULT_LYRICS_FORMATS);
        assert!(cfg.wants(LyricsKind::Lrc));
        assert!(!cfg.wants(LyricsKind::Ttml));
    }

    #[test]
    fn test_per_type_cache_ttl_defaults_to_off() {
        assert!(!PluginConfig::default().per_type_cache_ttl);
    }

    #[test]
    fn test_cache_ttl_uses_global_when_per_type_off() {
        let cfg = PluginConfig {
            per_type_cache_ttl: false,
            cache_ttl_hours: 168,
            ..PluginConfig::default()
        };

        assert_eq!(cfg.cache_ttl_hours_for(LyricsKind::Ttml), 168);
        assert_eq!(cfg.cache_ttl_hours_for(LyricsKind::Plain), 168);
    }

    #[test]
    fn test_cache_ttl_uses_type_ttls_when_per_type_on() {
        let cfg = PluginConfig {
            per_type_cache_ttl: true,
            cache_ttl_hours: 168,
            type_cache_ttl_hours: TypeCacheTtls {
                plain: 1,
                lrc: 2,
                elrc: 3,
                ttml: 4,
                srt: 5,
                lyricsfile: 6,
                instrumental: 7,
            },
            ..PluginConfig::default()
        };

        assert_eq!(cfg.cache_ttl_hours_for(LyricsKind::Plain), 1);
        assert_eq!(cfg.cache_ttl_hours_for(LyricsKind::Lrc), 2);
        assert_eq!(cfg.cache_ttl_hours_for(LyricsKind::Elrc), 3);
        assert_eq!(cfg.cache_ttl_hours_for(LyricsKind::Ttml), 4);
        assert_eq!(cfg.cache_ttl_hours_for(LyricsKind::Srt), 5);
        assert_eq!(cfg.cache_ttl_hours_for(LyricsKind::Lyricsfile), 6);
        assert_eq!(cfg.cache_ttl_hours_for(LyricsKind::Instrumental), 7);
    }

    #[test]
    fn test_duration_tolerance_ms_keeps_fractional_seconds() {
        let cfg = PluginConfig {
            duration_tolerance_secs: 2.5,
            ..PluginConfig::default()
        };

        assert_eq!(cfg.duration_tolerance_ms(), 2500);
    }

    #[test]
    fn test_parse_lyrics_formats_preserves_order() {
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
    fn test_parse_lyrics_formats_trims_and_ignores_case() {
        assert_eq!(
            parse_lyrics_formats(" LRC , Plain "),
            vec![LyricsKind::Lrc, LyricsKind::Plain]
        );
    }

    #[test]
    fn test_parse_lyrics_formats_dedups_and_skips_unknown() {
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
    fn test_normalize_extension_strips_dots_and_whitespace() {
        assert_eq!(normalize_extension("lrc"), "lrc");
        assert_eq!(normalize_extension(".lrc"), "lrc");
        assert_eq!(normalize_extension("...lrc"), "lrc");
        assert_eq!(normalize_extension("  .txt  "), "txt");
        assert_eq!(normalize_extension("."), "");
    }

    #[test]
    fn test_normalize_extension_strips_path_separators() {
        for raw in ["txt/../../evil", "../../etc/passwd", r"txt\..\evil", "a:b"] {
            let ext = normalize_extension(raw);
            assert!(
                !ext.contains(['/', '\\', ':']),
                "{raw:?} left separators in {ext:?}"
            );
        }

        assert_eq!(normalize_extension("///"), "");
    }
}
