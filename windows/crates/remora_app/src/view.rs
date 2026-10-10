use chrono::{DateTime, Utc};
use crate::Inbox;
use remora_core::{host_matches, InboxItem, InboxLayout, ItemState, ReviewSize, SnoozeInsight, WaitingHelp};
use remora_plugins::registry;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub in_progress: usize,
    pub my_turn: usize,
    pub waiting: usize,
    pub snoozed: usize,
    pub done: usize,
}

/// What the inbox screen shows.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxView {
    pub layout: InboxLayout,
    pub counts: Counts,
    pub last_refresh: Option<DateTime<Utc>>,
    pub errors: Vec<String>,
    /// What the footer says about the sources, and the accounts to reconnect by name.
    pub health: remora_core::SourcesHealth,
    pub reconnect_names: Vec<String>,
    /// Lists cut at their limit, missing permissions: in the footer, details in Settings.
    pub remarks: Vec<String>,
    pub connected: bool,
    pub demo: bool,
    /// Account names to show next to items, only when a tool has several accounts.
    pub account_names: HashMap<String, String>,
    /// Pinned, snoozed until, reminded: what the user decided about each item.
    pub states: HashMap<String, ItemState>,
    /// What Remora can tell about each shown item: review prep, help with what you wait on, the same
    /// thing in another tool, why it ranks where it does, how often it was snoozed.
    pub extras: HashMap<String, ItemExtras>,
    /// Patterns in the snoozed pile, one card at a time, with the id "Not now" dismisses.
    pub insights: Vec<InsightView>,
    /// Your review pace (1 until five were timed), for the review session's time left.
    pub review_pace: f64,
    /// The assistant, when one is connected and allowed: nothing about it shows otherwise.
    pub assistant: Option<AssistantView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantView {
    pub name: String,
    /// The brief of the whole inbox is on in Settings.
    pub whole_inbox: bool,
    pub brief: Option<remora_core::Brief>,
    /// While fresh, a new brief isn't asked for.
    pub brief_fresh: bool,
    /// A summary per group, by bundle id.
    pub summaries: HashMap<String, String>,
    /// Claude's proposal for the snoozed pile, until applied or discarded.
    pub triage: Vec<remora_core::TriageSuggestion>,
}

