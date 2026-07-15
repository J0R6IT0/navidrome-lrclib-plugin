use crate::cache::LyricsCache;
use crate::config::{PluginConfig, ProviderEntry, ProviderMode};
use crate::providers::{LyricsProvider, register_providers};
use crate::registry::ProviderRegistry;
use crate::selection;
use crate::types::{Lyrics, LyricsKind};
use extism_pdk::{info, warn};
use nd_pdk::lyrics::TrackInfo;

pub enum Outcome {
    Found(Lyrics),
    NotFound,
    ProviderError,
}

pub fn run(track: &TrackInfo, cfg: &PluginConfig, cache: &Option<LyricsCache>) -> Outcome {
    let mut registry = ProviderRegistry::new();
    register_providers(&mut registry);

    Orchestrator {
        registry: &registry,
        track,
        cfg,
        cache,
    }
    .run()
}

struct Orchestrator<'a> {
    registry: &'a ProviderRegistry,
    track: &'a TrackInfo,
    cfg: &'a PluginConfig,
    cache: &'a Option<LyricsCache>,
}

impl Orchestrator<'_> {
    fn run(&self) -> Outcome {
        match self.cfg.provider_mode {
            ProviderMode::BestQuality => self.best_quality(),
            ProviderMode::Priority | ProviderMode::Rotation => self.sequential(),
        }
    }

    fn sequential(&self) -> Outcome {
        let mut had_error = false;

        for entry in selection::order_providers(self.cfg) {
            let Some(prepared) = self.prepare(entry) else {
                continue;
            };

            match self.fetch_one(&prepared) {
                ProviderFetch::Lyrics(lyrics) => return Outcome::Found(lyrics),
                ProviderFetch::Error => had_error = true,
                ProviderFetch::Empty => {}
            }
        }

        outcome_without_lyrics(had_error)
    }

    fn best_quality(&self) -> Outcome {
        let priority = self.cfg.resolve_order();
        let mut best: Option<Lyrics> = None;
        let mut best_rank = usize::MAX;
        let mut had_error = false;

        for entry in &self.cfg.providers {
            let Some(prepared) = self.prepare(entry) else {
                continue;
            };

            let ceiling = best_rank_for(prepared.provider.supported_kinds(), priority);
            if ceiling.is_some_and(|rank| rank >= best_rank) {
                info!(
                    "skipping provider '{}': best possible ({}) cannot improve on current {} lyrics",
                    prepared.label,
                    ceiling
                        .and_then(|rank| priority.get(rank))
                        .map(LyricsKind::slug)
                        .unwrap_or("none"),
                    best.as_ref().map(|l| l.kind().slug()).unwrap_or("none"),
                );
                continue;
            }

            match self.fetch_one(&prepared) {
                ProviderFetch::Lyrics(lyrics) => {
                    if matches!(lyrics, Lyrics::Instrumental) {
                        return Outcome::Found(lyrics);
                    }

                    let rank = kind_rank(lyrics.kind(), priority);
                    if best.is_none() || rank < best_rank {
                        best_rank = rank;
                        best = Some(lyrics);
                    }

                    if best_rank == 0 {
                        break;
                    }
                }
                ProviderFetch::Error => had_error = true,
                ProviderFetch::Empty => {}
            }
        }

        match best {
            Some(lyrics) => Outcome::Found(lyrics),
            None => outcome_without_lyrics(had_error),
        }
    }

    fn prepare(&self, entry: &ProviderEntry) -> Option<PreparedProvider> {
        let Some(provider) = self.registry.create(entry) else {
            warn!("unknown provider '{}', skipping", entry.name);
            return None;
        };

        if !provider
            .supported_kinds()
            .iter()
            .any(|&kind| self.cfg.wants(kind))
        {
            return None;
        }

        Some(PreparedProvider {
            label: provider_label(&entry.name, &provider.log_params()),
            provider_id: entry.cache_id(),
            provider,
        })
    }

    fn fetch_one(&self, prepared: &PreparedProvider) -> ProviderFetch {
        let PreparedProvider {
            provider,
            label,
            provider_id,
        } = prepared;

        if self.is_negative(provider_id) {
            info!(
                "provider '{}' negative-cached for this track, skipping",
                label
            );
            return ProviderFetch::Empty;
        }

        info!("trying provider '{}'", label);
        match provider.fetch_lyrics(self.track, self.cfg) {
            Ok(Some(mut lyrics)) => {
                lyrics.sanitize(self.cfg);
                if lyrics.is_empty() {
                    warn!(
                        "provider '{}' returned empty lyrics after sanitization, skipping",
                        label
                    );
                    self.mark_negative(provider_id);
                    ProviderFetch::Empty
                } else {
                    info!(
                        "provider '{}' returned {} lyrics",
                        label,
                        lyrics.kind().slug()
                    );
                    ProviderFetch::Lyrics(lyrics)
                }
            }
            Ok(None) => {
                info!("provider '{}' returned no lyrics", label);
                self.mark_negative(provider_id);
                ProviderFetch::Empty
            }
            Err(e) => {
                warn!("provider '{}' failed: {}", label, e);
                ProviderFetch::Error
            }
        }
    }

    fn is_negative(&self, provider_id: &str) -> bool {
        self.cache
            .as_ref()
            .is_some_and(|cache| cache.is_negative(&self.track.id, provider_id))
    }

    fn mark_negative(&self, provider_id: &str) {
        if !self.cfg.negative_cache {
            return;
        }

        if let Some(cache) = self.cache {
            match cache.write_negative(&self.track.id, provider_id) {
                Ok(()) => info!(
                    "cached negative result for track '{}' (provider {provider_id}, ttl {}h)",
                    self.track.id, self.cfg.negative_cache_ttl_hours
                ),
                Err(err) => warn!("failed to persist negative cache entry: {err}"),
            }
        }
    }
}

