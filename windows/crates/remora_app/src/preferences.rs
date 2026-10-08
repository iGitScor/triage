use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrayCount {
    /// What needs you (My turn, without quiet items).
    WaitingOnMe,
    Everything,
    Hidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    System,
    Light,
    Dark,
}

/// The user's settings, as in the macOS app. Missing keys keep their defaults, so adding one never
/// resets the others.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
    pub refresh_minutes: u32,
    pub tray_count: TrayCount,
    pub notify_arrivals: bool,
    pub notify_status_changes: bool,
    /// Bring snoozed items back early on new activity.
    pub wake_on_activity: bool,
    pub appearance: Appearance,
    pub open_in_apps: bool,
    /// Privacy: plugins allowed to connect (None = all), avatars. External AI has no plugin yet on Windows.
    pub allowed_plugins: Option<Vec<String>>,
    pub allow_external_ai: bool,
    pub allow_remote_images: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Preferences {
            refresh_minutes: 5,
            tray_count: TrayCount::WaitingOnMe,
            notify_arrivals: true,
            notify_status_changes: true,
            wake_on_activity: true,
            appearance: Appearance::System,
            open_in_apps: true,
            allowed_plugins: None,
            allow_external_ai: false,
            allow_remote_images: true,
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
    }
}