#[derive(Serialize)]
pub struct InsightView {
    pub id: String,
    /// For "You often put off…": a small way in (English key).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nudge: Option<&'static str>,
    #[serde(flatten)]
    pub insight: SnoozeInsight,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ItemExtras {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prep: Option<PrepView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<HelpView>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub linked: Vec<LinkedView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ranking: Option<String>,
    #[serde(skip_serializing_if = "is_zero")]
    pub snoozed_times: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepView {
    pub minutes: u32,
    pub file_count: usize,
    pub lines: Option<u32>,
    pub size: &'static str,
    pub tests_touched: bool,
    /// English titles (the interface translates them) and a symbol name for its icons.
    pub flags: Vec<(&'static str, &'static str)>,
    pub top_files: Vec<remora_core::ChangedFile>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelpView {
    /// "suggestReviewers" or "nudge".
    pub kind: &'static str,
    pub people: Vec<remora_core::Person>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedView {
    pub id: String,
    pub title: String,
    pub context: String,
    pub plugin_id: String,
}

fn extras(inbox: &Inbox, item: &InboxItem, now: DateTime<Utc>, tr: &dyn Fn(&str) -> String) -> ItemExtras {
    ItemExtras {
        prep: inbox.review_prep(item).map(|p| PrepView {
            minutes: p.estimated_minutes,
            file_count: p.file_count,
            lines: p.lines,
            size: match p.size {
                ReviewSize::Tiny => "tiny",
                ReviewSize::Small => "small",
                ReviewSize::Medium => "medium",
                ReviewSize::Large => "large",
            },
            tests_touched: p.tests_touched,
            flags: p.flags.iter().map(|f| (f.title(), f.symbol())).collect(),
            top_files: p.top_files,
        }),
        help: inbox.waiting_help(item, now).map(|help| match help {
            WaitingHelp::SuggestReviewers(people) => HelpView { kind: "suggestReviewers", people, days: None },
            WaitingHelp::Nudge { people, days } => HelpView { kind: "nudge", people, days: Some(days) },
        }),
        linked: inbox
            .linked(&item.id)
            .into_iter()
            .map(|other| LinkedView { id: other.id, title: other.title, context: other.context, plugin_id: other.plugin_id })
            .collect(),
        ranking: inbox.ranking_reason(item, tr),
        snoozed_times: if inbox.states.get(&item.id).and_then(|s| s.snooze.as_ref()).is_some() { inbox.snooze_count(&item.id, now) } else { 0 },
    }
}

/// The inbox screen's data. Avatars are kept only from allowed hosts, when the policy allows images.
/// `tr`: English key → the interface's language, for the few sentences built here (ranking reasons).
pub fn inbox_view(inbox: &Inbox, query: &str, demo: bool, tr: &dyn Fn(&str) -> String) -> InboxView {
    let mut layout = inbox.layout(Utc::now(), query);
    let counts = Counts {
        in_progress: layout.in_progress.len(),
        my_turn: layout.my_turn_items().count(),
        waiting: layout.waiting_count(),
        snoozed: layout.snoozed.len(),
        done: layout.done.len(),
    };
    let images = image_hosts(inbox);
    let strip = |item: &mut InboxItem| {
        let allowed = images.get(&item.account_id);
        let keep = |url: &Option<String>| {
            url.as_deref().map(|u| remora_plugins::Request::get(u).host()).is_some_and(|host| allowed.is_some_and(|hosts| host_matches(&host, hosts)))
        };
        if let Some(author) = item.author.as_mut() {
            if !keep(&author.avatar_url) {
                author.avatar_url = None;
            }
        }
        for person in &mut item.participants {
            if !keep(&person.avatar_url) {
                person.avatar_url = None;
            }
        }
    };
    layout.in_progress.iter_mut().for_each(&strip);
    layout.pinned.iter_mut().for_each(&strip);
    layout.my_turn.iter_mut().chain(layout.waiting.iter_mut()).flat_map(|g| g.items.iter_mut()).for_each(&strip);
    layout.snoozed.iter_mut().chain(layout.done.iter_mut()).for_each(&strip);

    let mut per_plugin: HashMap<&str, usize> = HashMap::new();
    for account in &inbox.accounts {
        *per_plugin.entry(account.plugin_id.as_str()).or_default() += 1;
    }
    let account_names = inbox
        .accounts
        .iter()
        .filter(|a| per_plugin[a.plugin_id.as_str()] > 1)
        .map(|a| (a.id.clone(), a.name.clone().or_else(|| a.identity.clone()).unwrap_or_else(|| a.plugin_id.clone())))
        .collect();

    let health = inbox.health();
    let reconnect_names = match &health {
        remora_core::SourcesHealth::Reconnect { accounts } => accounts
            .iter()
            .filter_map(|id| inbox.accounts.iter().find(|a| &a.id == id))
            .map(|a| a.name.clone().or_else(|| registry::manifest(&a.plugin_id).map(|m| m.name)).unwrap_or_else(|| a.plugin_id.clone()))
            .collect(),
        _ => vec![],
    };

    let now = Utc::now();
    let shown = layout.in_progress.iter().chain(&layout.pinned).chain(layout.my_turn.iter().chain(&layout.waiting).flat_map(|g| &g.items))
        .chain(&layout.snoozed);
    let extras = shown.map(|item| (item.id.clone(), extras(inbox, item, now, tr))).collect();
    InboxView {
        extras,
        insights: inbox
            .insights(now)
            .into_iter()
            .map(|insight| {
                let nudge = match &insight {
                    SnoozeInsight::Avoidance { items, .. } => items.first().map(|item| remora_core::SnoozeAdvisor::new().nudge(item)),
                    _ => None,
                };
                InsightView { id: insight.id(), nudge, insight }
            })
            .collect(),
        review_pace: inbox.review_pace(),
        assistant: inbox.assistant_account().map(|account| AssistantView {
            name: account.name.clone().unwrap_or_else(|| {
                remora_plugins::claude::assistant_manifests().into_iter().find(|m| m.id == account.plugin_id).map(|m| m.name).unwrap_or_default()
            }),
            whole_inbox: inbox.preferences.whole_inbox_brief,
            brief: inbox.brief.clone(),
            brief_fresh: inbox.brief_is_fresh(now),
            summaries: inbox.bundle_summaries.iter().map(|(id, s)| (id.clone(), s.text.clone())).collect(),
            triage: inbox.triage.clone(),
        }),
        layout,
        counts,
        last_refresh: inbox.last_refresh,
        errors: inbox.errors(),
        health,
        reconnect_names,
        remarks: inbox.remarks(),
        connected: !inbox.accounts.is_empty(),
        demo,
        account_names,
        states: inbox.states.clone(),
    }
}

/// Avatars load only when the policy allows remote images, and only from the account's own hosts.
fn image_hosts(inbox: &Inbox) -> HashMap<String, Vec<String>> {
    if !inbox.policy().allow_remote_images {
        return HashMap::new();
    }
    inbox
        .accounts
        .iter()
        .filter_map(|a| {
            let mut hosts = registry::allowed_hosts(a, &registry::manifest(&a.plugin_id)?);
            hosts.extend(IMAGE_ONLY_HOSTS.iter().filter(|(plugin, _)| *plugin == a.plugin_id).map(|(_, host)| host.to_string()));
            Some((a.id.clone(), hosts))
        })
        .collect()
}

/// Hosts a tool's avatars come from without the tool ever being asked anything there: GitLab serves most
/// avatars from Gravatar. Images only, and only while the policy allows them; never a request with a token.
const IMAGE_ONLY_HOSTS: &[(&str, &str)] = &[("gitlab", "secure.gravatar.com"), ("gitlab", "www.gravatar.com")];

/// Whether an avatar may be shown, and so fetched: an https URL on one of an account's hosts, while the policy
/// allows images. The window's avatars come only through this check.
pub fn image_allowed(inbox: &Inbox, url: &str) -> bool {
    if !url.starts_with("https://") {
        return false;
    }
    let host = remora_plugins::Request::get(url).host();
    image_hosts(inbox).values().any(|hosts| host_matches(&host, hosts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Managed, MemoryVault, Preferences};
    use remora_core::Account;
    use std::sync::Arc;

    fn inbox(plugin: &str, settings: &[(&str, &str)]) -> Inbox {
        let mut inbox = Inbox::in_memory(Arc::new(MemoryVault::default()), Managed::default());
        let settings = settings.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        inbox.accounts = vec![Account { id: "a1".into(), plugin_id: plugin.into(), name: None, settings, identity: None }];
        inbox
    }

    /// The window's avatars come only from an account's hosts (Gravatar for GitLab), while images are on.
    #[test]
    fn avatars_come_only_from_allowed_hosts() {
        let gitlab = inbox("gitlab", &[("host", "https://gitlab.acme.io")]);
        assert!(image_allowed(&gitlab, "https://gitlab.acme.io/uploads/-/system/user/avatar/1/a.png"));
        assert!(image_allowed(&gitlab, "https://secure.gravatar.com/avatar/abc?s=80"), "GitLab's Gravatar avatars");
        assert!(!image_allowed(&gitlab, "https://evil.example.com/a.png"));
        assert!(!image_allowed(&gitlab, "http://gitlab.acme.io/a.png"), "https only");

        let github = inbox("github", &[("host", "https://github.com")]);
        assert!(image_allowed(&github, "https://avatars.githubusercontent.com/u/1?v=4"));
        assert!(!image_allowed(&github, "https://secure.gravatar.com/avatar/abc"), "Gravatar is GitLab's only");

        let mut off = gitlab;
        off.preferences = Preferences { allow_remote_images: false, ..Preferences::default() };
        assert!(!image_allowed(&off, "https://gitlab.acme.io/a.png"), "nothing while images are off");
    }
}
