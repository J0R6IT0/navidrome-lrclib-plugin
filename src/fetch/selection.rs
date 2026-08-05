use crate::config::{PluginConfig, ProviderEntry, ProviderMode};
use extism_pdk::warn;
use nd_pdk::host::cache;

const ROTATION_KEY: &str = "rotation:cursor";

const ROTATION_TTL_SECONDS: i64 = 3600;

pub(super) fn order_providers(cfg: &PluginConfig) -> Vec<&ProviderEntry> {
    match cfg.provider_mode {
        ProviderMode::Priority | ProviderMode::TypePriority | ProviderMode::BestSyncLevel => {
            cfg.providers.iter().collect()
        }
        ProviderMode::Rotation => rotate(&cfg.providers),
    }
}

fn rotate(providers: &[ProviderEntry]) -> Vec<&ProviderEntry> {
    if providers.is_empty() {
        return Vec::new();
    }

    let start = load_cursor() % providers.len();
    store_cursor((start + 1) % providers.len());

    providers
        .iter()
        .cycle()
        .skip(start)
        .take(providers.len())
        .collect()
}

fn load_cursor() -> usize {
    cache::get_bytes(ROTATION_KEY)
        .ok()
        .flatten()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

fn store_cursor(value: usize) {
    if let Err(err) = cache::set_bytes(
        ROTATION_KEY,
        value.to_string().into_bytes(),
        ROTATION_TTL_SECONDS,
    ) {
        warn!("failed to persist rotation cursor: {err}");
    }
}
