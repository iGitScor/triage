//! What the assistant used on this computer, per day: requests and tokens, for the last `KEPT_DAYS` days with
//! any. Tokens only, no price: Remora doesn't know what each plan charges. A port of the macOS `AssistantUsage.swift`.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The tokens one assistant answer used, as its provider reported them. Input counts the cached prompt too.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
}

/// One local day's use, or a month's added up.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageDay {
    /// "2026-10-15" (a month: "2026-10").
    pub day: String,
    pub requests: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl UsageDay {
    pub fn tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

/// Enough for this month's total, and small.
pub const KEPT_DAYS: usize = 31;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AssistantUsage {
    /// Oldest first.
    pub days: Vec<UsageDay>,
}

impl AssistantUsage {
    /// One request sent on `today` (the local date), with the tokens its answers reported (None: the provider said
    /// nothing).
    pub fn record(&mut self, tokens: Option<TokenUsage>, today: NaiveDate) {
        let key = key(today);
        if self.days.last().is_none_or(|d| d.day != key) {
            self.days.push(UsageDay { day: key, ..UsageDay::default() });
        }
        let day = self.days.last_mut().expect("just added");
        day.requests += 1;
        day.input_tokens += tokens.map_or(0, |t| t.input);
        day.output_tokens += tokens.map_or(0, |t| t.output);
        let extra = self.days.len().saturating_sub(KEPT_DAYS);
        self.days.drain(..extra);
    }

    pub fn today(&self, today: NaiveDate) -> UsageDay {
        let key = key(today);
        self.days.iter().rev().find(|d| d.day == key).cloned().unwrap_or(UsageDay { day: key, ..UsageDay::default() })
    }

    /// This calendar month's days added up.
    pub fn this_month(&self, today: NaiveDate) -> UsageDay {
        let month = today.format("%Y-%m").to_string();
        self.days.iter().filter(|d| d.day.starts_with(&month)).fold(
            UsageDay { day: month.clone(), ..UsageDay::default() },
            |mut total, d| {
                total.requests += d.requests;
                total.input_tokens += d.input_tokens;
                total.output_tokens += d.output_tokens;
                total
            },
        )
    }

    /// Whether another request fits today's limit (`limit` requests a day; 0 is no limit).
    pub fn allows(&self, limit: u32, today: NaiveDate) -> bool {
        limit == 0 || self.today(today).requests < limit
    }
}

fn key(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    fn tokens(input: u64, output: u64) -> Option<TokenUsage> {
        Some(TokenUsage { input, output })
    }

    #[test]
    fn records_requests_and_tokens_per_day() {
        let mut usage = AssistantUsage::default();
        usage.record(tokens(1_000, 200), date("2026-10-15"));
        usage.record(None, date("2026-10-15"));
        let today = usage.today(date("2026-10-15"));
        assert_eq!(today, UsageDay { day: "2026-10-15".into(), requests: 2, input_tokens: 1_000, output_tokens: 200 });
        assert_eq!(today.tokens(), 1_200);
    }

    /// A new local day starts a new count, and with it the limit (the inbox passes the local date).
    #[test]
    fn a_new_day_starts_the_count_again() {
        let mut usage = AssistantUsage::default();
        usage.record(tokens(10, 5), date("2026-10-15"));
        assert!(!usage.allows(1, date("2026-10-15")));
        assert_eq!(usage.today(date("2026-10-16")).requests, 0);
        assert!(usage.allows(1, date("2026-10-16")));
        usage.record(tokens(1, 1), date("2026-10-16"));
        assert_eq!(usage.days.iter().map(|d| d.day.as_str()).collect::<Vec<_>>(), ["2026-10-15", "2026-10-16"]);
    }

    #[test]
    fn the_month_adds_up_its_days_only() {
        let mut usage = AssistantUsage::default();
        usage.record(tokens(100, 0), date("2026-09-30"));
        usage.record(tokens(10, 1), date("2026-10-01"));
        usage.record(tokens(20, 2), date("2026-10-15"));
        let month = usage.this_month(date("2026-10-15"));
        assert_eq!((month.requests, month.tokens()), (2, 33));
    }

    #[test]
    fn keeps_the_last_days_only() {
        let mut usage = AssistantUsage::default();
        for day in 0..40 {
            usage.record(tokens(1, 0), date("2026-01-01") + chrono::Days::new(day));
        }
        assert_eq!(usage.days.len(), KEPT_DAYS);
        assert_eq!(usage.days.last().unwrap().day, "2026-02-09");
    }

    #[test]
    fn no_limit_by_default() {
        let mut usage = AssistantUsage::default();
        for _ in 0..100 {
            usage.record(None, date("2026-10-15"));
        }
        assert!(usage.allows(0, date("2026-10-15")));
        assert!(!usage.allows(100, date("2026-10-15")));
    }

    #[test]
    fn survives_a_save_and_a_load() {
        let mut usage = AssistantUsage::default();
        usage.record(tokens(3, 4), date("2026-10-15"));
        let loaded: AssistantUsage = serde_json::from_str(&serde_json::to_string(&usage).unwrap()).unwrap();
        assert_eq!(loaded, usage);
    }
}
