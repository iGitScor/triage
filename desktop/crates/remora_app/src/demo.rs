//! Sample items for `--demo`, so the app can be explored without connecting anything. The same inbox as
//! the macOS demo (made-up people and repositories), with Linear for tasks.
use remora_core::badge;

use crate::{Inbox, Managed, MemoryVault};
use chrono::{DateTime, Duration, Utc};
use remora_core::{
    Account, Badge, InboxBundle, InboxItem, ItemState, Person, Reaction, Snooze, SnoozeClock, SnoozeMode, SnoozeReason,
    Tone,
};
use std::collections::HashMap;
use std::sync::Arc;

fn account(id: &str, plugin: &str) -> Account {
    Account {
        id: id.into(),
        plugin_id: plugin.into(),
        name: None,
        settings: HashMap::new(),
        identity: Some("you".into()),
    }
}

struct Demo {
    now: DateTime<Utc>,
    items: HashMap<String, Vec<InboxItem>>,
}

impl Demo {
    #[allow(clippy::too_many_arguments)]
    fn add(
        &mut self,
        n: u32,
        plugin: &str,
        bundle: InboxBundle,
        title: &str,
        context: &str,
        minutes: i64,
        author: &str,
        badges: Vec<Badge>,
        people: &[&str],
        preview: Option<&str>,
    ) {
        let needs_action = bundle != InboxBundle::authored()
            || badges.iter().any(|b| b.id == badge::APPROVED || b.id == badge::CHANGES);
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
            expires: None,
            changes: None,
            suggested_people: None,
            recurrence: None,
            reactions: None,
            reason: None,
        });
    }

    fn edit(&mut self, n: u32, change: impl FnOnce(&mut InboxItem)) {
        let id = format!("demo/{n}");
        if let Some(item) = self.items.values_mut().flatten().find(|i| i.id == id) {
            change(item);
        }
    }
}

