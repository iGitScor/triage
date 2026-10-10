use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Anything that deserves attention: a review request, a Slack mention, a Linear issue, a reminder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItem {
    pub id: String,
    pub account_id: String,
    pub plugin_id: String,
    pub bundle: InboxBundle,
    pub title: String,
    pub context: String,
    #[serde(default)]
    pub preview: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    /// Opens the item in the tool's desktop app (slack://, linear://) when one is installed.
    #[serde(default)]
    pub app_url: Option<String>,
    #[serde(default)]
    pub author: Option<Person>,
    #[serde(default)]
    pub participants: Vec<Person>,
    #[serde(default)]
    pub badges: Vec<Badge>,
    pub date: DateTime<Utc>,
    #[serde(default)]
    pub needs_action: bool,
    #[serde(default)]
    pub priority: Option<Priority>,
    #[serde(default)]
    pub due: Option<DateTime<Utc>>,
    /// When it stops being worth showing (an app's reminder of an event that has started): it then leaves the
    /// inbox as if cleared, unless you pinned or started it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<DateTime<Utc>>,
    /// The changed files' paths and line counts (never the code), for review prep.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes: Option<ChangeSet>,
    /// People the tool suggests as reviewers, for the waiting assistant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_people: Option<Vec<Person>>,
}

/// What a pull or merge request changes: paths and line counts only.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSet {
    pub files: Vec<ChangedFile>,
    /// The total the tool gives, which can be more than the files listed.
    pub file_count: usize,
}

impl ChangeSet {
    pub fn new(files: Vec<ChangedFile>, file_count: Option<usize>) -> Self {
        let file_count = file_count.unwrap_or(files.len()).max(files.len());
        ChangeSet { files, file_count }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: String,
    #[serde(default)]
    pub additions: Option<u32>,
    #[serde(default)]
    pub deletions: Option<u32>,
}

impl ChangedFile {
    pub fn lines(&self) -> u32 {
        self.additions.unwrap_or(0) + self.deletions.unwrap_or(0)
    }
}

impl InboxItem {
    /// Changes whenever something noteworthy happens, so Done or snoozed items can resurface.
    pub fn fingerprint(&self) -> String {
        let mut ids: Vec<&str> = self.badges.iter().map(|b| b.id.as_str()).collect();
        ids.sort_unstable();
        format!("{}|{}", self.date.timestamp(), ids.join(","))
    }

    pub fn has_badge(&self, id: &str) -> bool {
        self.badges.iter().any(|b| b.id == id)
    }

    /// Counted in the tray and announced on arrival.
    pub fn counts(&self) -> bool {
        !self.bundle.is_quiet() && self.priority != Some(Priority::Low)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub name: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub tone: Option<Tone>,
}

impl Person {
    pub fn named(name: &str) -> Self {
        Person { name: name.to_string(), avatar_url: None, tone: None }
    }
}

/// A status chip. When `notify` is set, its first appearance on an item triggers a notification.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Badge {
    pub id: String,
    /// English text, translated by the UI.
    pub label: String,
    pub tone: Tone,
    #[serde(default)]
    pub notify: Option<String>,
}

impl Badge {
    pub fn new(id: &str, label: &str, tone: Tone) -> Self {
        Badge { id: id.into(), label: label.into(), tone, notify: None }
    }

    pub fn notifying(mut self, title: &str) -> Self {
        self.notify = Some(title.into());
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    Accent,
    Positive,
    Negative,
    Warning,
    Neutral,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    Normal,
    High,
    Urgent,
}

/// Items are grouped by what they ask of you (verbs), not by where they come from.
/// Plugins report a *kind*; `VerbClassifier` moves items to a *verb*.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InboxBundle {
    pub id: String,
    pub title: String,
    pub rank: i32,
}

impl InboxBundle {
    fn of(id: &str, title: &str, rank: i32) -> Self {
        InboxBundle { id: id.into(), title: title.into(), rank }
    }

