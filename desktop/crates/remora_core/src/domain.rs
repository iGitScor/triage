use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

/// Anything that deserves attention: a review request, a Slack mention, a Linear issue, a reminder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename = "Item")]
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
    /// How a reminder repeats. Reminders only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<crate::Recurrence>,
    /// Emoji reactions on the message (Slack); None for tools without them. Not in the fingerprint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reactions: Option<Vec<crate::Reaction>>,
    /// Why Remora put a chat message where it is; None for what the source sorted. Not in the fingerprint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reason: Option<SortReason>,
}

/// Why a message went where it is (Swift's `SortReason`): the words that decided, the model, nothing clear, or you.
/// Shown on the row and given to agents, so a strong signal can be told from a guess. Set on chat messages, the items
/// Remora sorts by their text; a source's own sort is told by its badges. `code` is one of `sort_reason`'s, kept as
/// a string so a saved inbox holding a code this version doesn't know still loads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SortReason {
    pub code: String,
    /// The words that decided, as found in the message ("can you", "merci de", "fyi"), for `KEYWORD`, `NO_REPLY`,
    /// `INFO` and `CLOSING`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub detail: Option<String>,
}

impl SortReason {
    pub fn new(code: &str, detail: Option<&str>) -> Self {
        Self { code: code.into(), detail: detail.map(Into::into) }
    }
}

/// `SortReason` codes, the same strings as the Mac's.
pub mod sort_reason {
    /// A question mark ends a sentence.
    pub const QUESTION: &str = "question";
    /// Words that ask: "can you", "peux-tu", "merci de relire".
    pub const KEYWORD: &str = "keyword";
    /// Words that say nothing is expected: "no need to reply".
    pub const NO_REPLY: &str = "noReply";
    /// Words that announce: "fyi", "@here", "pour info".
    pub const INFO: &str = "info";
    /// A short thank-you or OK, or nothing but emoji.
    pub const CLOSING: &str = "closing";
    /// Nothing but code or a link.
    pub const CODE_ONLY: &str = "codeOnly";
    /// No words decided; a model read the meaning (the Mac's sentence embeddings; none yet on Windows and Linux).
    pub const MODEL: &str = "model";
    /// Nothing clear: a message to you stays in To reply to be safe, a message of yours isn't taken for a question.
    pub const UNSURE: &str = "unsure";
    /// You put it in To reply.
    pub const MOVED: &str = "moved";
}

/// What a pull or merge request changes: paths and line counts only.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
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

    /// Written by an app, a bot or an agent account, as the source declares it.
    pub fn from_bot(&self) -> bool {
        self.author.as_ref().is_some_and(|a| a.bot)
    }

    /// Whether the user can put it in To reply: a chat message Remora sorted elsewhere by its text.
    pub fn can_move_to_reply(&self) -> bool {
        self.reason.is_some() && self.bundle != InboxBundle::reply() && self.bundle != InboxBundle::awaiting()
    }

    /// As the user put it: in To reply, waiting on them, said so on the row.
    pub fn moved_to_reply(mut self) -> Self {
        if self.can_move_to_reply() {
            self.bundle = InboxBundle::reply();
            self.needs_action = true;
            self.expires = None;
            self.reason = Some(SortReason::new(sort_reason::MOVED, None));
        }
        self
    }

    /// Item ids are the account's id, a slash, then the plugin's own id (Swift's `InboxItem.makeID`). Two accounts on
    /// the same tool never share an item (Done, snoozes and pins are kept by id), and an account connected again finds
    /// its items.
    pub fn make_id(account_id: &str, local: impl std::fmt::Display) -> String {
        format!("{}{local}", Self::account_prefix(account_id))
    }

    /// What starts the id of every item of this account.
    pub fn account_prefix(account_id: &str) -> String {
        format!("{account_id}/")
    }

    /// The plugin's own part of the id; none when the id doesn't start with this item's account.
    pub fn local_id(&self) -> Option<&str> {
        self.id.strip_prefix(self.account_id.as_str())?.strip_prefix('/')
    }

    /// Counted in the tray and announced on arrival.
    pub fn counts(&self) -> bool {
        !self.bundle.is_quiet() && self.priority != Some(Priority::Low)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", from = "SavedPerson")]
pub struct Person {
    pub name: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    /// A reviewer's stand; none while they haven't reviewed. The interface draws its colour and glyph from it.
    #[serde(default)]
    pub review: Option<ReviewState>,
    /// An app, a bot or an agent account, as the source declares it (GitHub's `Bot`, a Slack app, a Teams application,
    /// a Jira app account): never guessed from a name. Shown next to the author; the inbox can group their items apart.
    #[serde(default)]
    pub bot: bool,
}

/// Where a reviewer stands on a pull request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ReviewState {
    Approved,
    ChangesRequested,
}

