use crate::{InboxBundle, InboxItem, ItemState, SnoozeClock, SnoozeReason};
use chrono::{DateTime, Datelike, Days, Duration, Local, NaiveTime, TimeZone, Timelike, Utc, Weekday};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, LazyLock};

/// One snooze, kept locally to learn your habits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnoozeRecord {
    pub item_id: String,
    pub plugin_id: String,
    pub bundle_id: String,
    /// The place without the item number (`context_key`), so snoozes of sibling items add up.
    pub context: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<SnoozeReason>,
    pub at: DateTime<Utc>,
    pub until: DateTime<Utc>,
    /// When the item was finally marked done, if it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done_at: Option<DateTime<Utc>>,
}

impl SnoozeRecord {
    pub fn new(item: &InboxItem, reason: Option<SnoozeReason>, at: DateTime<Utc>, until: DateTime<Utc>) -> Self {
        SnoozeRecord {
            item_id: item.id.clone(),
            plugin_id: item.plugin_id.clone(),
            bundle_id: item.bundle.id.clone(),
            context: context_key(&item.context),
            reason,
            at,
            until,
            done_at: None,
        }
    }
}

/// Something worth telling about the snoozed pile. Remora shows one at a time.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SnoozeInsight {
    /// The same item keeps coming back.
    Loop { item: Box<InboxItem>, times: usize },
    /// You often put off this context when not feeling it.
    Avoidance { context: String, times: usize, items: Vec<InboxItem> },
    /// Too many items return at the same time.
    PileUp { at: DateTime<Utc>, items: Vec<InboxItem> },
    /// Several snoozed items are about the same thing.
    Cluster { items: Vec<InboxItem> },
    /// Snoozed items that nothing happened to for weeks.
    Stale { items: Vec<InboxItem> },
}

impl SnoozeInsight {
    /// Stable across refreshes, so a dismissed insight stays dismissed.
    pub fn id(&self) -> String {
        let sorted_ids = |items: &[InboxItem]| {
            let mut ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
            ids.sort_unstable();
            ids.join(",")
        };
        match self {
            SnoozeInsight::Loop { item, .. } => format!("loop/{}", item.id),
            SnoozeInsight::Avoidance { context, .. } => format!("avoidance/{context}"),
            // `{:?}` prints "1791446400.0", like Swift's `timeIntervalSince1970`, so ids match across platforms.
            SnoozeInsight::PileUp { at, .. } => format!("pileUp/{:?}", at.timestamp() as f64),
            SnoozeInsight::Cluster { items } => format!("cluster/{}", sorted_ids(items)),
            SnoozeInsight::Stale { items } => format!("stale/{}", sorted_ids(items)),
        }
    }

    pub fn items(&self) -> Vec<InboxItem> {
        match self {
            SnoozeInsight::Loop { item, .. } => vec![(**item).clone()],
            SnoozeInsight::Avoidance { items, .. }
            | SnoozeInsight::PileUp { items, .. }
            | SnoozeInsight::Cluster { items }
            | SnoozeInsight::Stale { items } => items.clone(),
        }
    }
}

/// Tells whether two titles are about the same thing.
pub trait SimilarityModel: Send + Sync {
    fn similar(&self, a: &str, b: &str) -> bool;
}

/// Snooze suggestions and pattern detection. Pure rules over your local history.
///
/// Hours and days are read in `tz`: the user's local time zone in the app, a fixed one in tests.
#[derive(Clone)]
pub struct SnoozeAdvisor<Tz: TimeZone = Local> {
    pub tz: Tz,
    pub similarity: Vec<Arc<dyn SimilarityModel>>,
}

impl Default for SnoozeAdvisor<Local> {
    fn default() -> Self {
        Self::new()
    }
}

impl SnoozeAdvisor<Local> {
    pub fn new() -> Self {
        Self::in_time_zone(Local)
    }
}

impl<Tz: TimeZone> SnoozeAdvisor<Tz> {
    /// Gap between items when spreading a pile-up.
    pub const SPREAD_STEP: Duration = Duration::minutes(30);

