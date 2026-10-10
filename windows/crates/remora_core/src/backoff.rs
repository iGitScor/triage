//! When to ask a tool again after it failed, as `Backoff` on macOS. One failure changes nothing: the next
//! refresh tries again. Then less and less often (every 2, then 4 refreshes…, at most every 30 minutes), so a broken
//! server or token isn't hammered. A rate limit is waited out until the server's reset time, even by a manual refresh.

use chrono::{DateTime, Duration, Utc};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Backoff {
    failures: u32,
    not_before: Option<DateTime<Utc>>,
    rate_limited_until: Option<DateTime<Utc>>,
}

impl Backoff {
    pub const LONGEST_WAIT: Duration = Duration::minutes(30);

    pub fn failed(&mut self, now: DateTime<Utc>, interval: Duration, rate_limited_until: Option<DateTime<Utc>>) {
        self.failures += 1;
        let doubled = interval * 2i32.saturating_pow(self.failures.saturating_sub(1).min(16));
        let wait = doubled.min(Self::LONGEST_WAIT.max(interval));
        // A little early: the next scheduled refresh comes back about one interval later, not to the second.
        let tolerance = Duration::seconds(10).min(interval / 10);
        let mut not_before = now + wait - tolerance;
        self.rate_limited_until = rate_limited_until.map(|until| until.max(now));
        if let Some(until) = self.rate_limited_until {
            not_before = not_before.max(until);
        }
        self.not_before = Some(not_before);
    }

    /// Whether this account sits this refresh out. A manual refresh skips the slow-down, never a rate limit.
    pub fn waits(&self, now: DateTime<Utc>, manual: bool) -> bool {
        if self.rate_limited_until.is_some_and(|until| until > now) {
            return true;
        }
        !manual && self.not_before.is_some_and(|at| at > now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        "2027-01-15T08:00:00Z".parse().unwrap()
    }

    #[test]
    fn one_failure_waits_only_for_the_next_refresh() {
        let mut backoff = Backoff::default();
        backoff.failed(now(), Duration::minutes(5), None);
        assert!(!backoff.waits(now() + Duration::minutes(5), false));
    }

    #[test]
    fn repeated_failures_wait_longer_up_to_half_an_hour() {
        let mut backoff = Backoff::default();
        backoff.failed(now(), Duration::minutes(5), None);
        backoff.failed(now(), Duration::minutes(5), None);
        assert!(backoff.waits(now() + Duration::minutes(5), false), "skips one refresh");
        assert!(!backoff.waits(now() + Duration::minutes(10), false));
        for _ in 0..10 {
            backoff.failed(now(), Duration::minutes(5), None);
        }
        assert!(!backoff.waits(now() + Backoff::LONGEST_WAIT, false), "never longer than 30 minutes");
        assert!(!backoff.waits(now(), true), "a manual refresh doesn't wait for a slow-down");
    }

    #[test]
    fn a_rate_limit_is_waited_out_even_by_a_manual_refresh() {
        let mut backoff = Backoff::default();
        let reset = now() + Duration::minutes(20);
        backoff.failed(now(), Duration::minutes(5), Some(reset));
        assert!(backoff.waits(now() + Duration::minutes(5), true));
        assert!(!backoff.waits(reset + Duration::seconds(1), false));
    }
}
