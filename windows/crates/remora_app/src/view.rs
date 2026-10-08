use chrono::{DateTime, Utc};
use crate::Inbox;
use remora_core::{host_matches, InboxItem, InboxLayout, ItemState};
use remora_plugins::registry;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
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
    pub connected: bool,
    pub demo: bool,
    /// Account names to show next to items, only when a tool has several accounts.
    pub account_names: HashMap<String, String>,
    /// Pinned, snoozed until, reminded: what the user decided about each item.
    pub states: HashMap<String, ItemState>,
}

/// The inbox screen's data. Avatars are kept only from allowed hosts, when the policy allows images.
pub fn inbox_view(inbox: &Inbox, query: &str, demo: bool) -> InboxView {
    let mut layout = inbox.layout(Utc::now(), query);
    let counts = Counts {
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

    InboxView {
        layout,
        counts,
        last_refresh: inbox.last_refresh,
        errors: inbox.errors(),
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
        .filter_map(|a| Some((a.id.clone(), registry::allowed_hosts(a, &registry::manifest(&a.plugin_id)?))))
        .collect()
}
