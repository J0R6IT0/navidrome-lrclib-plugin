use extism_pdk::warn;
use nd_pdk::host::cache;
use std::time::{SystemTime, UNIX_EPOCH};

const KEY_PREFIX: &str = "ratelimit:";

pub(super) fn remaining(provider_id: &str) -> Option<i64> {
    let deadline = cache::get_int(&key(provider_id)).ok().flatten()?;
    time_left(deadline, now())
}

pub(super) fn record(provider_id: &str, retry_after_secs: i64) {
    let deadline = now().saturating_add(retry_after_secs);

    if let Err(err) = cache::set_int(&key(provider_id), deadline, retry_after_secs) {
        warn!("failed to persist the rate limit for provider {provider_id}: {err}");
    }
}

fn key(provider_id: &str) -> String {
    format!("{KEY_PREFIX}{provider_id}")
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since_epoch| since_epoch.as_secs() as i64)
}

fn time_left(deadline: i64, now: i64) -> Option<i64> {
    (deadline > now).then(|| deadline - now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn check_time_left(deadline: i64, now: i64, expected: Option<i64>) {
        assert_eq!(
            time_left(deadline, now),
            expected,
            "deadline {deadline} at {now}"
        );
    }

    #[test]
    fn a_deadline_ahead_is_a_wait() {
        check_time_left(1_700_000_060, 1_700_000_000, Some(60));
        check_time_left(1_700_000_001, 1_700_000_000, Some(1));
    }

    #[test]
    fn a_deadline_behind_is_no_wait() {
        check_time_left(1_700_000_000, 1_700_000_000, None);
        check_time_left(1_699_999_940, 1_700_000_000, None);
    }

    #[test]
    fn each_provider_gets_its_own_deadline() {
        assert_eq!(key("abc123"), "ratelimit:abc123");
        assert_ne!(key("abc123"), key("def456"));
    }
}
