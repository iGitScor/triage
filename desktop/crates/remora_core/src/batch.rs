//! Notifications grouped at set times: what arrives and what changes waits for the next time the user
//! chose, then comes as it is, or as one notification when there is more. Reminders and snoozes coming back keep
//! their own time. Same rule on macOS (`NotificationBatch.swift`).

use crate::{Notice, NoticeKind, WHILE_CLOSED_LIMIT, WHILE_CLOSED_SUFFIX};
use chrono::{DateTime, Duration, Local, NaiveTime, TimeZone, Utc};

/// The times offered at first: mid-morning, after lunch, before leaving.
pub const DEFAULT_NOTIFICATION_TIMES: &str = "10:00, 14:00, 17:00";
/// The grouped notification's text, translated by the UI; `%d` is its `count`.
const BATCH_TITLE: &str = "%d new in Remora";
const BATCH_BODY: &str = "Open Remora to see them.";

/// "10:00, 14:00 17:00" → the times, in order and once each. Anything that isn't a time is left out.
pub fn parse_times(text: &str) -> Vec<NaiveTime> {
    let mut times: Vec<NaiveTime> =
        text.split([',', ';', ' ']).filter_map(|part| NaiveTime::parse_from_str(part.trim(), "%H:%M").ok()).collect();
    times.sort();
    times.dedup();
    times
}

/// Whether one of `times` (on this computer's clock) came after `since` and by `now`. After two days away, one has.
pub fn is_due(times: &[NaiveTime], since: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    if times.is_empty() || now <= since {
        return false;
    }
    if now - since > Duration::days(2) {
        return true;
    }
    let mut day = since.with_timezone(&Local).date_naive();
    let last = now.with_timezone(&Local).date_naive();
    while day <= last {
        for time in times {
            if let Some(at) = Local.from_local_datetime(&day.and_time(*time)).earliest() {
                let at = at.with_timezone(&Utc);
                if since < at && at <= now {
                    return true;
                }
            }
        }
        let Some(next) = day.succ_opt() else { break };
        day = next;
    }
    false
}

/// Whether a notice waits for the next set time: what arrived or changed, not a reminder.
pub fn waits(notice: &Notice) -> bool {
    notice.kind != NoticeKind::Reminder
}

/// What waited, as notifications: up to `WHILE_CLOSED_LIMIT` as they are, more as one that says how many and opens
/// Remora.
pub fn grouped(waiting: Vec<Notice>) -> Vec<Notice> {
    if waiting.len() <= WHILE_CLOSED_LIMIT {
        return waiting;
    }
    vec![Notice {
        kind: NoticeKind::Arrival,
        item_id: format!("batch{WHILE_CLOSED_SUFFIX}"),
        title: BATCH_TITLE.into(),
        subtitle: String::new(),
        body: BATCH_BODY.into(),
        url: None,
        count: Some(waiting.len()),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
        Local.with_ymd_and_hms(2026, 10, day, hour, minute, 0).unwrap().with_timezone(&Utc)
    }

    fn time(text: &str) -> NaiveTime {
        NaiveTime::parse_from_str(text, "%H:%M").unwrap()
    }

    #[test]
    fn reads_the_times_however_they_are_typed() {
        assert_eq!(parse_times("17:00, 10:00;14:00 10:00 noon"), [time("10:00"), time("14:00"), time("17:00")]);
        assert!(parse_times("").is_empty());
        assert_eq!(parse_times(DEFAULT_NOTIFICATION_TIMES).len(), 3);
    }

    #[test]
    fn a_set_time_is_due_once_it_has_passed() {
        let times = parse_times("10:00, 17:00");
        assert!(!is_due(&times, at(13, 9, 0), at(13, 9, 59)), "not yet");
        assert!(is_due(&times, at(13, 9, 0), at(13, 10, 0)), "10:00 passed");
        assert!(!is_due(&times, at(13, 10, 0), at(13, 16, 0)), "flushed at 10:00, 17:00 still to come");
        assert!(is_due(&times, at(13, 18, 0), at(14, 10, 5)), "overnight, the next morning's");
        assert!(is_due(&times, at(10, 18, 0), at(13, 9, 0)), "days away");
        assert!(!is_due(&[], at(13, 9, 0), at(13, 18, 0)), "no times: never");
    }

    #[test]
    fn several_waiting_come_as_one() {
        let notice = |id: &str, kind| Notice {
            kind,
            item_id: id.into(),
            title: "To review".into(),
            subtitle: String::new(),
            body: id.into(),
            url: None,
            count: None,
        };
        let few = vec![notice("a", NoticeKind::Arrival), notice("b", NoticeKind::StatusChange)];
        assert_eq!(grouped(few.clone()), few);
        let many: Vec<Notice> = ["a", "b", "c", "d"].iter().map(|id| notice(id, NoticeKind::Arrival)).collect();
        let one = grouped(many);
        assert_eq!((one.len(), one[0].count, one[0].item_id.as_str()), (1, Some(4), "batch/while-closed"));
        assert!(waits(&notice("a", NoticeKind::StatusChange)) && !waits(&notice("r", NoticeKind::Reminder)));
    }
}
