use crate::Preferences;
use remora_core::CompliancePolicy;
use serde::Serialize;
use std::collections::HashSet;

/// The organization's policy, from `HKLM\SOFTWARE\Policies\Remora` (Group Policy or Intune):
/// `AllowedPlugins` (REG_MULTI_SZ of plugin IDs), `AllowExternalAI` and `AllowRemoteImages` (REG_DWORD 0/1).
/// A key that is set wins over the user's own choice.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Managed {
    pub allowed_plugins: Option<Vec<String>>,
    pub allow_external_ai: Option<bool>,
    pub allow_remote_images: Option<bool>,
}

pub const POLICY_KEY: &str = r"SOFTWARE\Policies\Remora";

impl Managed {
    #[cfg(windows)]
    pub fn read() -> Self {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        let Ok(key) = winreg::RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(POLICY_KEY) else { return Managed::default() };
        let flag = |name: &str| key.get_value::<u32, _>(name).ok().map(|v| v != 0);
        Managed {
            allowed_plugins: key.get_value::<Vec<String>, _>("AllowedPlugins").ok(),
            allow_external_ai: flag("AllowExternalAI"),
            allow_remote_images: flag("AllowRemoteImages"),
        }
    }

    /// No registry outside Windows: nothing is managed.
    #[cfg(not(windows))]
    pub fn read() -> Self {
        Managed::default()
    }

    /// True when the organization set at least one key: the Privacy settings are then read-only.
    pub fn is_managed(&self) -> bool {
        self.allowed_plugins.is_some() || self.allow_external_ai.is_some() || self.allow_remote_images.is_some()
    }

    pub fn policy(&self, preferences: &Preferences) -> CompliancePolicy {
        let allowed = self.allowed_plugins.as_ref().or(preferences.allowed_plugins.as_ref());
        CompliancePolicy {
            allowed_plugins: allowed.map(|ids| ids.iter().cloned().collect::<HashSet<_>>()),
            allow_external_ai: self.allow_external_ai.unwrap_or(preferences.allow_external_ai),
            allow_remote_images: self.allow_remote_images.unwrap_or(preferences.allow_remote_images),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_keys_win_over_the_user() {
        let user = Preferences { allowed_plugins: Some(vec!["slack".into()]), allow_external_ai: true, ..Preferences::default() };
        assert_eq!(Managed::default().policy(&user).allowed_plugins, Some(HashSet::from(["slack".to_string()])));
        assert!(!Managed::default().is_managed());

        let it = Managed { allowed_plugins: Some(vec!["github".into()]), allow_external_ai: Some(false), allow_remote_images: None };
        let policy = it.policy(&user);
        assert!(it.is_managed());
        assert_eq!(policy.allowed_plugins, Some(HashSet::from(["github".to_string()])));
        assert!(!policy.allow_external_ai, "the organization's false beats the user's true");
        assert!(policy.allow_remote_images, "unset keys keep the user's choice");
    }
}