/// A person as saved, also before 0.10, when a reviewer's stand was only the colour of their avatar (`tone`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedPerson {
    name: String,
    #[serde(default)]
    avatar_url: Option<String>,
    #[serde(default)]
    review: Option<ReviewState>,
    #[serde(default)]
    tone: Option<Tone>,
    #[serde(default)]
    bot: bool,
}

impl From<SavedPerson> for Person {
    fn from(saved: SavedPerson) -> Self {
        let review = saved.review.or(match saved.tone {
            Some(Tone::Accent | Tone::Positive) => Some(ReviewState::Approved),
            Some(Tone::Negative) => Some(ReviewState::ChangesRequested),
            _ => None,
        });
        Person { name: saved.name, avatar_url: saved.avatar_url, review, bot: saved.bot }
    }
}

impl Person {
    pub fn named(name: &str) -> Self {
        Person { name: name.to_string(), avatar_url: None, review: None, bot: false }
    }
}

/// A status chip. When `notify` is set, its first appearance on an item triggers a notification.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Badge {
    pub id: String,
    /// English text, translated by the UI.
    pub label: String,
    pub tone: Tone,
    #[serde(default)]
    pub notify: Option<String>,
}

/// The badge ids plugins write and the rules read back (Swift's `BadgeID`), so a typo is a compile error, not an
/// item quietly in the wrong verb. They are saved with the inbox: renaming one is a format change.
pub mod badge {
    pub const APPROVED: &str = "approved";
    pub const CHANGES: &str = "changes";
    pub const DRAFT: &str = "draft";
    pub const CONFLICTS: &str = "conflicts";
    pub const CHECKS_PASSING: &str = "checks.passing";
    pub const CHECKS_FAILING: &str = "checks.failing";
    pub const CHECKS_RUNNING: &str = "checks.running";
    pub const COMMENTS: &str = "comments";
    pub const DIFF: &str = "diff";
    pub const STATUS: &str = "status";
    pub const PRIORITY_HIGH: &str = "priority.high";
    pub const PRIORITY_URGENT: &str = "priority.urgent";
    pub const DUE: &str = "due";
    pub const DUE_TODAY: &str = "due.today";
    pub const OVERDUE: &str = "overdue";
    pub const IN_PROGRESS: &str = "inProgress";
    pub const INVITATION: &str = "invitation";
    pub const FLAGGED: &str = "flagged";
    pub const EVENT: &str = "event";
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    Accent,
    Positive,
    Negative,
    Warning,
    Neutral,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    Normal,
    High,
    Urgent,
}

/// Items are grouped by what they ask of you (verbs), not by where they come from.
/// Plugins report a *kind*; `VerbClassifier` moves items to a *verb*.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(rename = "Bundle")]
pub struct InboxBundle {
    pub id: String,
    pub title: String,
    pub rank: i32,
}

impl InboxBundle {
    fn of(id: &str, title: &str, rank: i32) -> Self {
        InboxBundle { id: id.into(), title: title.into(), rank }
    }

