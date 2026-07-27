use super::host::get_string;
use crate::config::Result;
use extism_pdk::warn;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ProviderParams(BTreeMap<String, String>);

impl ProviderParams {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProviderMode {
    #[default]
    Priority,
    Rotation,
    BestQuality,
}

impl ProviderMode {
    pub fn slug(&self) -> &'static str {
        match self {
            ProviderMode::Priority => "priority",
            ProviderMode::Rotation => "rotation",
            ProviderMode::BestQuality => "quality",
        }
    }

    pub fn from_slug(slug: &str) -> Option<ProviderMode> {
        match slug.trim().to_ascii_lowercase().as_str() {
            "priority" => Some(ProviderMode::Priority),
            "rotation" => Some(ProviderMode::Rotation),
            "quality" => Some(ProviderMode::BestQuality),
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
}

pub(super) fn resolve_list() -> Result<Vec<ProviderEntry>> {
    let Some(raw) = get_string("providersList")? else {
        warn!("no providers configured, no lyrics will be fetched");
        return Ok(Vec::new());
    };

    match parse_providers(&raw) {
        Some(providers) if !providers.is_empty() => Ok(providers),
        Some(_) => {
            warn!("providersList has no usable entries, no lyrics will be fetched");
            Ok(Vec::new())
        }
        None => {
            warn!("providersList is not valid JSON, no lyrics will be fetched");
            Ok(Vec::new())
        }
    }
}

pub(super) fn resolve_mode() -> Result<ProviderMode> {
    let Some(raw) = get_string("providerMode")? else {
        return Ok(ProviderMode::default());
    };

    match ProviderMode::from_slug(&raw) {
        Some(mode) => Ok(mode),
        None => {
            let fallback = ProviderMode::default();
            warn!("unknown providerMode '{raw}', using '{}'", fallback.slug());
            Ok(fallback)
        }
    }
}

fn parse_providers(raw: &str) -> Option<Vec<ProviderEntry>> {
    let rows: Vec<BTreeMap<String, Value>> = serde_json::from_str(raw).ok()?;

    let mut seen = HashSet::new();
    Some(
        rows.into_iter()
            .filter_map(parse_provider_row)
            .filter(|entry| seen.insert(entry.clone()))
            .collect(),
    )
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
    fn test_provider_mode_from_slug() {
        assert_eq!(
            ProviderMode::from_slug("priority"),
            Some(ProviderMode::Priority)
        );
        assert_eq!(
            ProviderMode::from_slug(" Rotation "),
            Some(ProviderMode::Rotation)
        );
        assert_eq!(
            ProviderMode::from_slug("quality"),
            Some(ProviderMode::BestQuality)
        );
        assert_eq!(ProviderMode::from_slug("foo"), None);
    }

    #[test]
    fn test_provider_mode_slug_round_trips() {
        for mode in [
            ProviderMode::Priority,
            ProviderMode::Rotation,
            ProviderMode::BestQuality,
        ] {
            assert_eq!(ProviderMode::from_slug(mode.slug()), Some(mode));
        }
    }

    #[test]
    fn test_provider_mode_default_is_priority() {
        assert_eq!(ProviderMode::default(), ProviderMode::Priority);
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
    fn test_parse_providers_basic() {
        assert_eq!(
            parse_providers(r#"[{"provider":"lrclib"},{"provider":"lyrics.ovh"}]"#),
            Some(vec![entry("lrclib", &[]), entry("lyrics.ovh", &[])])
        );
    }

    #[test]
    fn test_parse_providers_with_base_url() {
        assert_eq!(
            parse_providers(r#"[{"provider":"lrclib","baseUrl":"http://localhost:7592"}]"#),
            Some(vec![entry(
                "lrclib",
                &[("baseUrl", "http://localhost:7592")]
            )])
        );
    }

    #[test]
    fn test_parse_providers_drops_blank_params() {
        assert_eq!(
            parse_providers(r#"[{"provider":"lrclib","baseUrl":"   "}]"#),
            Some(vec![entry("lrclib", &[])])
        );
    }

    #[test]
    fn test_parse_providers_named_params() {
        assert_eq!(
            parse_providers(
                r#"[{"provider":"applemusic","mediaUserToken":" abc ","storefront":"gb","baseUrl":""}]"#
            ),
            Some(vec![entry(
                "applemusic",
                &[("mediaUserToken", "abc"), ("storefront", "gb")]
            )])
        );
    }

    #[test]
    fn test_parse_providers_coerces_bool_and_number() {
        assert_eq!(
            parse_providers(
                r#"[{"provider":"applemusic","mediaUserToken":"abc","includeTranslations":true,"storefront":""}]"#
            ),
            Some(vec![entry(
                "applemusic",
                &[("includeTranslations", "true"), ("mediaUserToken", "abc")]
            )])
        );
    }

    #[test]
    fn test_parse_providers_skips_unnamed_and_dedups() {
        assert_eq!(
            parse_providers(
                r#"[{"provider":""},{"provider":"kugou"},{"provider":"kugou"},{"provider":"netease"}]"#
            ),
            Some(vec![entry("kugou", &[]), entry("netease", &[])])
        );
    }

    #[test]
    fn test_parse_providers_invalid_json_is_none() {
        assert_eq!(parse_providers("not json"), None);
    }

    #[test]
    fn test_parse_providers_empty_array_is_empty() {
        assert_eq!(parse_providers("[]"), Some(Vec::new()));
    }
}
