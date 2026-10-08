//! What the interface may ask. Each command locks the inbox briefly; network calls run without the lock.

use crate::{changed, AppState, REMINDER_SHORTCUT};
use chrono::{DateTime, Duration, Utc};
use remora_app::view::{inbox_view, InboxView};
use remora_app::{paths, AccountInfo, Managed, Preferences, SourceInfo};
use remora_core::{CompliancePolicy, Preset, SnoozeClock, SnoozeMode, SnoozeReason};
use remora_plugins::registry;
use serde::Serialize;
use std::collections::HashMap;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_opener::OpenerExt;

type Result<T> = std::result::Result<T, String>;

#[tauri::command]
pub async fn inbox(state: State<'_, AppState>, query: String) -> Result<InboxView> {
    Ok(inbox_view(&*state.inbox.lock().await, &query, state.demo))
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
    if title.trim().is_empty() {
        return Err("Type what to remember.".into());
    }
    state.inbox.lock().await.add_reminder(&title, at, Utc::now());
    changed(&app).await;
    Ok(())
}

/// The desktop app when it handles the link (slack://, linear://) and the preference allows it, else the
/// browser. `in_browser` forces the web page.
#[tauri::command]
pub async fn open_item(app: AppHandle, state: State<'_, AppState>, id: String, in_browser: bool) -> Result<()> {
    let (item, open_in_apps) = {
        let mut inbox = state.inbox.lock().await;
        let item = inbox.item(&id).ok_or("This item is gone.")?;
        inbox.opened(&id);
        (item, inbox.preferences.open_in_apps)
    };
    let opener = app.opener();
    let in_app = !in_browser && open_in_apps && item.app_url.as_deref().is_some_and(|url| opener.open_url(url, None::<&str>).is_ok());
    if !in_app {
        if let Some(url) = &item.url {
            opener.open_url(url, None::<&str>).map_err(|e| e.to_string())?;
        }
    }
    changed(&app).await;
    Ok(())
}

/// Opens a tool's token page (only the URLs the plugins declare).
#[tauri::command]
pub fn open_setup(app: AppHandle, plugin_id: String) -> Result<()> {
    let url = registry::manifest(&plugin_id).and_then(|m| m.setup_url).ok_or("No setup page for this tool.")?;
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
    let (account, plugin) = state.inbox.lock().await.prepare_connect(&plugin_id, name, settings, &secrets, state.http.clone())?;
    let snapshot = plugin.fetch().await.map_err(|e| e.to_string())?;
    state.inbox.lock().await.finish_connect(account, secrets, snapshot, Utc::now())?;
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
    let interval_changed = {
        let mut inbox = state.inbox.lock().await;
        let changed = inbox.preferences.refresh_minutes != preferences.refresh_minutes;
        inbox.set_preferences(preferences);
        changed
    };
    if interval_changed {
        state.refresh_now.notify_one();
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
    changed(&app).await;
    Ok(())
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