    pub fn in_time_zone(tz: Tz) -> Self {
        SnoozeAdvisor { tz, similarity: vec![Arc::new(KeywordSimilarity)] }
    }

    // When to come back

    /// Days off are skipped: a suggestion for a later day that falls on a weekend comes back the next working
    /// day, at the same time. Today stays today, weekend or not: you're working.
    pub fn suggested_return(&self, reason: SnoozeReason, history: &[SnoozeRecord], now: DateTime<Utc>) -> DateTime<Utc> {
        self.workday(self.raw_return(reason, history, now), now)
    }

    fn raw_return(&self, reason: SnoozeReason, history: &[SnoozeRecord], now: DateTime<Utc>) -> DateTime<Utc> {
        match reason {
            SnoozeReason::Waiting => self.morning(3, now, 9),
            SnoozeReason::NoTime => {
                let later = now + Duration::hours(3);
                if self.same_day(later, now) && self.local(later).hour() < 19 {
                    rounded_up(later)
                } else {
                    SnoozeClock::tomorrow_morning(self.local(now))
                }
            }
            SnoozeReason::Focus | SnoozeReason::Motivation => self.next(self.best_hour(history), now),
            SnoozeReason::NotUrgent => {
                // The next Monday strictly after today, like Calendar.nextDate(matching: weekday 2).
                let days = 7 - self.local(now).weekday().num_days_from_monday() as u64;
                self.morning(days, now, 9)
            }
        }
    }

    /// The hour you most often finish things, learned from done snoozes (9:00 until there's enough data).
    /// The reason you usually give in this repo or channel: at least three snoozes there, 60 % of them for it.
    pub fn usual_reason(&self, item: &InboxItem, history: &[SnoozeRecord]) -> Option<SnoozeReason> {
        let place = context_key(&item.context);
        let reasons: Vec<SnoozeReason> = history.iter().filter(|r| r.context == place).filter_map(|r| r.reason).collect();
        if reasons.len() < 3 {
            return None;
        }
        let mut counts: Vec<(SnoozeReason, usize)> = vec![];
        for reason in &reasons {
            match counts.iter_mut().find(|(r, _)| r == reason) {
                Some((_, n)) => *n += 1,
                None => counts.push((*reason, 1)),
            }
        }
        let (reason, count) = counts.into_iter().max_by_key(|(_, n)| *n)?;
        (count as f64 / reasons.len() as f64 >= 0.6).then_some(reason)
    }

    pub fn best_hour(&self, history: &[SnoozeRecord]) -> u32 {
        let hours: Vec<u32> = history
            .iter()
            .filter_map(|r| r.done_at)
            .map(|d| self.local(d).hour())
            .filter(|h| (7..=20).contains(h))
            .collect();
        if hours.len() < 5 {
            return 9;
        }
        let mut counts: BTreeMap<u32, usize> = BTreeMap::new();
        for hour in hours {
            *counts.entry(hour).or_default() += 1;
        }
        // Most frequent; on a tie, the earliest hour.
        counts.into_iter().max_by_key(|(hour, count)| (*count, std::cmp::Reverse(*hour))).map_or(9, |(hour, _)| hour)
    }

    /// What you usually pick for this repo or channel (or this kind of item), from at least three past snoozes.
    pub fn usual_return(&self, item: &InboxItem, history: &[SnoozeRecord], now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let key = context_key(&item.context);
        let same_context: Vec<&SnoozeRecord> = history.iter().filter(|r| r.context == key).collect();
        let similar = if same_context.len() >= 3 {
            same_context
        } else {
            history.iter().filter(|r| r.bundle_id == item.bundle.id).collect()
        };
        if similar.len() < 3 {
            return None;
        }
        let mut durations: Vec<Duration> = similar.iter().map(|r| r.until - r.at).collect();
        durations.sort();
        let median = durations[durations.len() / 2];
        let date = now + median;
        Some(if median >= Duration::hours(12) { self.workday(self.morning(0, date, 9), now) } else { rounded_up(date) })
    }

