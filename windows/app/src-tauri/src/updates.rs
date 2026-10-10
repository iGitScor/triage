//! Updates, opt-in: nothing calls home until the user, or the organization's `AutomaticUpdates` policy,
//! turns them on, except a Check now the user asks for. The release's `latest.json` and installer come from GitHub;
//! Tauri's updater installs a download only if it carries the release key's signature for that exact version
//! (`requireSignedVersion`), and the installer itself is Authenticode-signed when SignPath is set up.

use crate::AppState;
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::{Mutex, Notify};

const DAY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available {
        version: String,
        notes: Option<String>,
    },
    Installing {
        version: String,
    },
    Failed {
        message: String,
    },
    /// The organization turned updating off.
    Managed,
    /// A build without the release key (development): it can't check a download, so it doesn't offer updates.
    Unsupported,
}

#[derive(Default)]
pub struct Updates {
    status: Mutex<UpdateStatus>,
    pending: Mutex<Option<Update>>,
    /// Wakes the daily loop: the user just turned automatic checks on.
    pub now: Notify,
}

/// The release key is in tauri.conf.json; a build without one can't verify an update.
fn supported(app: &AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|updater| updater.get("pubkey"))
        .and_then(|key| key.as_str())
        .is_some_and(|key| !key.trim().is_empty())
}

async fn set(app: &AppHandle, status: UpdateStatus) {
    *app.state::<AppState>().updates.status.lock().await = status.clone();
    let _ = app.emit("updates-changed", status);
    crate::update_tray(app).await;
}

/// What Settings shows.
pub async fn status(app: &AppHandle) -> UpdateStatus {
    if !supported(app) {
        return UpdateStatus::Unsupported;
    }
    if !app.state::<AppState>().inbox.lock().await.managed.updates_allowed() {
        return UpdateStatus::Managed;
    }
    app.state::<AppState>().updates.status.lock().await.clone()
}

/// The version on offer, for the tray menu.
pub async fn offered(app: &AppHandle) -> Option<String> {
    match &*app.state::<AppState>().updates.status.lock().await {
        UpdateStatus::Available { version, .. } => Some(version.clone()),
        _ => None,
    }
}

/// One request to `latest.json`. Does nothing when updates are off by policy, in a build without the key, or while
/// a check or an install is running.
pub async fn check(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.demo || !supported(app) || !state.inbox.lock().await.managed.updates_allowed() {
        return;
    }
    if matches!(*state.updates.status.lock().await, UpdateStatus::Checking | UpdateStatus::Installing { .. }) {
        return;
    }
    set(app, UpdateStatus::Checking).await;
    let result = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(error) => Err(error),
    };
    let status = match result {
        Ok(Some(update)) => {
            let status =
                UpdateStatus::Available { version: update.version.clone(), notes: release_page(&update.version) };
            *state.updates.pending.lock().await = Some(update);
            status
        }
        Ok(None) => UpdateStatus::UpToDate,
        Err(error) => UpdateStatus::Failed { message: error.to_string() },
    };
    set(app, status).await;
}

/// The release's page on GitHub, never a link taken from the manifest.
fn release_page(version: &str) -> Option<String> {
    let version = version.trim_start_matches('v');
    version
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.')
        .then(|| format!("https://github.com/iGitScor/triage/releases/tag/v{version}"))
}

/// Downloads and runs the installer: Windows closes Remora while it installs, and the installer opens it again.
pub async fn install(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Some(update) = state.updates.pending.lock().await.take() else { return };
    let version = update.version.clone();
    set(app, UpdateStatus::Installing { version: version.clone() }).await;
    if let Err(error) = update.download_and_install(|_, _| {}, || {}).await {
        set(app, UpdateStatus::Failed { message: error.to_string() }).await;
        return;
    }
    // Reached outside Windows, where the installer doesn't close the app.
    app.restart();
}

/// Checks once a day while automatic checks are on, and right away when they are turned on.
pub async fn daily(app: AppHandle) {
    loop {
        let automatic = {
            let state = app.state::<AppState>();
            let inbox = state.inbox.lock().await;
            inbox.managed.checks_for_updates(&inbox.preferences)
        };
        if automatic {
            check(&app).await;
        }
        let state = app.state::<AppState>();
        tokio::select! {
            _ = state.updates.now.notified() => {}
            _ = tokio::time::sleep(DAY) => {}
        }
    }
}

/// Erase: forget what was found.
pub async fn forget(app: &AppHandle) {
    *app.state::<AppState>().updates.pending.lock().await = None;
    set(app, UpdateStatus::Idle).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_link_only_to_the_release_page() {
        assert_eq!(release_page("0.4.0").as_deref(), Some("https://github.com/iGitScor/triage/releases/tag/v0.4.0"));
        assert_eq!(release_page("v0.4.0").as_deref(), Some("https://github.com/iGitScor/triage/releases/tag/v0.4.0"));
        assert_eq!(release_page("0.4.0/../../evil"), None);
    }
}