    pub fn reminders() -> Self {
        Self::of("reminders", "Reminders", 0)
    }
    pub fn reply() -> Self {
        Self::of("verb.reply", "To reply", 1)
    }
    pub fn reviews() -> Self {
        Self::of("code.review", "To review", 2)
    }
    pub fn fix() -> Self {
        Self::of("verb.fix", "To fix", 3)
    }
    pub fn merge() -> Self {
        Self::of("verb.merge", "Ready to merge", 4)
    }
    pub fn tasks() -> Self {
        Self::of("docs.tasks", "To do", 5)
    }
    pub fn read() -> Self {
        Self::of("verb.read", "To read", 8)
    }
    pub fn awaiting() -> Self {
        Self::of("verb.awaiting", "Waiting on others", 9)
    }
    /// A conversation that needs nothing more from you: placed in Done until someone writes again.
    pub fn finished() -> Self {
        Self::of("verb.finished", "Finished", 10)
    }
    /// Not a verb: where items written by apps, bots and agents go when the user groups them apart, last in My turn
    /// and still counted. Each item keeps its own verb.
    pub fn bots() -> Self {
        Self::of("group.bots", "Bots and agents", 11)
    }
    pub fn mentions() -> Self {
        Self::of("chat.mentions", "Mentions", 6)
    }
    pub fn direct_messages() -> Self {
        Self::of("chat.direct", "Direct messages", 7)
    }
    pub fn authored() -> Self {
        Self::of("code.authored", "Your merge requests", 9)
    }
    /// Your last message in a chat thread nobody answered yet: `VerbClassifier` keeps the questions, as Waiting.
    pub fn asked() -> Self {
        Self::of("chat.asked", "Your questions", 9)
    }
    /// Your answer, last in a conversation someone else started: a question waits on them, anything else finishes it.
    pub fn answered() -> Self {
        Self::of("chat.answered", "Your answers", 9)
    }

    /// Whose turn the bundle is, when the bundle alone tells. Kinds leave it to `needs_action`.
    pub fn is_mine(&self) -> Option<bool> {
        match self.id.as_str() {
            "verb.awaiting" => Some(false),
            "code.authored" | "chat.mentions" | "chat.direct" | "chat.asked" | "chat.answered" => None,
            _ => Some(true),
        }
    }

    /// Shown, but neither counted nor announced: what's to read, and a finished conversation you pinned.
    pub fn is_quiet(&self) -> bool {
        self.id == "verb.read" || self.id == "verb.finished"
    }
}

/// What the user decided about an item. Kept locally, never sent to the source.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct ItemState {
    pub pinned: bool,
    pub done: Option<Mark>,
    pub snooze: Option<Snooze>,
    pub reminded_at: Option<DateTime<Utc>>,
    /// Set while the user is working on it: the item leaves the inbox for the In progress view.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub started_at: Option<DateTime<Utc>>,
    /// When the user put a message Remora sorted elsewhere in To reply: it stays there, whatever its words say. Only
    /// towards To reply, so a correction can never hide a request.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub moved_to_reply_at: Option<DateTime<Utc>>,
    /// When the user handed it to an agent (a prompt copied): the row says so, so it isn't forgotten or done twice.
    /// The item stays where it is, and leaves as usual when the tool shows the reply or the fix.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub handed_off_at: Option<DateTime<Utc>>,
}