    /// A small, guilt-free way in when you're not feeling it. English text, translated by the UI.
    pub fn nudge(&self, item: &InboxItem) -> &'static str {
        if let Some(size) = item.diff_size() {
            return if size > 300 { "Do just the first step" } else { "Start with 10 minutes" };
        }
        if item.bundle == InboxBundle::reply() || item.bundle == InboxBundle::tasks() {
            return "Start with 10 minutes";
        }
        "Pair it with something easy"
    }

    /// Reschedules items 30 minutes apart, starting at `start`.
    pub fn spread(&self, items: &[InboxItem], start: DateTime<Utc>) -> HashMap<String, DateTime<Utc>> {
        self.spread_every(items, start, Self::SPREAD_STEP)
    }

    /// Reschedules items `step` apart, starting at `start`.
    pub fn spread_every(&self, items: &[InboxItem], start: DateTime<Utc>, step: Duration) -> HashMap<String, DateTime<Utc>> {
        items.iter().enumerate().map(|(i, item)| (item.id.clone(), start + step * i as i32)).collect()
    }

    // Patterns

    pub fn insights(
        &self,
        snoozed: &[InboxItem],
        states: &HashMap<String, ItemState>,
        history: &[SnoozeRecord],
        now: DateTime<Utc>,
    ) -> Vec<SnoozeInsight> {
        let recent: Vec<&SnoozeRecord> = history.iter().filter(|r| r.at > now - Duration::days(30)).collect();
        let mut insights = vec![];

        let mut loops: Vec<(&InboxItem, usize)> = snoozed
            .iter()
            .map(|item| (item, recent.iter().filter(|r| r.item_id == item.id).count()))
            .filter(|(_, times)| *times >= 3)
            .collect();
        loops.sort_by_key(|loop_| std::cmp::Reverse(loop_.1));
        insights.extend(loops.into_iter().map(|(item, times)| SnoozeInsight::Loop { item: Box::new(item.clone()), times }));

        // BTreeMap, so contexts with as many snoozes come in a stable order (Swift's dictionary order is arbitrary).
        let mut avoided: BTreeMap<&str, usize> = BTreeMap::new();
        for record in recent.iter().filter(|r| r.reason == Some(SnoozeReason::Motivation)) {
            *avoided.entry(record.context.as_str()).or_default() += 1;
        }
        let mut avoided: Vec<(&str, usize)> = avoided.into_iter().filter(|(_, times)| *times >= 3).collect();
        avoided.sort_by_key(|context| std::cmp::Reverse(context.1));
        for (context, times) in avoided {
            let items: Vec<InboxItem> = snoozed.iter().filter(|i| context_key(&i.context) == context).cloned().collect();
            if !items.is_empty() {
                insights.push(SnoozeInsight::Avoidance { context: context.to_string(), times, items });
            }
        }

        let mut by_hour: BTreeMap<DateTime<Utc>, Vec<InboxItem>> = BTreeMap::new();
        for item in snoozed {
            if let Some(snooze) = states.get(&item.id).and_then(|s| s.snooze.as_ref()) {
                by_hour.entry(self.start_of_hour(snooze.until)).or_default().push(item.clone());
            }
        }
        // The earliest hour with a pile.
        if let Some((at, mut items)) = by_hour.into_iter().find(|(_, items)| items.len() >= 4) {
            items.sort_by_key(|item| std::cmp::Reverse(item.date));
            insights.push(SnoozeInsight::PileUp { at, items });
        }

        if let Some(cluster) = self.largest_cluster(snoozed).filter(|c| c.len() >= 2) {
            insights.push(SnoozeInsight::Cluster { items: cluster });
        }

        let stale: Vec<InboxItem> = snoozed.iter().filter(|i| i.date < now - Duration::days(21)).cloned().collect();
        if !stale.is_empty() {
            insights.push(SnoozeInsight::Stale { items: stale });
        }

        insights
    }

    fn largest_cluster(&self, items: &[InboxItem]) -> Option<Vec<InboxItem>> {
        let groups = items
            .iter()
            .map(|seed| items.iter().filter(|i| i.id == seed.id || self.is_similar(i, seed)).cloned().collect::<Vec<_>>());
        // The first of the largest, as Swift's `max(by:)` keeps it.
        groups.fold(None, |best: Option<Vec<InboxItem>>, group| match best {
            Some(best) if best.len() >= group.len() => Some(best),
            _ => Some(group),
        })
    }

    fn is_similar(&self, a: &InboxItem, b: &InboxItem) -> bool {
        self.similarity.iter().any(|model| model.similar(&a.title, &b.title))
    }

    // Helpers

    /// The next working day at the same time, for a date on a later day that falls on a weekend.
    /// chrono knows no regional weekend, so it is Saturday and Sunday (as the French calendar has it on macOS).
    pub fn workday(&self, date: DateTime<Utc>, now: DateTime<Utc>) -> DateTime<Utc> {
        if self.same_day(date, now) {
            return date;
        }
        let mut date = date;
        for _ in 0..7 {
            if !matches!(self.local(date).weekday(), Weekday::Sat | Weekday::Sun) {
                break;
            }
            date = self.add_days(date, 1);
        }
        date
    }

    fn local(&self, date: DateTime<Utc>) -> DateTime<Tz> {
        date.with_timezone(&self.tz)
    }

    fn same_day(&self, a: DateTime<Utc>, b: DateTime<Utc>) -> bool {
        self.local(a).date_naive() == self.local(b).date_naive()
    }

    /// Same wall-clock time `days` later, across a DST change too.
    fn add_days(&self, date: DateTime<Utc>, days: u64) -> DateTime<Utc> {
        let naive = self.local(date).naive_local() + Days::new(days);
        self.resolve(naive).unwrap_or(date + Duration::days(days as i64))
    }

    fn morning(&self, days_after: u64, date: DateTime<Utc>, hour: u32) -> DateTime<Utc> {
        let day = self.add_days(date, days_after);
        let naive = self.local(day).date_naive().and_time(NaiveTime::from_hms_opt(hour, 0, 0).unwrap());
        self.resolve(naive).unwrap_or(day)
    }

    fn next(&self, hour: u32, now: DateTime<Utc>) -> DateTime<Utc> {
        let today = self.morning(0, now, hour);
        if today - now > Duration::hours(1) { today } else { self.morning(1, now, hour) }
    }

    fn start_of_hour(&self, date: DateTime<Utc>) -> DateTime<Utc> {
        let local = self.local(date).naive_local();
        let naive = local.date().and_time(NaiveTime::from_hms_opt(local.hour(), 0, 0).unwrap());
        self.resolve(naive).unwrap_or(date)
    }

    fn resolve(&self, naive: chrono::NaiveDateTime) -> Option<DateTime<Utc>> {
        self.tz.from_local_datetime(&naive).earliest().map(|d| d.with_timezone(&Utc))
    }
}

