//! What the interface may ask. Each command locks the inbox briefly; network calls run without the lock.

use crate::{changed, AppState, REMINDER_SHORTCUT};
use chrono::{DateTime, Duration, Utc};
use remora_app::view::{inbox_view, InboxView};
use remora_app::{paths, AccountInfo, Managed, Preferences, SourceInfo};
use remora_core::{CompliancePolicy, Preset, SnoozeClock, SnoozeMode, SnoozeReason};
use remora_plugins::{links, registry};
use serde::Serialize;
use std::collections::HashMap;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_opener::OpenerExt;

type Result<T> = std::result::Result<T, String>;

#[tauri::command]
pub async fn inbox(state: State<'_, AppState>, query: String) -> Result<InboxView> {
    let translator = crate::i18n::translator();
    Ok(inbox_view(&*state.inbox.lock().await, &query, state.demo, &|key| translator.t(key)))
}

#[tauri::command]
pub async fn refresh(state: State<'_, AppState>) -> Result<()> {
    state.refresh_now.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn toggle_done(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<()> {
    state.inbox.lock().await.toggle_done(&id, Utc::now());
    changed(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn toggle_pin(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<()> {
    state.inbox.lock().await.toggle_pin(&id);
    changed(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn start(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<()> {
    state.inbox.lock().await.start(&id, Utc::now());
    changed(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn stop(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<()> {
    state.inbox.lock().await.stop(&id);
    changed(&app).await;
    Ok(())
}

/// Undoes the last Done or Clear all.
#[tauri::command]
pub async fn undo(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    if !state.inbox.lock().await.undo() {
        return Err("Nothing to undo.".into());
    }
    changed(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn clear_done(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    state.inbox.lock().await.clear_done(Utc::now());
    changed(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn snooze(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    until: DateTime<Utc>,
    mode: SnoozeMode,
    reason: Option<SnoozeReason>,
    until_news: bool,
) -> Result<()> {
    state.inbox.lock().await.snooze(&id, until, mode, reason, until_news);
    changed(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn unsnooze(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<()> {
    state.inbox.lock().await.unsnooze(&id);
    changed(&app).await;
    Ok(())
}

#[tauri::command]
pub fn presets() -> Vec<Preset> {
    SnoozeClock::local_presets()
}

/// The slider counts from `from` (a preset picked first) or from now: minutes, then hours, then days.
#[tauri::command]
pub fn slider_date(progress: f64, from: Option<DateTime<Utc>>) -> DateTime<Utc> {
    from.unwrap_or_else(Utc::now) + Duration::seconds(SnoozeClock::duration_at(progress))
}

#[tauri::command]
pub fn slider_progress(date: DateTime<Utc>, from: Option<DateTime<Utc>>) -> f64 {
    SnoozeClock::progress_for((date - from.unwrap_or_else(Utc::now)).num_seconds())
}

#[tauri::command]
pub async fn add_reminder(app: AppHandle, state: State<'_, AppState>, title: String, at: DateTime<Utc>) -> Result<()> {
    state.inbox.lock().await.add_reminder(&title, at, Utc::now())?;
    changed(&app).await;
    Ok(())
}

/// The desktop app when it handles the link (slack://, linear://) and the preference allows it, else the
/// browser. `in_browser` forces the web page.
#[tauri::command]
pub async fn open_item(app: AppHandle, id: String, in_browser: bool) -> Result<()> {
    open(&app, &id, in_browser).await
}

/// Opens an item as a click in the inbox does: from the inbox, and from a notification. What to open is
/// `Inbox::open_plan`'s choice; this only opens it.
pub async fn open(app: &AppHandle, id: &str, in_browser: bool) -> Result<()> {
    let plan = app.state::<AppState>().inbox.lock().await.open_plan(id, in_browser)?;
    let opener = app.opener();
    let in_app = plan.app.as_deref().is_some_and(|url| opener.open_url(url, None::<&str>).is_ok());
    if !in_app {
        if let Some(url) = plan.web.as_deref() {
            opener.open_url(url, None::<&str>).map_err(|e| e.to_string())?;
        }
    }
    changed(app).await;
    Ok(())
}

/// Opens a tool's token page (only the URLs the plugins declare), on the host typed in the form.
#[tauri::command]
pub fn open_setup(app: AppHandle, plugin_id: String, host: Option<String>) -> Result<()> {
    let manifest = registry::manifest(&plugin_id).ok_or("No setup page for this tool.")?;
    let url = links::setup_url(&manifest, host.as_deref()).ok_or("No setup page for this tool.")?;
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn sources(state: State<'_, AppState>) -> Result<Vec<SourceInfo>> {
    Ok(state.inbox.lock().await.sources())
}

#[tauri::command]
pub async fn accounts(state: State<'_, AppState>) -> Result<Vec<AccountInfo>> {
    Ok(state.inbox.lock().await.account_infos())
}

/// Checks the settings with one fetch, then saves the account and its secrets.
#[tauri::command]
pub async fn connect(
    app: AppHandle,
    state: State<'_, AppState>,
    plugin_id: String,
    name: Option<String>,
    settings: HashMap<String, String>,
    secrets: HashMap<String, String>,
) -> Result<()> {
    // An assistant: tested once, then kept; its first brief doubles as the test.
    if remora_plugins::claude::is_assistant(&plugin_id) {
        let language = crate::i18n::translator().lang();
        let (account, plugin, whole_inbox, items) = {
            let inbox = state.inbox.lock().await;
            let (account, plugin) =
                inbox.prepare_assistant_connect(&plugin_id, name, settings, &secrets, state.http.clone(), language)?;
            (account, plugin, inbox.preferences.whole_inbox_brief, inbox.brief_items(Utc::now()))
        };
        let brief = if whole_inbox {
            Some(plugin.brief(&items, Utc::now()).await.map_err(|e| e.to_string())?)
        } else {
            plugin
                .digest(&[remora_app::Inbox::connection_test(Utc::now())], "Test", Utc::now())
                .await
                .map_err(|e| e.to_string())?;
            None
        };
        let mut inbox = state.inbox.lock().await;
        inbox.finish_assistant_connect(account, secrets)?;
        if brief.is_some() {
            inbox.store_brief(brief);
        }
        drop(inbox);
        changed(&app).await;
        return Ok(());
    }
    let (account, plugin) =
        state.inbox.lock().await.prepare_connect(&plugin_id, name, settings, &secrets, state.http.clone())?;
    let snapshot = plugin.fetch().await.map_err(|e| e.to_string())?;
    state.inbox.lock().await.finish_connect(account, secrets, snapshot, Utc::now())?;
    changed(&app).await;
    Ok(())
}

/// Reconnect: the same account with a new token, checked with one fetch first.
#[tauri::command]
pub async fn reconnect(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: String,
    secrets: HashMap<String, String>,
) -> Result<()> {
    let plugin = state.inbox.lock().await.prepare_reconnect(&account_id, &secrets, state.http.clone())?;
    let snapshot = plugin.fetch().await.map_err(|e| e.to_string())?;
    state.inbox.lock().await.finish_reconnect(&account_id, secrets, snapshot, Utc::now())?;
    changed(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn rename_account(app: AppHandle, state: State<'_, AppState>, id: String, name: String) -> Result<()> {
    state.inbox.lock().await.rename(&id, &name);
    changed(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn disconnect(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<()> {
    state.inbox.lock().await.disconnect(&id)?;
    changed(&app).await;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    preferences: Preferences,
    managed: Managed,
    is_managed: bool,
    policy: CompliancePolicy,
    autostart: bool,
    version: &'static str,
    data_dir: String,
    shortcut: &'static str,
}

#[tauri::command]
pub async fn settings(app: AppHandle, state: State<'_, AppState>) -> Result<SettingsView> {
    let inbox = state.inbox.lock().await;
    Ok(SettingsView {
        preferences: inbox.preferences.clone(),
        managed: inbox.managed.clone(),
        is_managed: inbox.managed.is_managed(),
        policy: inbox.policy(),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        version: env!("CARGO_PKG_VERSION"),
        data_dir: paths::data_dir().display().to_string(),
        shortcut: REMINDER_SHORTCUT,
    })
}

#[tauri::command]
pub async fn set_preferences(app: AppHandle, state: State<'_, AppState>, preferences: Preferences) -> Result<()> {
    let (interval_changed, updates_turned_on) = {
        let mut inbox = state.inbox.lock().await;
        let was = inbox.managed.checks_for_updates(&inbox.preferences);
        let interval_changed = inbox.set_preferences(preferences);
        (interval_changed, !was && inbox.managed.checks_for_updates(&inbox.preferences))
    };
    if interval_changed {
        state.refresh_now.notify_one();
    }
    // Checks at once, then daily.
    if updates_turned_on {
        state.updates.now.notify_one();
    }
    changed(&app).await;
    Ok(())
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<()> {
    let launcher = app.autolaunch();
    if enabled { launcher.enable() } else { launcher.disable() }.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn erase_local_data(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    state.inbox.lock().await.erase_local_data()?;
    crate::updates::forget(&app).await;
    crate::avatars::forget();
    changed(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn update_status(app: AppHandle) -> crate::updates::UpdateStatus {
    crate::updates::status(&app).await
}

/// Check now: the user asked, so it runs even with automatic checks off, unless the organization forbids it.
#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> crate::updates::UpdateStatus {
    crate::updates::check(&app).await;
    crate::updates::status(&app).await
}

#[tauri::command]
pub async fn install_update(app: AppHandle) {
    crate::updates::install(&app).await;
}

#[tauri::command]
pub fn hide(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    app.exit(0);
}

// MARK: Snooze reasons and insights, review sessions, the waiting assistant

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnoozeAdvice {
    /// When to come back for each reason, from your habits.
    pub returns: HashMap<String, DateTime<Utc>>,
    /// The reason usually given in this repo or channel, preselected.
    pub usual: Option<SnoozeReason>,
    /// A small way in for "Not feeling it" (English key, translated by the interface).
    pub nudge: Option<&'static str>,
}

#[tauri::command]
pub async fn snooze_advice(state: State<'_, AppState>, id: String) -> Result<SnoozeAdvice> {
    let inbox = state.inbox.lock().await;
    let now = Utc::now();
    let returns = SnoozeReason::ALL
        .iter()
        .map(|reason| {
            (
                serde_json::to_value(reason).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(),
                inbox.suggested_return(*reason, now),
            )
        })
        .collect();
    let nudge = inbox.item(&id).map(|item| remora_core::SnoozeAdvisor::new().nudge(&item));
    Ok(SnoozeAdvice { returns, usual: inbox.usual_reason(&id), nudge })
}

#[tauri::command]
pub async fn dismiss_insight(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<()> {
    state.inbox.lock().await.dismiss_insight(&id);
    changed(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn spread(app: AppHandle, state: State<'_, AppState>, ids: Vec<String>, from: DateTime<Utc>) -> Result<()> {
    state.inbox.lock().await.spread(&ids, from);
    changed(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn align_returns(app: AppHandle, state: State<'_, AppState>, ids: Vec<String>) -> Result<()> {
    state.inbox.lock().await.align_returns(&ids);
    changed(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn snooze_many(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<String>,
    until: DateTime<Utc>,
    reason: Option<SnoozeReason>,
) -> Result<()> {
    state.inbox.lock().await.snooze_many(&ids, until, reason);
    changed(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn sweep(app: AppHandle, state: State<'_, AppState>, ids: Vec<String>) -> Result<()> {
    state.inbox.lock().await.sweep(&ids, Utc::now());
    changed(&app).await;
    Ok(())
}

/// The nudge or reviewer suggestion to paste, in the interface's language. Nothing is sent.
#[tauri::command]
pub async fn waiting_draft(state: State<'_, AppState>, id: String) -> Result<Option<String>> {
    let translator = crate::i18n::translator();
    Ok(state.inbox.lock().await.waiting_draft(&id, Utc::now(), &|key| translator.t(key)))
}

#[tauri::command]
pub async fn review_queue(state: State<'_, AppState>) -> Result<Vec<remora_core::InboxItem>> {
    Ok(state.inbox.lock().await.review_queue(Utc::now()))
}

// MARK: The assistant: brief, summaries, triage

/// A brief of the whole inbox. Skipped while the last one is fresh, unless asked for (`force`).
#[tauri::command]
pub async fn make_brief(app: AppHandle, state: State<'_, AppState>, force: bool) -> Result<()> {
    let language = crate::i18n::translator().lang();
    let (plugin, items) = {
        let mut inbox = state.inbox.lock().await;
        if !inbox.preferences.whole_inbox_brief || (!force && inbox.brief_is_fresh(Utc::now())) {
            return Ok(());
        }
        let Some(plugin) = inbox.assistant(state.http.clone(), language) else { return Ok(()) };
        (plugin?, inbox.brief_items(Utc::now()))
    };
    let brief = plugin.brief(&items, Utc::now()).await.map_err(|e| e.to_string())?;
    state.inbox.lock().await.store_brief(Some(brief));
    changed(&app).await;
    Ok(())
}

/// A short summary of one group, kept while its items don't change.
#[tauri::command]
pub async fn summarize(app: AppHandle, state: State<'_, AppState>, bundle_id: String, topic: String) -> Result<()> {
    let language = crate::i18n::translator().lang();
    let (plugin, items) = {
        let mut inbox = state.inbox.lock().await;
        let (items, fresh) = inbox.bundle_items(&bundle_id, Utc::now());
        if items.is_empty() || fresh {
            return Ok(());
        }
        let Some(plugin) = inbox.assistant(state.http.clone(), language) else { return Ok(()) };
        (plugin?, items)
    };
    let text = plugin.digest(&items, &topic, Utc::now()).await.map_err(|e| e.to_string())?;
    state.inbox.lock().await.store_summary(&bundle_id, remora_core::BundleSummary::new(text, &items, Utc::now()));
    changed(&app).await;
    Ok(())
}

/// What to do with each snoozed item, as Claude sees it: nothing is applied until ticked.
#[tauri::command]
pub async fn triage(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    let language = crate::i18n::translator().lang();
    let (plugin, snoozed) = {
        let mut inbox = state.inbox.lock().await;
        let snoozed = inbox.snoozed_for_triage(Utc::now());
        if snoozed.is_empty() {
            return Ok(());
        }
        let Some(plugin) = inbox.assistant(state.http.clone(), language) else { return Ok(()) };
        (plugin?, snoozed)
    };
    let suggestions = plugin.triage(&snoozed, Utc::now()).await.map_err(|e| e.to_string())?;
    state.inbox.lock().await.triage = suggestions;
    changed(&app).await;
    Ok(())
}

/// Applies the ticked suggestions; the first "Now" item opens.
#[tauri::command]
pub async fn apply_triage(
    app: AppHandle,
    state: State<'_, AppState>,
    selected: Vec<remora_core::TriageSuggestion>,
) -> Result<()> {
    let first_now = state.inbox.lock().await.apply_triage(&selected, Utc::now());
    if let Some(id) = first_now {
        let _ = open(&app, &id, false).await;
    }
    changed(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn discard_triage(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    state.inbox.lock().await.triage.clear();
    changed(&app).await;
    Ok(())
}
