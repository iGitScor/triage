//! What the tray shows: the count, the tooltip, the icon and the one task in progress, and the strings Rust
//! translates itself. The tray app only draws it, so all of it is tested here.

use crate::{Inbox, TrayCount};
use chrono::{DateTime, Utc};
use remora_core::Notice;
use std::collections::HashMap;

/// English text → the system's language, from the interface's dictionary (src/i18n/fr.json).
pub struct Translator {
    dictionary: Option<HashMap<String, String>>,
}

impl Translator {
    /// French when the system locale is, with that dictionary; English otherwise.
    pub fn new(locale: Option<&str>, french_json: &str) -> Self {
        let french = locale.is_some_and(|l| l.to_lowercase().starts_with("fr"));
        Translator { dictionary: french.then(|| serde_json::from_str(french_json).unwrap_or_default()) }
    }

    pub fn english() -> Self {
        Translator { dictionary: None }
    }

    /// The language chosen, for the interface to use the same one.
    pub fn lang(&self) -> &'static str {
        if self.dictionary.is_some() { "fr" } else { "en" }
    }

    pub fn t(&self, text: &str) -> String {
        self.dictionary.as_ref().and_then(|d| d.get(text)).cloned().unwrap_or_else(|| text.to_string())
    }
}

/// The strings the tray and its menu show, each translated in fr.json.
pub const TRAY_STRINGS: &[&str] = &[
    "Nothing needs you", "%d need you", "%d in progress", "%d min", "%d h",
    "Done", "Stop", "Open Remora", "Refresh", "New reminder", "Settings", "Quit",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayImage {
    Quiet,
    Attention,
    /// The inverted icon: focus on a task in progress.
    Focus,
}

/// The inbox, as the tray sums it up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraySummary {
    pub count: usize,
    pub show_count: bool,
    pub running: usize,
    pub since: Option<DateTime<Utc>>,
    /// While a task is in progress and focus is on, only that task shows.
    pub focused: bool,
    /// The one task in progress (id, title), when there is exactly one: the menu starts with it.
    pub task: Option<(String, String)>,
}

impl TraySummary {
    pub fn of(inbox: &Inbox, now: DateTime<Utc>) -> Self {
        let layout = inbox.layout(now, "");
        let running = layout.in_progress.len();
        let since = layout.in_progress.first().and_then(|i| inbox.states.get(&i.id)).and_then(|s| s.started_at);
        let task = (running == 1).then(|| (layout.in_progress[0].id.clone(), layout.in_progress[0].title.clone()));
        let (count, show_count) = match inbox.preferences.tray_count {
            TrayCount::WaitingOnMe => (layout.action_count(), true),
            TrayCount::Everything => (layout.my_turn_items().count() + layout.waiting_count(), true),
            TrayCount::Hidden => (layout.action_count(), false),
        };
        let focused = inbox.preferences.focus_while_in_progress && running > 0;
        TraySummary { count, show_count, running, since, focused, task }
    }

    /// "Remora · 3 need you", "Remora · 1 in progress (25 min)", or both.
    pub fn tooltip(&self, now: DateTime<Utc>, t: &Translator) -> String {
        let counts = match self.count {
            0 => t.t("Nothing needs you"),
            n => t.t("%d need you").replace("%d", &n.to_string()),
        };
        match (self.running, self.focused, self.show_count) {
            (0, _, true) => format!("Remora · {counts}"),
            (0, _, false) => "Remora".into(),
            (_, true, _) | (_, _, false) => format!("Remora · {}", running_text(self.running, self.since, now, t)),
            _ => format!("Remora · {} · {counts}", running_text(self.running, self.since, now, t)),
        }
    }

    pub fn icon(&self) -> TrayImage {
        if self.focused {
            TrayImage::Focus
        } else if (self.count > 0 && self.show_count) || self.running > 0 {
            TrayImage::Attention
        } else {
            TrayImage::Quiet
        }
    }
}

/// "1 in progress (25 min)", "1 in progress (2 h 05)".
pub fn running_text(running: usize, since: Option<DateTime<Utc>>, now: DateTime<Utc>, t: &Translator) -> String {
    let text = t.t("%d in progress").replace("%d", &running.to_string());
    let Some(since) = since else { return text };
    let minutes = (now - since).num_minutes().max(0);
    let elapsed = if minutes < 60 {
        t.t("%d min").replace("%d", &minutes.to_string())
    } else {
        let hours = t.t("%d h").replace("%d", &(minutes / 60).to_string());
        if minutes % 60 == 0 { hours } else { format!("{hours} {:02}", minutes % 60) }
    };
    format!("{text} ({elapsed})")
}