/// Up to the next quarter hour, on absolute time like the macOS `SnoozeClock.roundedUp`.
fn rounded_up(date: DateTime<Utc>) -> DateTime<Utc> {
    let quarter = 15 * 60;
    let past = date.timestamp().rem_euclid(quarter);
    if past == 0 && date.timestamp_subsec_nanos() == 0 {
        return date;
    }
    let base = date.timestamp() - past;
    Utc.timestamp_opt(base + quarter, 0).single().unwrap_or(date)
}

/// Same ticket key ("ENG-123"), or most significant words in common.
#[derive(Clone, Copy, Debug, Default)]
pub struct KeywordSimilarity;

static STOP_WORDS: [&str; 33] = [
    "the", "and", "for", "with", "from", "into", "this", "that", "your", "about", "when", "what", "after", "new",
    "pour", "avec", "dans", "les", "des", "une", "sur", "est", "pas", "qui", "que", "vous", "nous", "vers",
    "la", "le", "de", "du", "au",
];

static TICKET_KEY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Z][A-Z0-9]+-\d+").unwrap());

impl KeywordSimilarity {
    fn ticket_keys(text: &str) -> HashSet<&str> {
        TICKET_KEY.find_iter(text).map(|m| m.as_str()).collect()
    }

    fn words(text: &str) -> HashSet<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphabetic() && !c.is_numeric())
            .filter(|w| w.chars().count() >= 4 && !STOP_WORDS.contains(w))
            .map(str::to_string)
            .collect()
    }
}

