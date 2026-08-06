use crate::config::{PluginConfig, ProviderMode};
use crate::types::{Lyrics, LyricsKind, SyncLevel};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Rank {
    primary: usize,
    censored: bool,
}

impl Rank {
    pub(super) const BEST: Rank = Rank {
        primary: 0,
        censored: false,
    };

    pub(super) fn is_censored(&self) -> bool {
        self.censored
    }
}

pub(super) struct Ranker<'a> {
    basis: Basis<'a>,
    prefer_uncensored: bool,
}

enum Basis<'a> {
    TypePriority(&'a [LyricsKind]),
    SyncLevel(&'a [LyricsKind]),
}

impl<'a> Ranker<'a> {
    pub(super) fn for_mode(cfg: &'a PluginConfig) -> Option<Self> {
        let priority = &cfg.lyrics_type_priority;

        let basis = match cfg.provider_mode {
            ProviderMode::TypePriority => Basis::TypePriority(priority),
            ProviderMode::BestSyncLevel => Basis::SyncLevel(priority),
            ProviderMode::Priority | ProviderMode::Rotation => return None,
        };

        Some(Self {
            basis,
            prefer_uncensored: cfg.prefer_uncensored,
        })
    }

    pub(super) fn rank(&self, lyrics: &Lyrics) -> Rank {
        Rank {
            primary: match &self.basis {
                Basis::TypePriority(priority) => kind_rank(lyrics.kind(), priority),
                Basis::SyncLevel(_) => lyrics.sync_level().rank(),
            },
            censored: self.prefer_uncensored && lyrics.is_censored(),
        }
    }

    pub(super) fn ceiling(&self, kinds: &[LyricsKind]) -> Option<Rank> {
        let primary = match &self.basis {
            Basis::TypePriority(priority) => best_rank_for(kinds, priority),
            Basis::SyncLevel(wanted) => kinds
                .iter()
                .filter(|kind| wanted.contains(kind))
                .map(|kind| kind.max_sync_level().rank())
                .min(),
        }?;

        Some(Rank {
            primary,
            censored: false,
        })
    }

    pub(super) fn describe(&self, rank: Rank) -> String {
        let primary = match &self.basis {
            Basis::TypePriority(priority) => priority.get(rank.primary).map(|kind| kind.slug()),
            Basis::SyncLevel(_) => SyncLevel::from_rank(rank.primary).map(|level| level.slug()),
        };

        match (primary, rank.censored) {
            (None, _) => "none".to_string(),
            (Some(name), true) => format!("censored {name}"),
            (Some(name), false) => name.to_string(),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LyricsKind::{Elrc, Lrc, Plain, Srt, Ttml};

    fn type_priority(priority: &[LyricsKind]) -> Ranker<'_> {
        Ranker {
            basis: Basis::TypePriority(priority),
            prefer_uncensored: false,
        }
    }

    fn sync(wanted: &[LyricsKind]) -> Ranker<'_> {
        Ranker {
            basis: Basis::SyncLevel(wanted),
            prefer_uncensored: false,
        }
    }

    fn sync_uncensored(wanted: &[LyricsKind]) -> Ranker<'_> {
        Ranker {
            basis: Basis::SyncLevel(wanted),
            prefer_uncensored: true,
        }
    }

    fn rank(primary: usize, censored: bool) -> Rank {
        Rank { primary, censored }
    }

    fn clean_lrc() -> Lyrics {
        Lyrics::Lrc("[00:01.00]Be humble hol' up".to_string())
    }

    fn censored_lrc() -> Lyrics {
        Lyrics::Lrc("[00:01.00]B**ch be humble hol' up".to_string())
    }

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
    fn test_sync_ranker_orders_by_sync_level() {
        let ranker = sync(&[Ttml, Elrc, Lrc, Plain]);

        assert_eq!(ranker.rank(&Lyrics::Elrc(String::new())), rank(0, false));
        assert_eq!(ranker.rank(&Lyrics::Lrc(String::new())), rank(1, false));
        assert_eq!(ranker.rank(&Lyrics::Plain(String::new())), rank(2, false));
    }

    #[test]
    fn test_sync_ranker_reads_the_level_out_of_ttml() {
        let ranker = sync(&[Ttml]);
        let word = Lyrics::Ttml(r#"<tt itunes:timing="Word"></tt>"#.to_string());
        let plain = Lyrics::Ttml(r#"<tt itunes:timing="None"></tt>"#.to_string());

        assert_eq!(ranker.rank(&word), rank(0, false));
        assert_eq!(ranker.rank(&plain), rank(2, false));
    }

    #[test]
    fn test_sync_ranker_ceiling_uses_the_finest_wanted_kind() {
        let ranker = sync(&[Ttml, Elrc, Lrc, Plain]);

        assert_eq!(ranker.ceiling(&[Lrc, Plain]), Some(rank(1, false)));
        assert_eq!(ranker.ceiling(&[Plain]), Some(rank(2, false)));
        assert_eq!(ranker.ceiling(&[Ttml]), Some(rank(0, false)));
    }

    #[test]
    fn test_sync_ranker_ceiling_ignores_disabled_kinds() {
        let ranker = sync(&[Lrc, Plain]);

        assert_eq!(ranker.ceiling(&[Elrc, Lrc]), Some(rank(1, false)));
        assert_eq!(ranker.ceiling(&[Srt]), None);
    }

    #[test]
    fn test_type_priority_ranker_matches_the_priority_list() {
        let ranker = type_priority(&[Ttml, Elrc, Lrc, Plain]);

        assert_eq!(ranker.rank(&Lyrics::Ttml(String::new())), rank(0, false));
        assert_eq!(ranker.rank(&Lyrics::Lrc(String::new())), rank(2, false));
        assert_eq!(ranker.ceiling(&[Lrc, Elrc]), Some(rank(1, false)));
    }

    #[test]
    fn test_describe_per_mode() {
        assert_eq!(type_priority(&[Ttml, Lrc]).describe(rank(1, false)), "lrc");
        assert_eq!(sync(&[Ttml]).describe(rank(0, false)), "word-by-word");
        assert_eq!(sync(&[Ttml]).describe(rank(9, false)), "none");
    }

    #[test]
    fn test_describe_marks_censored_results() {
        assert_eq!(
            type_priority(&[Ttml, Lrc]).describe(rank(1, true)),
            "censored lrc"
        );
        assert_eq!(sync(&[Ttml]).describe(rank(usize::MAX, true)), "none");
    }

    #[test]
    fn test_is_censored_tracks_the_option() {
        assert!(!sync(&[Lrc]).rank(&censored_lrc()).is_censored());
        assert!(sync_uncensored(&[Lrc]).rank(&censored_lrc()).is_censored());
        assert!(!sync_uncensored(&[Lrc]).rank(&clean_lrc()).is_censored());
    }

    #[test]
    fn test_censoring_is_ignored_unless_the_option_is_on() {
        assert_eq!(sync(&[Lrc]).rank(&censored_lrc()), rank(1, false));
        assert_eq!(sync_uncensored(&[Lrc]).rank(&censored_lrc()), rank(1, true));
        assert_eq!(sync_uncensored(&[Lrc]).rank(&clean_lrc()), rank(1, false));
    }

    #[test]
    fn test_sync_level_outranks_censoring() {
        let ranker = sync_uncensored(&[Elrc, Lrc]);
        let censored_word = Lyrics::Elrc("[00:01.00]<00:01.00>f**k".to_string());

        assert!(ranker.rank(&censored_word) < ranker.rank(&clean_lrc()));
    }

    #[test]
    fn test_censoring_breaks_ties_at_the_same_level() {
        let ranker = sync_uncensored(&[Lrc]);
        assert!(ranker.rank(&clean_lrc()) < ranker.rank(&censored_lrc()));
    }

    #[test]
    fn test_ceiling_can_still_beat_a_censored_best() {
        let ranker = sync_uncensored(&[Lrc]);
        let censored_best = ranker.rank(&censored_lrc());

        assert!(ranker.ceiling(&[Lrc]).unwrap() < censored_best);
    }

    #[test]
    fn test_ceiling_cannot_beat_a_clean_best_at_the_same_level() {
        let ranker = sync_uncensored(&[Lrc]);
        assert!(ranker.ceiling(&[Lrc]).unwrap() >= ranker.rank(&clean_lrc()));
    }

    #[test]
    fn test_rank_best_is_word_level_and_clean() {
        let ranker = sync_uncensored(&[Elrc]);
        assert_eq!(ranker.rank(&Lyrics::Elrc("clean".to_string())), Rank::BEST);
    }

    #[test]
    fn test_for_mode_only_ranks_the_ranked_modes() {
        use crate::config::ProviderMode;

        for (mode, ranked) in [
            (ProviderMode::TypePriority, true),
            (ProviderMode::BestSyncLevel, true),
            (ProviderMode::Priority, false),
            (ProviderMode::Rotation, false),
        ] {
            let cfg = PluginConfig {
                provider_mode: mode,
                ..PluginConfig::default()
            };
            assert_eq!(Ranker::for_mode(&cfg).is_some(), ranked, "{mode:?}");
        }
    }

    #[test]
    fn test_for_mode_carries_the_prefer_uncensored_flag() {
        let cfg = PluginConfig {
            provider_mode: ProviderMode::BestSyncLevel,
            prefer_uncensored: true,
            lyrics_type_priority: vec![Lrc],
            ..PluginConfig::default()
        };

        let ranker = Ranker::for_mode(&cfg).unwrap();
        assert_eq!(ranker.rank(&censored_lrc()), rank(1, true));
    }
}
