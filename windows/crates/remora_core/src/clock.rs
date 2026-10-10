use chrono::{DateTime, Datelike, Duration, Local, NaiveTime, TimeZone, Timelike, Utc, Weekday};
use serde::Serialize;

/// A quick choice in the time picker. `label` is English, translated by the UI.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Preset {
    pub label: &'static str,
    pub date: DateTime<Utc>,
}

/// Snooze and reminder times: a non-linear scrubber (minutes, then hours, then days) and presets.
pub struct SnoozeClock;

impl SnoozeClock {
    pub const STEPS_MINUTES: [i64; 19] =
        [5, 10, 15, 20, 30, 45, 60, 90, 120, 180, 240, 360, 480, 720, 1_440, 2_880, 4_320, 7_200, 10_080];

    /// Drag position (0…1) → seconds. Small moves give minutes, long ones give days.
    pub fn duration_at(progress: f64) -> i64 {
        let clamped = progress.clamp(0.0, 1.0);
        let index = (clamped * (Self::STEPS_MINUTES.len() - 1) as f64).round() as usize;
        Self::STEPS_MINUTES[index] * 60
    }

    /// Seconds → drag position, interpolated between steps, so the knob follows a date picked another way.
    pub fn progress_for(seconds: i64) -> f64 {
        let steps: Vec<f64> = Self::STEPS_MINUTES.iter().map(|m| (*m * 60) as f64).collect();
        let seconds = seconds as f64;
        if seconds <= steps[0] {
            return 0.0;
        }
        if seconds >= steps[steps.len() - 1] {
            return 1.0;
        }
        let upper = steps.iter().position(|s| *s >= seconds).unwrap();
        let lower = upper - 1;
        let fraction = (seconds - steps[lower]) / (steps[upper] - steps[lower]);
        (lower as f64 + fraction) / (steps.len() - 1) as f64
    }

    pub fn presets<Tz: TimeZone>(now: DateTime<Tz>) -> Vec<Preset> {
        let mut presets = vec![];
        let later = round_up_quarter(now.clone() + Duration::hours(3));
        if later.date_naive() == now.date_naive() {
            presets.push(Preset { label: "Later today", date: later.with_timezone(&Utc) });
        }
        let evening = at(&now, 0, 18);
        if evening.clone().signed_duration_since(now.clone()) > Duration::hours(1) {
            presets.push(Preset { label: "This evening", date: evening.with_timezone(&Utc) });
        }
        presets.push(Preset { label: "Tomorrow", date: Self::tomorrow_morning(now.clone()) });
        let days_to_monday = (7 - now.weekday().num_days_from_monday() as i64) % 7;
        let monday = at(&now, if days_to_monday == 0 { 7 } else { days_to_monday }, 9);
        debug_assert_eq!(monday.weekday(), Weekday::Mon);
        presets.push(Preset { label: "Next week", date: monday.with_timezone(&Utc) });
        presets
    }

    pub fn tomorrow_morning<Tz: TimeZone>(now: DateTime<Tz>) -> DateTime<Utc> {
        at(&now, 1, 9).with_timezone(&Utc)
    }

    pub fn local_presets() -> Vec<Preset> {
        Self::presets(Local::now())
    }
}

fn at<Tz: TimeZone>(day: &DateTime<Tz>, days_after: i64, hour: u32) -> DateTime<Tz> {
    let date = (day.clone() + Duration::days(days_after)).date_naive();
    let naive = date.and_time(NaiveTime::from_hms_opt(hour, 0, 0).unwrap());
    day.timezone().from_local_datetime(&naive).earliest().unwrap_or_else(|| day.clone())
}

fn round_up_quarter<Tz: TimeZone>(date: DateTime<Tz>) -> DateTime<Tz> {
    let past = (date.minute() % 15) as i64 * 60 + date.second() as i64;
    if past == 0 {
        date
    } else {
        date + Duration::seconds(15 * 60 - past)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    fn paris(hour: u32) -> DateTime<FixedOffset> {
        FixedOffset::east_opt(2 * 3600).unwrap().with_ymd_and_hms(2026, 10, 8, hour, 0, 0).unwrap()
    }

    #[test]
    fn scrubber_grows_from_minutes_to_days() {
        assert_eq!(SnoozeClock::duration_at(0.0), 5 * 60);
        assert_eq!(SnoozeClock::duration_at(1.0), 7 * 86_400);
        let samples: Vec<i64> = (0..=20).map(|i| SnoozeClock::duration_at(i as f64 / 20.0)).collect();
        assert!(samples.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn progress_follows_any_date() {
        assert_eq!(SnoozeClock::progress_for(60), 0.0);
        assert_eq!(SnoozeClock::progress_for(30 * 86_400), 1.0);
        for minutes in SnoozeClock::STEPS_MINUTES {
            let seconds = minutes * 60;
            assert_eq!(SnoozeClock::duration_at(SnoozeClock::progress_for(seconds)), seconds);
        }
    }

    #[test]
    fn presets_are_in_the_future_and_skip_same_day_when_late() {
        let labels = |now: DateTime<FixedOffset>| SnoozeClock::presets(now).iter().map(|p| p.label).collect::<Vec<_>>();
        assert_eq!(labels(paris(10)), ["Later today", "This evening", "Tomorrow", "Next week"]);
        assert!(SnoozeClock::presets(paris(10)).iter().all(|p| p.date > paris(10).with_timezone(&Utc)));
        assert_eq!(labels(paris(23)), ["Tomorrow", "Next week"]);
    }
}