/// `t` translates the titles and previews, as the interface's other text (English text → the system's language).
pub fn load(inbox: &mut Inbox, now: DateTime<Utc>, t: &dyn Fn(&str) -> String) {
    let passing = || Badge::new(badge::CHECKS_PASSING, "Checks", Tone::Positive);
    let diff = |a: u32, d: u32| Badge::new(badge::DIFF, &format!("+{a} −{d}"), Tone::Neutral);
    let mut demo = Demo { now, items: HashMap::new() };
    demo.add(
        1,
        "github",
        InboxBundle::reviews(),
        &t("Add passkeys to the sign-in page"),
        "acme/web #482",
        12,
        "erin",
        vec![passing(), diff(214, 37)],
        &["you", "frank"],
        None,
    );
    demo.add(
        2,
        "gitlab",
        InboxBundle::reviews(),
        &t("Move the image resizer to the new queue"),
        "acme/platform !1290",
        95,
        "dave",
        vec![Badge::new(badge::CHECKS_FAILING, "Checks failed", Tone::Negative), diff(88, 120)],
        &["you"],
        None,
    );
    demo.add(
        3,
        "slack",
        InboxBundle::mentions(),
        &t("Can you double check the rollout plan before Friday?"),
        "#releases",
        30,
        "Bob",
        vec![],
        &[],
        Some(&t("@you the flag is at 20% in prod, metrics look good so far…")),
    );
    demo.add(
        4,
        "slack",
        InboxBundle::direct_messages(),
        &t("Lunch tomorrow? 🍜"),
        &t("Direct message"),
        140,
        "Carol",
        vec![],
        &[],
        Some(&t("There’s a new ramen place near the office")),
    );
    demo.add(
        5,
        "github",
        InboxBundle::authored(),
        &t("Snooze reminders for review requests"),
        "acme/inbox #77",
        8,
        "you",
        vec![Badge::new(badge::APPROVED, "Approved", Tone::Accent), passing(), diff(320, 12)],
        &["erin", "dave"],
        None,
    );
    demo.add(
        6,
        "github",
        InboxBundle::authored(),
        &t("Cache GraphQL responses per account"),
        "acme/inbox #75",
        400,
        "you",
        vec![Badge::new(badge::CHANGES, "Changes requested", Tone::Negative), passing()],
        &["frank"],
        None,
    );
    demo.add(
        7,
        "github",
        InboxBundle::authored(),
        &t("WIP: Notion tasks bundle"),
        "acme/inbox #79",
        1_500,
        "you",
        vec![Badge::new(badge::DRAFT, "Draft", Tone::Neutral)],
        &[],
        None,
    );
    demo.add(
        8,
        "linear",
        InboxBundle::tasks(),
        &t("Write Q4 hiring scorecard"),
        "ENG-31",
        2_000,
        "Linear",
        vec![Badge::new(badge::DUE_TODAY, "Due today", Tone::Warning)],
        &[],
        None,
    );
    demo.add(
        9,
        "github",
        InboxBundle::reviews(),
        &t("WEB-42 retry failed uploads"),
        "acme/web #490",
        300,
        "erin",
        vec![passing(), diff(64, 10)],
        &[],
        None,
    );
    demo.add(
        10,
        "github",
        InboxBundle::reviews(),
        &t("WEB-42 alert on upload failures"),
        "acme/web #491",
        280,
        "frank",
        vec![],
        &[],
        None,
    );
    demo.add(
        11,
        "github",
        InboxBundle::reviews(),
        &t("Refactor the search service"),
        "acme/search #77",
        900,
        "dave",
        vec![diff(820, 410)],
        &[],
        None,
    );
    demo.add(
        12,
        "linear",
        InboxBundle::tasks(),
        &t("Update the onboarding checklist"),
        "ENG-12",
        40_000,
        "Linear",
        vec![],
        &[],
        None,
    );
    demo.add(
        13,
        "github",
        InboxBundle::authored(),
        &t("Show snooze reasons in notifications"),
        "acme/inbox #81",
        3_000,
        "you",
        vec![passing(), diff(96, 14)],
        &["erin", "bob"],
        None,
    );
    // Reactions as Slack shows them, and a question of yours waiting, acknowledged with a 👍. As on macOS.
    demo.edit(3, |i| i.reactions = Some(vec![Reaction::new("eyes", 2, false)]));
    demo.edit(4, |i| i.reactions = Some(vec![Reaction::new("+1", 2, true)]));
    demo.add(
        15,
        "slack",
        InboxBundle::awaiting(),
        &t("Can someone proofread the new onboarding copy?"),
        &t("Thread in %@").replace("%@", "#design"),
        55,
        "you",
        vec![],
        &[],
        None,
    );
    demo.edit(15, |i| {
        i.needs_action = false;
        i.reactions = Some(vec![Reaction::new("+1", 1, false)]);
    });
    // A message without a clear sign (kept in To reply to be safe), an FYI that can be put in To reply, and one the
    // user already put there (the states below). As on macOS.
    for (n, title, context, minutes, author) in [
        (16, "The client is still waiting for the quote", "#sales", 20, "Dana"),
        (17, "FYI the staging database restarts at 6 pm", "#ops", 75, "Eve"),
        (18, "FYI, the slides from today’s demo are in the shared drive", "#product", 200, "Frank"),
    ] {
        demo.add(n, "slack", InboxBundle::mentions(), &t(title), context, minutes, author, vec![], &[], None);
    }

    let monday = SnoozeClock::presets(now.with_timezone(&chrono::Local))
        .into_iter()
        .find(|p| p.label == "Next week")
        .map(|p| p.date)
        .unwrap_or(now + Duration::days(3));
    let mut states = HashMap::new();
    for item in
        demo.items.values().flatten().filter(|i| ["demo/9", "demo/10", "demo/11", "demo/12"].contains(&i.id.as_str()))
    {
        let reason = if item.id == "demo/11" { SnoozeReason::Motivation } else { SnoozeReason::NoTime };
        let snooze = Snooze {
            until: monday,
            mode: SnoozeMode::Hide,
            note: None,
            fingerprint: item.fingerprint(),
            reason: Some(reason),
            until_news: None,
        };
        states.insert(item.id.clone(), ItemState { snooze: Some(snooze), ..ItemState::default() });
    }
    states.insert("demo/18".into(), ItemState { moved_to_reply_at: Some(now), ..ItemState::default() });
    let accounts = ["github", "gitlab", "slack", "linear"].iter().map(|p| account(p, p)).collect();
    inbox.load_demo(accounts, demo.items, states);
}

