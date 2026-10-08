//! Sample items for `--demo`, so the app can be explored without connecting anything. The same inbox as
//! the macOS demo (made-up people and repositories), with Linear for tasks.

use crate::Inbox;
use chrono::{DateTime, Duration, Utc};
use remora_core::{Account, Badge, InboxBundle, InboxItem, ItemState, Person, Snooze, SnoozeClock, SnoozeMode, SnoozeReason, Tone};
use std::collections::HashMap;

fn account(id: &str, plugin: &str) -> Account {
    Account { id: id.into(), plugin_id: plugin.into(), name: None, settings: HashMap::new(), identity: Some("you".into()) }
}

struct Demo {
    now: DateTime<Utc>,
    items: HashMap<String, Vec<InboxItem>>,
}

impl Demo {
    #[allow(clippy::too_many_arguments)]
    fn add(&mut self, n: u32, plugin: &str, bundle: InboxBundle, title: &str, context: &str, minutes: i64, author: &str, badges: Vec<Badge>, people: &[&str], preview: Option<&str>) {
        let needs_action = bundle != InboxBundle::authored() || badges.iter().any(|b| b.id == "approved" || b.id == "changes");
        self.items.entry(plugin.into()).or_default().push(InboxItem {
            id: format!("demo/{n}"),
            account_id: plugin.into(),
            plugin_id: plugin.into(),
            bundle,
            title: title.into(),
            context: context.into(),
            preview: preview.map(str::to_string),
            url: Some(format!("https://{plugin}.com")),
            app_url: None,
            author: Some(Person::named(author)),
            participants: people.iter().map(|p| Person::named(p)).collect(),
            badges,
            date: self.now - Duration::minutes(minutes),
            needs_action,
            priority: None,
            due: None,
        });
    }
}

pub fn load(inbox: &mut Inbox, now: DateTime<Utc>) {
    let passing = || Badge::new("checks.passing", "Checks", Tone::Positive);
    let diff = |a: u32, d: u32| Badge::new("diff", &format!("+{a} −{d}"), Tone::Neutral);
    let mut demo = Demo { now, items: HashMap::new() };
    demo.add(1, "github", InboxBundle::reviews(), "Add passkeys to the sign-in page", "acme/web #482", 12, "erin", vec![passing(), diff(214, 37)], &["you", "frank"], None);
    demo.add(2, "gitlab", InboxBundle::reviews(), "Move the image resizer to the new queue", "acme/platform !1290", 95, "dave", vec![Badge::new("checks.failing", "Checks failed", Tone::Negative), diff(88, 120)], &["you"], None);
    demo.add(3, "slack", InboxBundle::mentions(), "Can you double check the rollout plan before Friday?", "#releases", 30, "Bob", vec![], &[], Some("@you the flag is at 20% in prod, metrics look good so far…"));
    demo.add(4, "slack", InboxBundle::direct_messages(), "Lunch tomorrow? 🍜", "Direct message", 140, "Carol", vec![], &[], Some("There’s a new ramen place near the office"));
    demo.add(5, "github", InboxBundle::authored(), "Snooze reminders for review requests", "acme/inbox #77", 8, "you", vec![Badge::new("approved", "Approved", Tone::Accent), passing(), diff(320, 12)], &["erin", "dave"], None);
    demo.add(6, "github", InboxBundle::authored(), "Cache GraphQL responses per account", "acme/inbox #75", 400, "you", vec![Badge::new("changes", "Changes requested", Tone::Negative), passing()], &["frank"], None);
    demo.add(7, "github", InboxBundle::authored(), "WIP: Notion tasks bundle", "acme/inbox #79", 1_500, "you", vec![Badge::new("draft", "Draft", Tone::Neutral)], &[], None);
    demo.add(8, "linear", InboxBundle::tasks(), "Write Q4 hiring scorecard", "ENG-31", 2_000, "Linear", vec![Badge::new("due.today", "Due today", Tone::Warning)], &[], None);
    demo.add(9, "github", InboxBundle::reviews(), "WEB-42 retry failed uploads", "acme/web #490", 300, "erin", vec![passing(), diff(64, 10)], &[], None);
    demo.add(10, "github", InboxBundle::reviews(), "WEB-42 alert on upload failures", "acme/web #491", 280, "frank", vec![], &[], None);
    demo.add(11, "github", InboxBundle::reviews(), "Refactor the search service", "acme/search #77", 900, "dave", vec![diff(820, 410)], &[], None);
    demo.add(12, "linear", InboxBundle::tasks(), "Update the onboarding checklist", "ENG-12", 40_000, "Linear", vec![], &[], None);
    demo.add(13, "github", InboxBundle::authored(), "Show snooze reasons in notifications", "acme/inbox #81", 3_000, "you", vec![passing(), diff(96, 14)], &["erin", "bob"], None);

    let monday = SnoozeClock::presets(now.with_timezone(&chrono::Local)).into_iter().find(|p| p.label == "Next week").map(|p| p.date).unwrap_or(now + Duration::days(3));
    let mut states = HashMap::new();
    for item in demo.items.values().flatten().filter(|i| ["demo/9", "demo/10", "demo/11", "demo/12"].contains(&i.id.as_str())) {
        let reason = if item.id == "demo/11" { SnoozeReason::Motivation } else { SnoozeReason::NoTime };
        let snooze = Snooze { until: monday, mode: SnoozeMode::Hide, note: None, fingerprint: item.fingerprint(), reason: Some(reason), until_news: None };
        states.insert(item.id.clone(), ItemState { snooze: Some(snooze), ..ItemState::default() });
    }
    let accounts = ["github", "gitlab", "slack", "linear"].iter().map(|p| account(p, p)).collect();
    inbox.load_demo(accounts, demo.items, states);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Managed, MemoryVault};
    use std::sync::Arc;

    #[test]
    fn the_demo_fills_every_tab() {
        let mut inbox = Inbox::in_memory(Arc::new(MemoryVault::default()), Managed::default());
        let now = Utc::now();
        load(&mut inbox, now);
        let layout = inbox.layout(now, "");
        assert!(layout.my_turn_items().count() >= 6);
        assert!(layout.waiting_count() >= 2);
        assert_eq!(layout.snoozed.len(), 4);
    }
}
