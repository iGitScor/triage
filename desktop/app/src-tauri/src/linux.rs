//! What the Linux desktop offers: a tray, global shortcuts. Asked through the session's own tools
//! (`gdbus`), so no D-Bus library is linked; when a tool is missing, the answer is the common case.

use std::process::Command;

/// Whether something shows tray icons: a StatusNotifierWatcher on the session bus (KDE, Cinnamon, XFCE, GNOME with
/// the AppIndicator extension). Stock GNOME has none, and Remora would be invisible.
pub fn tray_host() -> bool {
    if !cfg!(target_os = "linux") {
        return true;
    }
    let asked = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.freedesktop.DBus",
            "--object-path",
            "/org/freedesktop/DBus",
            "--method",
            "org.freedesktop.DBus.NameHasOwner",
            "org.kde.StatusNotifierWatcher",
        ])
        .output();
    match asked {
        Ok(output) if output.status.success() => has_owner(&String::from_utf8_lossy(&output.stdout)),
        // No gdbus to ask: most desktops have a tray.
        _ => true,
    }
}

/// `(true,)`, gdbus's answer to NameHasOwner.
fn has_owner(answer: &str) -> bool {
    answer.trim().trim_start_matches('(').starts_with("true")
}

/// A Wayland session, where an app can't grab a key for the whole desktop.
pub fn wayland() -> bool {
    cfg!(target_os = "linux")
        && (std::env::var_os("WAYLAND_DISPLAY").is_some()
            || std::env::var("XDG_SESSION_TYPE").is_ok_and(|kind| kind == "wayland"))
}

/// Ctrl+Alt+R from anywhere on Wayland, through the XDG GlobalShortcuts portal (KDE Plasma, GNOME 48 and later):
/// the desktop asks the user once, and may let them pick another key. Returns what it bound ("Ctrl+Alt+R", or the
/// user's choice), or None where there is no portal or the user said no; `pressed` runs at each press, for as long
/// as the app runs.
#[cfg(target_os = "linux")]
pub async fn portal_shortcut(description: String, pressed: impl Fn() + Send + 'static) -> Option<String> {
    use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
    use futures_util::StreamExt;

    const ID: &str = "new-reminder";
    let shortcuts = GlobalShortcuts::new().await.ok()?;
    let session = shortcuts.create_session(Default::default()).await.ok()?;
    // What the desktop's dialog says the shortcut does, in the user's language.
    let wanted = NewShortcut::new(ID, description).preferred_trigger("<Control><Alt>r");
    let bound = shortcuts.bind_shortcuts(&session, &[wanted], None, Default::default()).await.ok()?.response().ok()?;
    let trigger = bound
        .shortcuts()
        .iter()
        .find(|s| s.id() == ID)
        .map(|s| s.trigger_description().to_string())
        .filter(|t| !t.is_empty())
        // The key itself, not a sentence to translate.
        .unwrap_or_else(|| String::from("Ctrl+Alt+R"));
    let mut presses = shortcuts.receive_activated().await.ok()?;
    tauri::async_runtime::spawn(async move {
        // The portal and its session live as long as this task: the shortcut stays bound while the app runs.
        let _keep = (&shortcuts, &session);
        while let Some(press) = presses.next().await {
            if press.shortcut_id() == ID {
                pressed();
            }
        }
    });
    Some(trigger)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_gdbus_answers() {
        assert!(has_owner("(true,)\n"));
        assert!(!has_owner("(false,)\n"));
    }
}