/// What the interface reads from the app, for the demo inbox: the preview's fixture
/// (`app/src/fixtures/demo.json`, written by `make desktop-fixture`).
pub fn fixture(now: DateTime<Utc>, t: &dyn Fn(&str) -> String) -> serde_json::Value {
    let mut inbox = Inbox::in_memory(Arc::new(MemoryVault::default()), Managed::default());
    load(&mut inbox, now, t);
    serde_json::json!({
        "inbox": crate::view::inbox_view(&inbox, "", true, &|key| key.to_string()),
        "sources": inbox.sources(),
        "accounts": inbox.account_infos(),
        "preferences": inbox.preferences(),
        "managed": inbox.managed(),
        "policy": inbox.policy(),
        "snoozeReturns": SnoozeReason::ALL
            .iter()
            .map(|reason| (serde_json::to_value(reason).unwrap(), inbox.suggested_return(*reason, now)))
            .map(|(reason, date)| (reason.as_str().unwrap_or_default().to_string(), date))
            .collect::<std::collections::BTreeMap<_, _>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn the_demo_fills_every_tab() {
        let mut inbox = Inbox::in_memory(Arc::new(MemoryVault::default()), Managed::default());
        let now = Utc::now();
        load(&mut inbox, now, &|text| text.to_string());
        let layout = inbox.layout(now, "");
        assert!(layout.my_turn_items().count() >= 6);
        assert!(layout.waiting_count() >= 2);
        assert_eq!(layout.snoozed.len(), 4);
    }

    /// Every field path in a JSON value, array elements merged.
    fn paths(value: &serde_json::Value, prefix: &str, out: &mut BTreeSet<String>) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, value) in map {
                    let path = format!("{prefix}.{key}");
                    paths(value, &path, out);
                    out.insert(path);
                }
            }
            serde_json::Value::Array(values) => {
                values.iter().for_each(|value| paths(value, &format!("{prefix}[]"), out))
            }
            _ => {}
        }
    }

    #[test]
    fn the_preview_fixtures_have_every_field_the_app_sends() {
        let mut expected = BTreeSet::new();
        paths(&super::fixture(Utc::now(), &|text| text.to_string()), "", &mut expected);
        for (name, json) in [
            ("demo.json", include_str!("../../../app/src/fixtures/demo.json")),
            ("demo.fr.json", include_str!("../../../app/src/fixtures/demo.fr.json")),
        ] {
            let mut found = BTreeSet::new();
            paths(&serde_json::from_str(json).unwrap(), "", &mut found);
            assert_eq!(
                expected.symmetric_difference(&found).collect::<Vec<_>>(),
                Vec::<&String>::new(),
                "app/src/fixtures/{name} is stale: run `make desktop-fixture`"
            );
        }
    }

    #[test]
    fn the_french_demo_reads_french() {
        let french = crate::tray::Translator::new(Some("fr_FR"), include_str!("../../../app/src/i18n/fr.json"));
        let mut inbox = Inbox::in_memory(Arc::new(MemoryVault::default()), Managed::default());
        load(&mut inbox, Utc::now(), &|text| french.t(text));
        let layout = inbox.layout(Utc::now(), "");
        let titles: Vec<_> = layout.my_turn_items().map(|i| i.title.as_str()).collect();
        assert!(titles.contains(&"Ajouter les passkeys à la page de connexion"), "{titles:?}");
        assert!(titles.contains(&"On déjeune ensemble demain ? 🍜"), "{titles:?}");
    }
}
