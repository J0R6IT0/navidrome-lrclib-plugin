use crate::types::{Lyrics, LyricsKind, SyncLevel};

pub(super) enum Ranker<'a> {
    TypePriority(&'a [LyricsKind]),
    SyncLevel(&'a [LyricsKind]),
}

impl Ranker<'_> {
    pub(super) fn rank(&self, lyrics: &Lyrics) -> usize {
        match self {
            Ranker::TypePriority(priority) => kind_rank(lyrics.kind(), priority),
            Ranker::SyncLevel(_) => lyrics.sync_level().rank(),
        }
    }

    pub(super) fn ceiling(&self, kinds: &[LyricsKind]) -> Option<usize> {
        match self {
            Ranker::TypePriority(priority) => best_rank_for(kinds, priority),
            Ranker::SyncLevel(wanted) => kinds
                .iter()
                .filter(|kind| wanted.contains(kind))
                .map(|kind| kind.max_sync_level().rank())
                .min(),
        }
    }

    pub(super) fn describe_rank(&self, rank: usize) -> &'static str {
        match self {
            Ranker::TypePriority(priority) => priority.get(rank).map_or("none", |kind| kind.slug()),
            Ranker::SyncLevel(_) => SyncLevel::from_rank(rank).map_or("none", |l| l.slug()),
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
        let ranker = Ranker::SyncLevel(&[Ttml, Elrc, Lrc, Plain]);

        assert_eq!(ranker.rank(&Lyrics::Elrc(String::new())), 0);
        assert_eq!(ranker.rank(&Lyrics::Lrc(String::new())), 1);
        assert_eq!(ranker.rank(&Lyrics::Plain(String::new())), 2);
    }

    #[test]
    fn test_sync_ranker_reads_the_level_out_of_ttml() {
        let ranker = Ranker::SyncLevel(&[Ttml]);
        let word = Lyrics::Ttml(r#"<tt itunes:timing="Word"></tt>"#.to_string());
        let plain = Lyrics::Ttml(r#"<tt itunes:timing="None"></tt>"#.to_string());

        assert_eq!(ranker.rank(&word), 0);
        assert_eq!(ranker.rank(&plain), 2);
    }

    #[test]
    fn test_sync_ranker_ceiling_uses_the_finest_wanted_kind() {
        let ranker = Ranker::SyncLevel(&[Ttml, Elrc, Lrc, Plain]);

        assert_eq!(ranker.ceiling(&[Lrc, Plain]), Some(1));
        assert_eq!(ranker.ceiling(&[Plain]), Some(2));
        assert_eq!(ranker.ceiling(&[Ttml]), Some(0));
    }

    #[test]
    fn test_sync_ranker_ceiling_ignores_disabled_kinds() {
        let ranker = Ranker::SyncLevel(&[Lrc, Plain]);

        assert_eq!(ranker.ceiling(&[Elrc, Lrc]), Some(1));
        assert_eq!(ranker.ceiling(&[Srt]), None);
    }

    #[test]
    fn test_type_priority_ranker_matches_the_priority_list() {
        let ranker = Ranker::TypePriority(&[Ttml, Elrc, Lrc, Plain]);

        assert_eq!(ranker.rank(&Lyrics::Ttml(String::new())), 0);
        assert_eq!(ranker.rank(&Lyrics::Lrc(String::new())), 2);
        assert_eq!(ranker.ceiling(&[Lrc, Elrc]), Some(1));
    }

    #[test]
    fn test_describe_rank_per_mode() {
        assert_eq!(Ranker::TypePriority(&[Ttml, Lrc]).describe_rank(1), "lrc");
        assert_eq!(Ranker::SyncLevel(&[Ttml]).describe_rank(0), "word-by-word");
        assert_eq!(Ranker::SyncLevel(&[Ttml]).describe_rank(9), "none");
    }
}
