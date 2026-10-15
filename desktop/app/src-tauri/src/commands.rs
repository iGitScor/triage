//! What the interface may ask. Each command locks the inbox briefly; network calls run without the lock.

use crate::{changed, AppState, REMINDER_SHORTCUT};
use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use remora_app::view::{inbox_view, InboxView};
use remora_app::{paths, AccountInfo, Managed, Preferences, SourceInfo};
use remora_core::{CompliancePolicy, Message, Preset, RecurrenceRule, SnoozeClock, SnoozeMode, SnoozeReason, UsageDay};
use remora_plugins::assistant::AssistantMeter;
use remora_plugins::{links, oauth, registry, PluginError};
use serde::Serialize;
use std::collections::HashMap;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_opener::OpenerExt;
use ts_rs::TS;

/// Errors reach the interface as a code and its values, translated there by code (`MESSAGES` in i18n.ts).
type Result<T> = std::result::Result<T, Message>;

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

/// The next occurrence when the item is a repeating reminder.
#[tauri::command]
pub async fn toggle_done(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<Option<DateTime<Utc>>> {
    let next = state.inbox.lock().await.toggle_done(&id, Utc::now());
    changed(&app).await;
    Ok(next)
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

/// `until` is the last day (a date in this computer's time zone).
#[tauri::command]
pub async fn add_reminder(
    app: AppHandle,
    state: State<'_, AppState>,
    title: String,
    at: DateTime<Utc>,
    repeat: Option<RecurrenceRule>,
    until: Option<NaiveDate>,
) -> Result<()> {
    let until = until.and_then(|day| Local.from_local_datetime(&day.and_time(chrono::NaiveTime::MIN)).earliest());
    let until = until.map(|date| date.with_timezone(&Utc));
    state.inbox.lock().await.add_reminder(&title, at, repeat, until, Utc::now())?;
    changed(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn stop_repeating(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<()> {
    state.inbox.lock().await.stop_repeating(&id);
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
            opener.open_url(url, None::<&str>).map_err(|e| Message::from(e.to_string()))?;
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
    app.opener().open_url(url, None::<&str>).map_err(|e| Message::from(e.to_string()))
}

/// Windows' notification settings, to turn Remora's back on.
#[tauri::command]
pub fn open_notification_settings(app: AppHandle) -> Result<()> {
    app.opener().open_url("ms-settings:notifications", None::<&str>).map_err(|e| Message::from(e.to_string()))
}

/// Settings → Updates: the licences of what the app bundles (DEP-07), shipped next to it.
#[tauri::command]
pub fn open_notices(app: AppHandle) -> Result<()> {
    use tauri::Manager;
    let path = app.path().resource_dir().map_err(|e| Message::from(e.to_string()))?.join("THIRD-PARTY-NOTICES.txt");
    app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(|e| Message::from(e.to_string()))
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
            (account, plugin, inbox.preferences().whole_inbox_brief, inbox.brief_items(Utc::now()))
        };
        let brief = if whole_inbox {
            Some(plugin.brief(&items, Utc::now()).await?)
        } else {
            plugin.digest(&[remora_app::Inbox::connection_test(Utc::now())], "Test", Utc::now()).await?;
            None
        };
        state.tokens.connect_assistant(&state.inbox, account, secrets, brief).await?;
        changed(&app).await;
        return Ok(());
    }
    let (account, plugin) =
        state.inbox.lock().await.prepare_connect(&plugin_id, name, settings, &secrets, state.http.clone())?;
    let snapshot = plugin.fetch().await?;
    state.tokens.connect(&state.inbox, account, secrets, snapshot, Utc::now()).await?;
    changed(&app).await;
    Ok(())
}

/// Reconnect: the same account with a new token, checked with one fetch first (the assistant: with one
/// made-up item, so no inbox content leaves).
#[tauri::command]
pub async fn reconnect(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: String,
    secrets: HashMap<String, String>,
) -> Result<()> {
    let assistant = {
        let inbox = state.inbox.lock().await;
        let is_assistant =
            inbox.accounts().iter().any(|a| a.id == account_id && remora_plugins::claude::is_assistant(&a.plugin_id));
        if is_assistant {
            let language = crate::i18n::translator().lang();
            Some(inbox.prepare_assistant_reconnect(&account_id, &secrets, state.http.clone(), language)?)
        } else {
            None
        }
    };
    if let Some(plugin) = assistant {
        plugin.digest(&[remora_app::Inbox::connection_test(Utc::now())], "Test", Utc::now()).await?;
        state.tokens.reconnect(&state.inbox, &account_id, secrets, None, Utc::now()).await?;
        changed(&app).await;
        return Ok(());
    }
    let plugin = state.inbox.lock().await.prepare_reconnect(&account_id, &secrets, state.http.clone())?;
    let snapshot = plugin.fetch().await?;
    state.tokens.reconnect(&state.inbox, &account_id, secrets, Some(snapshot), Utc::now()).await?;
    changed(&app).await;
    Ok(())
}

/// How long the browser has to come back.
const SIGN_IN_WAIT: std::time::Duration = std::time::Duration::from_secs(300);

/// Signs in through the browser and returns the secrets for `connect` or `reconnect` (with `account_id`, that
/// account's settings are used). Only the plugin's declared sign-in page opens.
#[tauri::command]
pub async fn sign_in(
    app: AppHandle,
    state: State<'_, AppState>,
    plugin_id: String,
    account_id: Option<String>,
    settings: HashMap<String, String>,
) -> Result<HashMap<String, String>> {
    let remora_app::SignInPlan { sign_in, settings: values, http } =
        state.inbox.lock().await.prepare_sign_in(&plugin_id, account_id.as_deref(), settings, state.http.clone())?;
    let loopback = oauth::Loopback::bind().await?;
    let redirect = loopback.redirect.clone();
    let pkce = oauth::Pkce::new();
    let expected = oauth::random(16);
    let page = oauth::authorize_url(&sign_in, &values, &redirect, &pkce, &expected)?;
    // A sign-in started before this one stops waiting.
    state.cancel_sign_in.notify_waiters();
    let cancelled = state.cancel_sign_in.notified();
    app.opener().open_url(page, None::<&str>).map_err(|e| Message::from(e.to_string()))?;
    let code = tokio::select! {
        code = loopback.code(&expected) => code?,
        _ = tokio::time::sleep(SIGN_IN_WAIT) => return Err(PluginError::SignIn("timed out".into()).into()),
        _ = cancelled => return Err(PluginError::SignIn("cancelled".into()).into()),
    };
    let tokens = oauth::redeem(http.as_ref(), &sign_in, &values, &code, &redirect, &pkce.verifier).await?;
    let refresh = tokens.refresh_token.ok_or_else(|| PluginError::SignIn("no refresh token".into()))?;
    // `signIn` changes with every sign-in, so the account starts over with it (`RENEWED_SECRETS`).
    Ok(HashMap::from([("refreshToken".into(), refresh), ("signIn".into(), oauth::random(8))]))
}

#[tauri::command]
pub fn cancel_sign_in(state: State<'_, AppState>) {
    state.cancel_sign_in.notify_waiters();
}

#[tauri::command]
pub async fn rename_account(app: AppHandle, state: State<'_, AppState>, id: String, name: String) -> Result<()> {
    state.inbox.lock().await.rename(&id, &name);
    changed(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn disconnect(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<()> {
    state.tokens.disconnect(&state.inbox, &id).await?;
    changed(&app).await;
    Ok(())
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename = "Settings")]
pub struct SettingsView {
    preferences: Preferences,
    managed: Managed,
    is_managed: bool,
    policy: CompliancePolicy,
    autostart: bool,
    version: &'static str,
    data_dir: String,
    /// The new-reminder key: Ctrl+Alt+R, or what a Wayland desktop bound through its portal.
    shortcut: String,
    shortcut_registered: bool,
    /// A Wayland session: only the desktop's portal can bind a shortcut everywhere.
    wayland: bool,
    /// Windows won't show Remora's notifications: `app` (off for Remora), `all` (off for every app) or `policy`.
    #[ts(type = "'app' | 'all' | 'policy' | null")]
    notifications_off: Option<&'static str>,
    /// What the assistant used today and this month (AI-03).
    usage_today: UsageDay,
    usage_month: UsageDay,
}

#[tauri::command]
pub async fn settings(app: AppHandle, state: State<'_, AppState>) -> Result<SettingsView> {
    let inbox = state.inbox.lock().await;
    let (usage_today, usage_month) = inbox.assistant_usage(Utc::now());
    Ok(SettingsView {
        preferences: inbox.preferences().clone(),
        managed: inbox.managed().clone(),
        is_managed: inbox.managed().is_managed(),
        policy: inbox.policy(),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        version: env!("CARGO_PKG_VERSION"),
        data_dir: paths::data_dir().display().to_string(),
        shortcut: crate::PORTAL_SHORTCUT
            .lock()
            .ok()
            .and_then(|bound| bound.clone())
            .unwrap_or_else(|| REMINDER_SHORTCUT.into()),
        shortcut_registered: crate::SHORTCUT_REGISTERED.load(std::sync::atomic::Ordering::Relaxed),
        wayland: crate::linux::wayland(),
        notifications_off: crate::notifications_off(&app),
        usage_today,
        usage_month,
    })
}

#[tauri::command]
pub async fn set_preferences(app: AppHandle, state: State<'_, AppState>, preferences: Preferences) -> Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        crate::set_text_scale(&window, preferences.text_size.scale());
    }
    let (interval_changed, updates_turned_on) = {
        let mut inbox = state.inbox.lock().await;
        let was = inbox.checks_for_updates();
        let interval_changed = inbox.set_preferences(preferences);
        (interval_changed, !was && inbox.checks_for_updates())
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
    if enabled { launcher.enable() } else { launcher.disable() }.map_err(|e| Message::from(e.to_string()))
}

#[tauri::command]
pub async fn erase_local_data(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    state.tokens.erase(&state.inbox).await?;
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

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SnoozeAdvice {
    /// When to come back for each reason, from your habits.
    #[ts(type = "Record<SnoozeReason, string>")]
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
    until_news: bool,
) -> Result<()> {
    state.inbox.lock().await.snooze_many(&ids, until, reason, until_news);
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

/// One request to the assistant, counted with the tokens it reports whether it answers or fails (AI-03).
async fn measured<T>(
    state: &AppState,
    request: impl std::future::Future<Output = std::result::Result<T, PluginError>>,
) -> std::result::Result<T, PluginError> {
    let meter = AssistantMeter::default();
    let result = meter.measure(request).await;
    state.inbox.lock().await.record_usage(meter.tokens(), Utc::now());
    result
}

/// A brief of the whole inbox. Skipped while the last one is fresh, unless asked for (`force`).
#[tauri::command]
pub async fn make_brief(app: AppHandle, state: State<'_, AppState>, force: bool) -> Result<()> {
    let language = crate::i18n::translator().lang();
    // The assistant's token, read outside the lock if the first refresh hasn't yet; a failed read is its error.
    let _ = state.tokens.load_once(&state.inbox).await;
    let (from, plugin, items) = {
        let mut inbox = state.inbox.lock().await;
        if !inbox.preferences().whole_inbox_brief || (!force && inbox.brief_is_fresh(Utc::now())) {
            return Ok(());
        }
        let Some((from, plugin)) = inbox.assistant(state.http.clone(), language) else { return Ok(()) };
        (from, plugin?, inbox.brief_items(Utc::now()))
    };
    let brief = measured(&state, plugin.brief(&items, Utc::now())).await?;
    state.inbox.lock().await.keep_brief(&from, brief.written_from(&items));
    changed(&app).await;
    Ok(())
}

/// A short summary of one group, kept while its items don't change.
#[tauri::command]
pub async fn summarize(app: AppHandle, state: State<'_, AppState>, bundle_id: String, topic: String) -> Result<()> {
    let language = crate::i18n::translator().lang();
    // The assistant's token, read outside the lock if the first refresh hasn't yet; a failed read is its error.
    let _ = state.tokens.load_once(&state.inbox).await;
    let (from, plugin, items) = {
        let mut inbox = state.inbox.lock().await;
        let (items, fresh) = inbox.bundle_items(&bundle_id, Utc::now());
        if items.is_empty() || fresh {
            return Ok(());
        }
        let Some((from, plugin)) = inbox.assistant(state.http.clone(), language) else { return Ok(()) };
        (from, plugin?, items)
    };
    let text = measured(&state, plugin.digest(&items, &topic, Utc::now())).await?;
    state.inbox.lock().await.keep_summary(&from, &bundle_id, remora_core::BundleSummary::new(text, &items, Utc::now()));
    changed(&app).await;
    Ok(())
}

/// What to do with each snoozed item, as Claude sees it: nothing is applied until ticked.
#[tauri::command]
pub async fn triage(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    let language = crate::i18n::translator().lang();
    // The assistant's token, read outside the lock if the first refresh hasn't yet; a failed read is its error.
    let _ = state.tokens.load_once(&state.inbox).await;
    let (from, plugin, snoozed) = {
        let mut inbox = state.inbox.lock().await;
        let snoozed = inbox.snoozed_for_triage(Utc::now());
        if snoozed.is_empty() {
            return Ok(());
        }
        let Some((from, plugin)) = inbox.assistant(state.http.clone(), language) else { return Ok(()) };
        (from, plugin?, snoozed)
    };
    let suggestions = measured(&state, plugin.triage(&snoozed, Utc::now())).await?;
    state.inbox.lock().await.keep_triage(&from, suggestions);
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
    state.inbox.lock().await.discard_triage();
    changed(&app).await;
    Ok(())
}
