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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_gdbus_answers() {
        assert!(has_owner("(true,)\n"));
        assert!(!has_owner("(false,)\n"));
    }
}
