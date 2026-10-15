use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum TrayCount {
    /// What needs you (My turn, without quiet items).
    WaitingOnMe,
    Everything,
    Hidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    System,
    Light,
    Dark,
}

/// The text size, as on the Mac (A11Y-25): the window's content is zoomed by `scale`, and the window grows with it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum TextSize {
    #[default]
    Standard,
    Large,
    Larger,
}

impl TextSize {
    pub fn scale(self) -> f64 {
        match self {
            TextSize::Standard => 1.0,
            TextSize::Large => 1.15,
            TextSize::Larger => 1.3,
        }
    }
}

/// The user's settings, as in the macOS app. Missing keys keep their defaults, so adding one never
/// resets the others.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
    pub refresh_minutes: u32,
    pub tray_count: TrayCount,
    /// While a task is in progress, the tray shows only it.
    pub focus_while_in_progress: bool,
    pub notify_arrivals: bool,
    pub notify_status_changes: bool,
    /// What arrives and changes waits for the next of `notification_times`, then comes grouped (IDEA-26).
    pub batch_notifications: bool,
    /// "10:00, 14:00, 17:00", as typed: `remora_core::parse_times` reads it.
    pub notification_times: String,
    /// Plugins whose notifications only say where something happened, not what was written.
    pub hidden_content_plugins: Vec<String>,
    /// Bring snoozed items back early on new activity.
    pub wake_on_activity: bool,
    pub appearance: Appearance,
    pub text_size: TextSize,
    pub open_in_apps: bool,
    /// Privacy: plugins allowed to connect (None = all), avatars. External AI has no plugin yet on Windows.
    pub allowed_plugins: Option<Vec<String>>,
    pub allow_external_ai: bool,
    pub allow_remote_images: bool,
    /// Updates: off until the user turns them on; nothing calls home before.
    pub check_for_updates: bool,
    /// The assistant: a brief of the whole inbox, how long a brief or summary stays fresh, and the sources
    /// whose items never reach it (plugin ids, or "reminders").
    pub whole_inbox_brief: bool,
    pub brief_cache_minutes: i64,
    pub assistant_excluded_sources: Vec<String>,
    /// Assistant requests a day; 0 is no limit (AI-03).
    pub assistant_daily_limit: u32,
    /// The first-run tip (the tray, the reminder shortcut, start with Windows) was dismissed.
    pub welcomed: bool,
}

impl Preferences {
    /// Whether notifications from this source hide what was written: the user's choice or the organization's (`*`:
    /// every source).
    pub fn hides_content(&self, plugin_id: &str) -> bool {
        self.hidden_content_plugins.iter().any(|p| p == plugin_id || p == "*")
    }
}

impl Default for Preferences {
    fn default() -> Self {
        Preferences {
            refresh_minutes: 5,
            tray_count: TrayCount::WaitingOnMe,
            focus_while_in_progress: true,
            notify_arrivals: true,
            notify_status_changes: true,
            batch_notifications: false,
            notification_times: remora_core::DEFAULT_NOTIFICATION_TIMES.into(),
            hidden_content_plugins: vec![],
            wake_on_activity: true,
            appearance: Appearance::System,
            text_size: TextSize::Standard,
            open_in_apps: true,
            allowed_plugins: None,
            allow_external_ai: false,
            allow_remote_images: true,
            check_for_updates: false,
            whole_inbox_brief: true,
            brief_cache_minutes: 30,
            assistant_excluded_sources: vec![],
            assistant_daily_limit: 0,
            welcomed: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_keys_keep_their_defaults() {
        let prefs: Preferences = serde_json::from_str(r#"{"refreshMinutes": 15}"#).unwrap();
        assert_eq!(prefs.refresh_minutes, 15);
        assert!(prefs.notify_arrivals && !prefs.allow_external_ai);
        assert!(!prefs.check_for_updates, "updates are opt-in");
    }

    #[test]
    fn the_text_size_is_kept_and_defaults_to_standard() {
        let old: Preferences = serde_json::from_str(r#"{"appearance": "dark"}"#).unwrap();
        assert_eq!(old.text_size, TextSize::Standard, "a file from before keeps the default");
        let larger: Preferences = serde_json::from_str(r#"{"textSize": "larger"}"#).unwrap();
        assert_eq!((larger.text_size, larger.text_size.scale()), (TextSize::Larger, 1.3));
        assert!(serde_json::to_string(&larger).unwrap().contains(r#""textSize":"larger""#));
    }
}