impl SimilarityModel for KeywordSimilarity {
    fn similar(&self, a: &str, b: &str) -> bool {
        if !Self::ticket_keys(a).is_disjoint(&Self::ticket_keys(b)) {
            return true;
        }
        let (words_a, words_b) = (Self::words(a), Self::words(b));
        let shared = words_a.intersection(&words_b).count();
        let union = words_a.union(&words_b).count();
        shared >= 2 && shared as f64 / union.max(1) as f64 >= 0.4
    }
}

/// The place without the item number: "acme/app #12" → "acme/app", "g/p !3" → "g/p", "#releases" stays.
pub fn context_key(context: &str) -> String {
    let mut parts: Vec<&str> = context.split(' ').filter(|p| !p.is_empty()).collect();
    if let Some(last) = parts.last() {
        let mut chars = last.chars();
        let numbered = last.chars().count() > 1
            && chars.next().is_some_and(|c| c == '#' || c == '!')
            && chars.all(char::is_numeric);
        if numbered {
            parts.pop();
        }
    }
    if parts.is_empty() { context.to_string() } else { parts.join(" ") }
}

impl InboxItem {
    /// Lines changed, read from the "+a −d" badge.
    pub(crate) fn diff_size(&self) -> Option<u64> {
        let label = &self.badges.iter().find(|b| b.id == "diff")?.label;
        let numbers: Vec<u64> = label.split(|c: char| !c.is_numeric()).filter_map(|n| n.parse().ok()).collect();
        if numbers.is_empty() { None } else { Some(numbers.iter().sum()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Badge, InboxAssembler, Placement, Snooze, SnoozeMode, Tone};
    use chrono::FixedOffset;

    /// Europe/Paris in October 2026 (summer time until the 25th).
    fn paris() -> FixedOffset {
        FixedOffset::east_opt(2 * 3600).unwrap()
    }

    fn advisor() -> SnoozeAdvisor<FixedOffset> {
        SnoozeAdvisor::in_time_zone(paris())
    }

    fn at(day: u32, hour: u32) -> DateTime<Utc> {
        paris().with_ymd_and_hms(2026, 10, day, hour, 0, 0).unwrap().with_timezone(&Utc)
    }

    /// Thursday 8 October 2026, 10:00.
    fn morning() -> DateTime<Utc> {
        at(8, 10)
    }

    fn hour(date: DateTime<Utc>) -> u32 {
        date.with_timezone(&paris()).hour()
    }

    fn weekday(date: DateTime<Utc>) -> Weekday {
        date.with_timezone(&paris()).weekday()
    }

    fn make_item(id: &str) -> InboxItem {
        crate::fixtures::item(id, InboxBundle::reviews())
    }

    fn dated(id: &str, date: DateTime<Utc>) -> InboxItem {
        InboxItem { date, ..make_item(id) }
    }

    fn record(item: &InboxItem, reason: Option<SnoozeReason>, at: DateTime<Utc>, hours: i64, done_hour: Option<u32>) -> SnoozeRecord {
        let mut record = SnoozeRecord::new(item, reason, at, at + Duration::hours(hours));
        record.done_at = done_hour.map(|h| {
            let day = at.with_timezone(&paris()).date_naive();
            paris().from_local_datetime(&day.and_hms_opt(h, 10, 0).unwrap()).unwrap().with_timezone(&Utc)
        });
        record
    }

    fn snoozed(items: &[InboxItem], until: DateTime<Utc>) -> HashMap<String, ItemState> {
        items
            .iter()
            .map(|item| {
                let snooze = Snooze {
                    until,
                    mode: SnoozeMode::Hide,
                    note: None,
                    fingerprint: item.fingerprint(),
                    reason: None,
                    until_news: None,
                };
                (item.id.clone(), ItemState { snooze: Some(snooze), ..Default::default() })
            })
            .collect()
    }

    #[test]
    fn reasons_suggest_different_returns() {
        let advisor = advisor();
        let waiting = advisor.suggested_return(SnoozeReason::Waiting, &[], morning());
        assert!(weekday(waiting) == Weekday::Mon && hour(waiting) == 9);
        assert_eq!(hour(advisor.suggested_return(SnoozeReason::NoTime, &[], morning())), 13);
        assert_eq!(weekday(advisor.suggested_return(SnoozeReason::NotUrgent, &[], morning())), Weekday::Mon);
        let focus = advisor.suggested_return(SnoozeReason::Focus, &[], morning());
        assert!(hour(focus) == 9 && focus > morning());
    }

    #[test]
    fn returns_skip_the_weekend() {
        let advisor = advisor();
        let friday = at(9, 18);
        assert_eq!(weekday(advisor.suggested_return(SnoozeReason::NoTime, &[], friday)), Weekday::Mon, "Friday evening: Monday, not Saturday");
        assert_eq!(weekday(advisor.suggested_return(SnoozeReason::Focus, &[], friday)), Weekday::Mon);
        let saturday = at(10, 10);
        let later = advisor.suggested_return(SnoozeReason::NoTime, &[], saturday);
        assert_eq!(
            later.with_timezone(&paris()).date_naive(),
            saturday.with_timezone(&paris()).date_naive(),
            "working on a Saturday: later the same day stays"
        );
    }

    #[test]
    fn motivation_aims_for_your_best_hour() {
        let advisor = advisor();
        let item = make_item("x");
        let history: Vec<SnoozeRecord> =
            (0..6).map(|i| record(&item, None, morning() - Duration::days(i), 1, Some(15))).collect();
        assert_eq!(advisor.best_hour(&history), 15);
        assert_eq!(hour(advisor.suggested_return(SnoozeReason::Motivation, &history, morning())), 15);
        assert_eq!(advisor.best_hour(&history[..2]), 9, "not enough data: default 9:00");
    }

    #[test]
    fn nudge_depends_on_size() {
        let advisor = advisor();
        let mut big = make_item("1");
        big.badges = vec![Badge::new("diff", "+420 −80", Tone::Neutral)];
        let mut small = make_item("2");
        small.badges = vec![Badge::new("diff", "+12 −3", Tone::Neutral)];
        assert_eq!(advisor.nudge(&big), "Do just the first step");
        assert_eq!(advisor.nudge(&small), "Start with 10 minutes");
    }

    #[test]
    fn usual_return_needs_three_snoozes_in_the_same_context() {
        let advisor = advisor();
        let item = make_item("1");
        let two: Vec<SnoozeRecord> = (0..2).map(|_| record(&item, None, morning(), 2, None)).collect();
        assert_eq!(advisor.usual_return(&item, &two, morning()), None);
        let mut three = two.clone();
        three.push(record(&item, None, morning(), 2, None));
        assert_eq!(advisor.usual_return(&item, &three, morning()), Some(morning() + Duration::hours(2)));
        let days: Vec<SnoozeRecord> = (0..3).map(|_| record(&item, None, morning(), 48, None)).collect();
        let usual = advisor.usual_return(&item, &days, morning()).unwrap();
        assert_eq!(hour(usual), 9, "long snoozes align to the morning");
    }

    #[test]
    fn context_key_drops_the_item_number() {
        assert_eq!(context_key("acme/app #12"), "acme/app");
        assert_eq!(context_key("g/p !3"), "g/p");
        assert_eq!(context_key("#releases"), "#releases");
    }

    #[test]
    fn loops_come_first() {
        let advisor = advisor();
        let item = dated("1", morning());
        let history: Vec<SnoozeRecord> =
            (0..3).map(|i| record(&item, None, morning() - Duration::days(i), 24, None)).collect();
        let states = snoozed(std::slice::from_ref(&item), morning() + Duration::hours(1));
        let insights = advisor.insights(std::slice::from_ref(&item), &states, &history, morning());
        assert_eq!(insights.first(), Some(&SnoozeInsight::Loop { item: Box::new(item), times: 3 }));
    }

    #[test]
    fn avoidance_needs_three_not_feeling_it_snoozes() {
        let advisor = advisor();
        let item = dated("1", morning());
        let history: Vec<SnoozeRecord> = (0..3)
            .map(|i| record(&make_item(&format!("9{i}")), Some(SnoozeReason::Motivation), morning(), 24, None))
            .collect();
        let states = snoozed(std::slice::from_ref(&item), morning() + Duration::hours(1));
        let insights = advisor.insights(std::slice::from_ref(&item), &states, &history, morning());
        assert!(insights.iter().any(|i| matches!(i, SnoozeInsight::Avoidance { times: 3, items, .. } if *items == [item.clone()])));
    }

    #[test]
    fn pile_ups_are_detected_and_spread() {
        let advisor = advisor();
        let items: Vec<InboxItem> = (1..=4).map(|i| dated(&i.to_string(), morning())).collect();
        let monday = morning() + Duration::days(4);
        let insights = advisor.insights(&items, &snoozed(&items, monday), &[], morning());
        let Some(SnoozeInsight::PileUp { at, items: pile }) = insights.first() else { panic!("no pile-up") };
        assert_eq!(pile.len(), 4);
        let spread = advisor.spread(pile, *at);
        assert_eq!(spread.values().collect::<HashSet<_>>().len(), 4);
        assert_eq!(*spread.values().max().unwrap(), *at + Duration::minutes(90));
    }

    #[test]
    fn similar_items_cluster_and_stale_ones_show() {
        let advisor = advisor();
        let mut a = dated("1", morning());
        a.title = "WEB-42 retry uploads".into();
        let mut b = dated("2", morning());
        b.title = "Follow-up on WEB-42".into();
        let mut old = dated("3", morning() - Duration::days(30));
        old.title = "Update the README badges".into();
        let all = [a.clone(), b.clone(), old.clone()];
        let insights = advisor.insights(&all, &snoozed(&all, morning() + Duration::days(1)), &[], morning());
        assert!(insights.contains(&SnoozeInsight::Cluster { items: vec![a, b] }));
        assert!(insights.contains(&SnoozeInsight::Stale { items: vec![old] }));
    }

    #[test]
    fn keyword_similarity() {
        let similarity = KeywordSimilarity;
        assert!(similarity.similar("Retry image uploads", "Image uploads timeout"));
        assert!(!similarity.similar("Retry image uploads", "Lunch tomorrow"));
    }

    // `embeddingSimilaritySeparatesTopics` is not ported: it tests Apple's NLEmbedding model, which Windows lacks.

    #[test]
    fn until_news_wakes_even_when_globally_off() {
        let item = make_item("1");
        let state = ItemState {
            snooze: Some(Snooze {
                until: crate::fixtures::now() + Duration::hours(1),
                mode: SnoozeMode::Hide,
                note: None,
                fingerprint: "stale".into(),
                reason: None,
                until_news: Some(true),
            }),
            ..Default::default()
        };
        let assembler = InboxAssembler { wake_on_activity: false };
        assert_eq!(assembler.placement(&item, Some(&state), crate::fixtures::now()), Placement::Inbox);
    }

    #[test]
    fn states_saved_before_reasons_still_decode() {
        // Dates are RFC 3339 strings on Windows, where macOS writes seconds.
        let json = r#"{"pinned": false, "snooze": {"until": "1970-01-01T00:00:00Z", "mode": "hide", "fingerprint": "f"}}"#;
        let state: ItemState = serde_json::from_str(json).unwrap();
        assert_eq!(state.snooze.unwrap().reason, None);
    }

    #[test]
    fn records_and_insights_round_trip() {
        let item = make_item("1");
        let record = SnoozeRecord::new(&item, Some(SnoozeReason::NoTime), morning(), morning() + Duration::hours(2));
        assert_eq!(record.context, "repo");
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"itemId\":\"1\"") && json.contains("\"reason\":\"noTime\"") && !json.contains("doneAt"));
        assert_eq!(serde_json::from_str::<SnoozeRecord>(&json).unwrap(), record);
        let pile = SnoozeInsight::PileUp { at: morning(), items: vec![] };
        assert_eq!(pile.id(), format!("pileUp/{}.0", morning().timestamp()));
        assert_eq!(SnoozeInsight::Stale { items: vec![make_item("b"), make_item("a")] }.id(), "stale/a,b");
    }
}
