//! A reminder that repeats: every day, every weekday or every week at the same local time, until an optional last
//! day. Ported from the macOS `Recurrence`, with the same tests.

use chrono::{DateTime, Datelike, Duration, Local, LocalResult, NaiveDate, TimeZone, Utc, Weekday};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecurrenceRule {
    Daily,
    /// Monday to Friday.
    Weekdays,
    Weekly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recurrence {
    pub rule: RecurrenceRule,
    /// The first occurrence: its local time of day, and for `weekly` its weekday, set the others.
    pub anchor: DateTime<Utc>,
    /// The last day with an occurrence, inclusive. None: no end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<DateTime<Utc>>,
}

impl Recurrence {
    pub fn new(rule: RecurrenceRule, anchor: DateTime<Utc>, until: Option<DateTime<Utc>>) -> Self {
        Recurrence { rule, anchor, until }
    }

    /// The first occurrence after `now`, in this computer's time zone. None once past the last day.
    pub fn next_after(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.next_after_in(now, &Local)
    }

    /// The first occurrence strictly after `now` (and not before the anchor), in `tz`. Missed occurrences are
    /// skipped. None when it would fall after the last day.
    pub fn next_after_in<Tz: TimeZone>(&self, now: DateTime<Utc>, tz: &Tz) -> Option<DateTime<Utc>> {
        let anchor = self.anchor.with_timezone(tz);
        let time = anchor.time();
        let start = anchor.date_naive().max(now.with_timezone(tz).date_naive());
        let last = self.until.map(|until| until.with_timezone(tz).date_naive());
        // A week and a day always holds an occurrence of every rule.
        (0..=8)
            .filter_map(|offset| start.checked_add_signed(Duration::days(offset)))
            .filter(|day| self.matches(*day, anchor.weekday()))
            .take_while(|day| last.is_none_or(|last| *day <= last))
            .filter_map(|day| at(tz, day, time))
            .find(|date| *date > now && *date >= self.anchor)
    }

    /// Whether the last day comes before the first occurrence's day.
    pub fn ends_before_start(&self) -> bool {
        self.ends_before_start_in(&Local)
    }

    pub fn ends_before_start_in<Tz: TimeZone>(&self, tz: &Tz) -> bool {
        self.until
            .is_some_and(|until| until.with_timezone(tz).date_naive() < self.anchor.with_timezone(tz).date_naive())
    }

    fn matches(&self, day: NaiveDate, anchor_weekday: Weekday) -> bool {
        match self.rule {
            RecurrenceRule::Daily => true,
            RecurrenceRule::Weekdays => !matches!(day.weekday(), Weekday::Sat | Weekday::Sun),
            RecurrenceRule::Weekly => day.weekday() == anchor_weekday,
        }
    }
}

/// `time` on `day` in `tz`: the earlier of two when clocks go back, an hour later when that time is skipped.
fn at<Tz: TimeZone>(tz: &Tz, day: NaiveDate, time: chrono::NaiveTime) -> Option<DateTime<Utc>> {
    let local = day.and_time(time);
    match tz.from_local_datetime(&local) {
        LocalResult::Single(date) | LocalResult::Ambiguous(date, _) => Some(date.with_timezone(&Utc)),
        LocalResult::None => {
            tz.from_local_datetime(&(local + Duration::hours(1))).earliest().map(|d| d.with_timezone(&Utc))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::Europe::Paris;

    fn paris(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
        Paris.with_ymd_and_hms(year, month, day, hour, minute, 0).unwrap().with_timezone(&Utc)
    }

    /// Thursday 8 October 2026 at 09:00 in Paris.
    fn anchor() -> DateTime<Utc> {
        paris(2026, 10, 8, 9, 0)
    }

    fn next(rule: RecurrenceRule, until: Option<DateTime<Utc>>, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        Recurrence::new(rule, anchor(), until).next_after_in(now, &Paris)
    }

    #[test]
    fn daily_comes_back_the_next_day_at_the_same_time() {
        assert_eq!(next(RecurrenceRule::Daily, None, paris(2026, 10, 8, 9, 5)), Some(paris(2026, 10, 9, 9, 0)));
    }

    #[test]
    fn daily_before_todays_time_is_today() {
        assert_eq!(next(RecurrenceRule::Daily, None, paris(2026, 10, 12, 8, 0)), Some(paris(2026, 10, 12, 9, 0)));
    }

    #[test]
    fn weekdays_skip_the_weekend() {
        // Friday 9 after its time, then Saturday 10.
        assert_eq!(next(RecurrenceRule::Weekdays, None, paris(2026, 10, 9, 10, 0)), Some(paris(2026, 10, 12, 9, 0)));
        assert_eq!(next(RecurrenceRule::Weekdays, None, paris(2026, 10, 10, 8, 0)), Some(paris(2026, 10, 12, 9, 0)));
    }

    #[test]
    fn weekly_keeps_the_weekday() {
        assert_eq!(next(RecurrenceRule::Weekly, None, paris(2026, 10, 8, 10, 0)), Some(paris(2026, 10, 15, 9, 0)));
    }

    #[test]
    fn the_last_day_is_included() {
        let until = Some(paris(2026, 10, 9, 0, 0));
        assert_eq!(next(RecurrenceRule::Daily, until, paris(2026, 10, 8, 10, 0)), Some(paris(2026, 10, 9, 9, 0)));
        assert_eq!(next(RecurrenceRule::Daily, until, paris(2026, 10, 9, 10, 0)), None);
    }

    #[test]
    fn nothing_after_the_last_day() {
        let until = Some(paris(2026, 10, 9, 0, 0));
        assert_eq!(next(RecurrenceRule::Weekly, until, paris(2026, 10, 8, 10, 0)), None);
    }

    #[test]
    fn before_the_anchor_it_is_the_anchor() {
        assert_eq!(next(RecurrenceRule::Weekly, None, paris(2026, 10, 1, 9, 0)), Some(anchor()));
    }

    #[test]
    fn summer_time_keeps_nine_oclock() {
        // Clocks go forward on Sunday 29 March 2026.
        let recurrence = Recurrence::new(RecurrenceRule::Daily, paris(2026, 3, 27, 9, 0), None);
        let next = recurrence.next_after_in(paris(2026, 3, 29, 10, 0), &Paris).unwrap();
        assert_eq!(next, paris(2026, 3, 30, 9, 0));
        assert_eq!(next.with_timezone(&Paris).format("%H:%M").to_string(), "09:00");
    }

    #[test]
    fn an_end_before_the_start_is_refused() {
        let early = Recurrence::new(RecurrenceRule::Daily, anchor(), Some(paris(2026, 10, 7, 0, 0)));
        let same_day = Recurrence::new(RecurrenceRule::Daily, anchor(), Some(paris(2026, 10, 8, 0, 0)));
        assert!(early.ends_before_start_in(&Paris));
        assert!(!same_day.ends_before_start_in(&Paris));
    }

    #[test]
    fn round_trips_through_json_and_old_items_have_none() {
        let recurrence = Recurrence::new(RecurrenceRule::Weekdays, anchor(), Some(paris(2026, 12, 31, 0, 0)));
        let json = serde_json::to_string(&recurrence).unwrap();
        assert!(json.contains("\"rule\":\"weekdays\""));
        assert_eq!(serde_json::from_str::<Recurrence>(&json).unwrap(), recurrence);

        let mut item = crate::fixtures::item("reminder/1", crate::InboxBundle::reminders());
        let old = serde_json::to_value(&item).unwrap();
        assert!(old.get("recurrence").is_none());
        assert_eq!(serde_json::from_value::<crate::InboxItem>(old).unwrap().recurrence, None);
        item.recurrence = Some(recurrence.clone());
        let back: crate::InboxItem = serde_json::from_str(&serde_json::to_string(&item).unwrap()).unwrap();
        assert_eq!(back.recurrence, Some(recurrence));
    }
}