    pub fn reminders() -> Self { Self::of("reminders", "Reminders", 0) }
    pub fn reply() -> Self { Self::of("verb.reply", "To reply", 1) }
    pub fn reviews() -> Self { Self::of("code.review", "To review", 2) }
    pub fn fix() -> Self { Self::of("verb.fix", "To fix", 3) }
    pub fn merge() -> Self { Self::of("verb.merge", "Ready to merge", 4) }
    pub fn tasks() -> Self { Self::of("docs.tasks", "To do", 5) }
    pub fn read() -> Self { Self::of("verb.read", "To read", 8) }
    pub fn awaiting() -> Self { Self::of("verb.awaiting", "Waiting on others", 9) }
    pub fn mentions() -> Self { Self::of("chat.mentions", "Mentions", 6) }
    pub fn direct_messages() -> Self { Self::of("chat.direct", "Direct messages", 7) }
    pub fn authored() -> Self { Self::of("code.authored", "Your merge requests", 9) }

    /// Whose turn the bundle is, when the bundle alone tells. Kinds leave it to `needs_action`.
    pub fn is_mine(&self) -> Option<bool> {
        match self.id.as_str() {
            "verb.awaiting" => Some(false),
            "code.authored" | "chat.mentions" | "chat.direct" => None,
            _ => Some(true),
        }
    }

    /// Shown, but neither counted nor announced.
    pub fn is_quiet(&self) -> bool {
        self.id == "verb.read"
    }
}

/// What the user decided about an item. Kept locally, never sent to the source.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ItemState {
    pub pinned: bool,
    pub done: Option<Mark>,
    pub snooze: Option<Snooze>,
    pub reminded_at: Option<DateTime<Utc>>,
    /// Set while the user is working on it: the item leaves the inbox for the In progress view.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
}

impl ItemState {
    pub fn is_empty(&self) -> bool {
        !self.pinned && self.done.is_none() && self.snooze.is_none() && self.reminded_at.is_none() && self.started_at.is_none()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mark {
    pub at: DateTime<Utc>,
    pub fingerprint: String,
    #[serde(default)]
    pub cleared_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snooze {
    pub until: DateTime<Utc>,
    pub mode: SnoozeMode,
    #[serde(default)]
    pub note: Option<String>,
    pub fingerprint: String,
    #[serde(default)]
    pub reason: Option<SnoozeReason>,
    /// Comes back on any new activity, whatever the global setting.
    #[serde(default)]
    pub until_news: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SnoozeMode {
    Hide,
    Remind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SnoozeReason {
    Waiting,
    NoTime,
    Focus,
    NotUrgent,
    Motivation,
}

impl SnoozeReason {
    /// In the order the snooze sheet shows them, as on macOS.
    pub const ALL: [SnoozeReason; 5] =
        [SnoozeReason::Waiting, SnoozeReason::NoTime, SnoozeReason::Focus, SnoozeReason::NotUrgent, SnoozeReason::Motivation];
}

/// One connected instance of a plugin. Secrets live in the credential store, never here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub plugin_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub settings: HashMap<String, String>,
    #[serde(default)]
    pub identity: Option<String>,
}

/// Where a plugin's data goes. Declared by every plugin and enforced by the guarded HTTP client.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Egress {
    pub hosts: Vec<String>,
    pub description: String,
    pub external_ai: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigField {
    pub key: String,
    pub label: String,
    pub placeholder: String,
    pub default_value: String,
    pub is_secret: bool,
    pub is_optional: bool,
    pub help: Option<String>,
}

impl ConfigField {
    pub fn token(label: &str, help: &str) -> Self {
        ConfigField {
            key: "token".into(), label: label.into(), placeholder: String::new(), default_value: String::new(),
            is_secret: true, is_optional: false, help: Some(help.into()),
        }
    }

    pub fn host(default: &str) -> Self {
        ConfigField {
            key: "host".into(), label: "Host".into(), placeholder: default.into(), default_value: default.into(),
            is_secret: false, is_optional: false, help: None,
        }
    }
}

/// Describes a plugin and the settings it needs. The settings UI is generated from it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub fields: Vec<ConfigField>,
    pub setup_steps: Vec<String>,
    pub setup_label: String,
    /// A link to create the token; `{host}` is replaced by the account's host.
    pub setup_url: Option<String>,
    pub egress: Egress,
    /// File name of the bundled logo (SVG).
    pub logo: Option<String>,
}