fn outcome_without_lyrics(had_error: bool) -> Outcome {
    if had_error {
        Outcome::ProviderError
    } else {
        Outcome::NotFound
    }
}

struct PreparedProvider {
    provider: Box<dyn LyricsProvider>,
    label: String,
    provider_id: String,
}

enum ProviderFetch {
    Lyrics(Lyrics),
    Empty,
    Error,
}

fn kind_rank(kind: LyricsKind, priority: &[LyricsKind]) -> usize {
    priority
        .iter()
        .position(|&k| k == kind)
        .unwrap_or(usize::MAX)
}

fn best_rank_for(kinds: &[LyricsKind], priority: &[LyricsKind]) -> Option<usize> {
    kinds
        .iter()
        .map(|&kind| kind_rank(kind, priority))
        .filter(|&rank| rank != usize::MAX)
        .min()
}

fn provider_label(name: &str, params: &[(&'static str, String)]) -> String {
    if params.is_empty() {
        return name.to_string();
    }

    let joined = params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(", ");

    format!("{name}({joined})")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LyricsKind::{Elrc, Lrc, Plain, Srt, Ttml};

    #[test]
    fn test_kind_rank_orders_by_priority() {
        let priority = [Ttml, Elrc, Lrc, Plain];
        assert_eq!(kind_rank(Ttml, &priority), 0);
        assert_eq!(kind_rank(Elrc, &priority), 1);
        assert_eq!(kind_rank(Plain, &priority), 3);
    }

    #[test]
    fn test_kind_rank_absent_is_max() {
        let priority = [Lrc, Plain];
        assert_eq!(kind_rank(Ttml, &priority), usize::MAX);
    }

    #[test]
    fn test_best_rank_for_picks_highest_supported() {
        let priority = [Ttml, Elrc, Lrc, Plain];
        assert_eq!(best_rank_for(&[Lrc, Elrc], &priority), Some(1));
        assert_eq!(best_rank_for(&[Plain], &priority), Some(3));
    }

    #[test]
    fn test_best_rank_for_none_when_nothing_wanted() {
        let priority = [Lrc, Plain];
        assert_eq!(best_rank_for(&[Srt], &priority), None);
    }

    #[test]
    fn test_best_rank_for_ignores_unwanted_kinds() {
        let priority = [Lrc, Plain];
        assert_eq!(best_rank_for(&[Srt, Lrc], &priority), Some(0));
    }

    #[test]
    fn test_provider_label_without_params() {
        assert_eq!(provider_label("kugou", &[]), "kugou");
    }

    #[test]
    fn test_provider_label_with_params() {
        let params = [("baseUrl", "http://localhost:7592".to_string())];
        assert_eq!(
            provider_label("lrclib", &params),
            "lrclib(baseUrl=http://localhost:7592)"
        );
    }
}