/// A notification's title and its two lines: where (the subtitle), then what (the body), translated.
pub fn notification_text(notice: &Notice, t: &Translator) -> (String, String, String) {
    let body = [notice.subtitle.as_str(), notice.body.as_str()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n");
    let body = t.t(&body);
    let mut lines = body.splitn(2, '\n');
    let first = lines.next().unwrap_or_default().to_string();
    (t.t(&notice.title), first, lines.next().unwrap_or_default().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Managed, MemoryVault};
    use chrono::Duration;
    use remora_core::NoticeKind;
    use std::sync::Arc;

    const FRENCH: &str = include_str!("../../../app/src/i18n/fr.json");

    fn now() -> DateTime<Utc> {
        "2026-10-08T09:00:00Z".parse().unwrap()
    }

    fn inbox_with_reminders(titles: &[&str]) -> (Inbox, Vec<String>) {
        let mut inbox = Inbox::in_memory(Arc::new(MemoryVault::default()), Managed::default());
        let ids = titles.iter().map(|t| inbox.add_reminder(t, now() - Duration::minutes(1), now() - Duration::hours(1)).unwrap()).collect();
        inbox.tick(now());
        (inbox, ids)
    }

    /// The interface is told the same language.
    #[test]
    fn the_language_chosen_is_told_to_the_interface() {
        assert_eq!(Translator::new(Some("fr-FR"), FRENCH).lang(), "fr");
        assert_eq!(Translator::new(Some("en-GB"), FRENCH).lang(), "en");
        assert_eq!(Translator::english().lang(), "en");
    }

    #[test]
    fn every_tray_string_is_in_french() {
        let dictionary: HashMap<String, String> = serde_json::from_str(FRENCH).unwrap();
        for text in TRAY_STRINGS {
            assert!(dictionary.contains_key(*text), "{text} has no French translation in fr.json");
        }
        assert_eq!(Translator::new(Some("fr-FR"), FRENCH).t("Quit"), "Quitter");
        assert_eq!(Translator::new(Some("en-GB"), FRENCH).t("Quit"), "Quit");
        assert_eq!(Translator::new(None, FRENCH).t("Quit"), "Quit");
    }

    #[test]
    fn a_quiet_inbox() {
        let (inbox, _) = inbox_with_reminders(&[]);
        let summary = TraySummary::of(&inbox, now());
        assert_eq!(summary.tooltip(now(), &Translator::english()), "Remora · Nothing needs you");
        assert_eq!(summary.icon(), TrayImage::Quiet);
        assert_eq!(summary.task, None);
    }

    #[test]
    fn counts_what_needs_you_unless_hidden() {
        let (mut inbox, _) = inbox_with_reminders(&["Call the bank", "Book the room"]);
        let summary = TraySummary::of(&inbox, now());
        assert_eq!(summary.tooltip(now(), &Translator::english()), "Remora · 2 need you");
        assert_eq!(summary.icon(), TrayImage::Attention);

        inbox.preferences.tray_count = TrayCount::Hidden;
        let hidden = TraySummary::of(&inbox, now());
        assert_eq!(hidden.tooltip(now(), &Translator::english()), "Remora");
        assert_eq!(hidden.icon(), TrayImage::Quiet);
    }

    #[test]
    fn one_task_in_progress_takes_over_while_focused() {
        let (mut inbox, ids) = inbox_with_reminders(&["Write the report", "Book the room"]);
        inbox.start(&ids[0], now() - Duration::minutes(125));
        let summary = TraySummary::of(&inbox, now());
        assert_eq!(summary.task, Some((ids[0].clone(), "Write the report".into())));
        assert_eq!(summary.icon(), TrayImage::Focus);
        assert_eq!(summary.tooltip(now(), &Translator::english()), "Remora · 1 in progress (2 h 05)");

        inbox.preferences.focus_while_in_progress = false;
        let unfocused = TraySummary::of(&inbox, now());
        assert_eq!(unfocused.icon(), TrayImage::Attention);
        assert_eq!(unfocused.tooltip(now(), &Translator::english()), "Remora · 1 in progress (2 h 05) · 1 need you");

        inbox.start(&ids[1], now());
        assert_eq!(TraySummary::of(&inbox, now()).task, None, "the menu shows a task only when there's one");
    }

    #[test]
    fn running_text_in_both_languages() {
        let english = Translator::english();
        let started = Some(now() - Duration::minutes(25));
        assert_eq!(running_text(1, started, now(), &english), "1 in progress (25 min)");
        assert_eq!(running_text(1, Some(now() - Duration::hours(2)), now(), &english), "1 in progress (2 h)");
        assert_eq!(running_text(2, None, now(), &english), "2 in progress");
        let french = running_text(1, started, now(), &Translator::new(Some("fr"), FRENCH));
        assert!(!french.contains("in progress"), "{french}");
    }

    #[test]
    fn notifications_say_where_then_what() {
        let notice = |subtitle: &str, body: &str| Notice {
            kind: NoticeKind::Arrival,
            item_id: "1".into(),
            title: "New review".into(),
            subtitle: subtitle.into(),
            body: body.into(),
            url: None,
        };
        let english = Translator::english();
        assert_eq!(notification_text(&notice("acme/app #1", "Fix login"), &english), ("New review".into(), "acme/app #1".into(), "Fix login".into()));
        assert_eq!(notification_text(&notice("", "Fix login"), &english).1, "Fix login");
    }
}