impl ItemState {
    pub fn is_empty(&self) -> bool {
        !self.pinned
            && self.done.is_none()
            && self.snooze.is_none()
            && self.reminded_at.is_none()
            && self.started_at.is_none()
            && self.moved_to_reply_at.is_none()
            && self.handed_off_at.is_none()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Mark {
    pub at: DateTime<Utc>,
    pub fingerprint: String,
    #[serde(default)]
    pub cleared_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum SnoozeMode {
    Hide,
    Remind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
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
    pub const ALL: [SnoozeReason; 5] = [
        SnoozeReason::Waiting,
        SnoozeReason::NoTime,
        SnoozeReason::Focus,
        SnoozeReason::NotUrgent,
        SnoozeReason::Motivation,
    ];
}

/// One connected instance of a plugin. Secrets live in the credential store, never here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Egress {
    pub hosts: Vec<String>,
    pub description: String,
    pub external_ai: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename = "Field")]
pub struct ConfigField {
    pub key: String,
    pub label: String,
    pub placeholder: String,
    pub default_value: String,
    pub is_secret: bool,
    pub is_optional: bool,
    pub help: Option<String>,
    /// The policy value that sets this field for the organization (`MicrosoftTenant`), locking it in the form.
    #[serde(default)]
    pub managed_key: Option<String>,
    /// An on/off option, shown as a switch: "true" when on, empty when off.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_toggle: bool,
}

impl ConfigField {
    /// An option that is off unless turned on.
    pub fn toggle(key: &str, label: &str, help: &str) -> Self {
        ConfigField {
            key: key.into(),
            label: label.into(),
            placeholder: String::new(),
            default_value: String::new(),
            is_secret: false,
            is_optional: true,
            help: Some(help.into()),
            managed_key: None,
            is_toggle: true,
        }
    }

    pub fn token(label: &str, help: &str) -> Self {
        ConfigField {
            key: "token".into(),
            label: label.into(),
            placeholder: String::new(),
            default_value: String::new(),
            is_secret: true,
            is_optional: false,
            help: Some(help.into()),
            managed_key: None,
            is_toggle: false,
        }
    }

    /// A free text the plugin may do without.
    pub fn text(key: &str, label: &str, placeholder: &str, help: &str) -> Self {
        ConfigField {
            key: key.into(),
            label: label.into(),
            placeholder: placeholder.into(),
            default_value: String::new(),
            is_secret: false,
            is_optional: true,
            help: Some(help.into()),
            managed_key: None,
            is_toggle: false,
        }
    }

    pub fn host(default: &str) -> Self {
        ConfigField {
            key: "host".into(),
            label: "Host".into(),
            placeholder: default.into(),
            default_value: default.into(),
            is_secret: false,
            is_optional: false,
            help: None,
            managed_key: None,
            is_toggle: false,
        }
    }
}

/// Describes a plugin and the settings it needs. The settings UI is generated from it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename = "Manifest")]
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
    /// Set when the account signs in through the browser instead of pasting a token.
    #[serde(default)]
    pub sign_in: Option<SignIn>,
}

/// Signing in through the browser: OAuth 2's authorization code with PKCE, for an app that keeps no secret, the
/// answer coming back to this computer. `{key}` in the URLs is replaced by that setting of the account (a tenant).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SignIn {
    pub authorize_url: String,
    pub token_url: String,
    /// The setting holding the app's client id, registered by the organization.
    pub client_id_key: String,
    pub scopes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An inbox saved before 0.10 gave a reviewer's stand only as the colour of their avatar.
    #[test]
    fn a_saved_reviewer_colour_becomes_their_stand() {
        let read = |json: &str| serde_json::from_str::<Person>(json).expect("person").review;
        assert_eq!(read(r#"{"name":"a","tone":"accent"}"#), Some(ReviewState::Approved));
        assert_eq!(read(r#"{"name":"a","tone":"positive"}"#), Some(ReviewState::Approved));
        assert_eq!(read(r#"{"name":"a","tone":"negative"}"#), Some(ReviewState::ChangesRequested));
        assert_eq!(read(r#"{"name":"a","tone":null}"#), None);
        assert_eq!(read(r#"{"name":"a","review":"changesRequested"}"#), Some(ReviewState::ChangesRequested));
        let saved =
            serde_json::to_string(&Person { review: Some(ReviewState::Approved), ..Person::named("a") }).unwrap();
        assert_eq!(saved, r#"{"name":"a","avatarUrl":null,"review":"approved","bot":false}"#);
    }
}
