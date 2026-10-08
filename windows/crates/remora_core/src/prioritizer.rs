use crate::{InboxItem, Priority};
use chrono::{DateTime, Local, Utc};
use std::cmp::Ordering;

/// Orders a bundle by importance rather than recency: overdue or due today first, then priority,
/// then the nearest due date, then the most recent activity.
pub struct Prioritizer {
    pub now: DateTime<Utc>,
}

impl Prioritizer {
    pub fn new(now: DateTime<Utc>) -> Self {
        Prioritizer { now }
    }

    pub fn sort(&self, items: &mut [InboxItem]) {
        items.sort_by(|a, b| self.compare(a, b));
    }

    fn compare(&self, a: &InboxItem, b: &InboxItem) -> Ordering {
        let pressing = self.is_pressing(b).cmp(&self.is_pressing(a));
        let priority = b.priority.unwrap_or(Priority::Normal).cmp(&a.priority.unwrap_or(Priority::Normal));
        let due = match (a.due, b.due) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        };
        pressing.then(priority).then(due).then(b.date.cmp(&a.date))
    }

    /// Overdue or due today, in local time.
    pub fn is_pressing(&self, item: &InboxItem) -> bool {
        let Some(due) = item.due else { return false };
        let today = self.now.with_timezone(&Local).date_naive();
        due.with_timezone(&Local).date_naive() <= today
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;
    use crate::InboxBundle;
    use chrono::Duration;

    fn task(id: &str, priority: Option<Priority>, due: Option<i64>, minutes_ago: i64) -> InboxItem {
        let mut item = item(id, InboxBundle::tasks());
        item.priority = priority;
        item.due = due.map(|days| now() + Duration::days(days));
        item.date = now() - Duration::minutes(minutes_ago);
        item
    }

    #[test]
    fn pressing_then_priority_then_due_then_recent() {
        let mut items = vec![
            task("recent", None, None, 1),
            task("high", Some(Priority::High), None, 60),
            task("overdue", None, Some(-2), 600),
            task("low", Some(Priority::Low), None, 0),
            task("dueSoon", None, Some(2), 300),
            task("urgent", Some(Priority::Urgent), None, 900),
        ];
        Prioritizer::new(now()).sort(&mut items);
        let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["overdue", "urgent", "high", "dueSoon", "recent", "low"]);
    }
}
