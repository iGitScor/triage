use crate::Preferences;
use remora_core::CompliancePolicy;
use serde::Serialize;
use std::collections::HashSet;

/// The organization's policy, from `HKLM\SOFTWARE\Policies\Remora` (Group Policy or Intune), and
/// `HKCU\SOFTWARE\Policies\Remora` for user-scoped policies: a value set in HKLM wins.
/// `AllowedPlugins` (REG_MULTI_SZ of plugin IDs), `AllowExternalAI` and `AllowRemoteImages` (REG_DWORD 0/1), and
/// `AutomaticUpdates` (REG_DWORD 0/1): 1 checks every day, locked on; 0 turns updating off entirely, Check now included.
/// For the OpenAI-compatible assistant (AI-23): `AllowedAIServers` (REG_MULTI_SZ of URL prefixes), `AIServer`
/// (REG_SZ, the server, locked in Settings) and `AllowLocalAI` (REG_DWORD 0/1, a server on this computer).
/// A key that is set wins over the user's own choice.
///
/// It fails closed: a value Remora can't read (wrong type, unreadable key) is applied as strictly as possible (no
/// plugin, no external AI, no remote images) and listed in `unreadable`, so Settings can say so. An admin who made a
/// typing mistake gets a locked-down app and a message, never a silently open one.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Managed {
    pub allowed_plugins: Option<Vec<String>>,
    pub allow_external_ai: Option<bool>,
    pub allow_remote_images: Option<bool>,
    pub automatic_updates: Option<bool>,
    /// URL prefixes the OpenAI-compatible assistant may use. Empty: none (an unreadable value).
    pub allowed_ai_servers: Option<Vec<String>>,
    /// The OpenAI-compatible assistant's server. Empty: unreadable, so the assistant is refused.
    pub ai_server: Option<String>,
    pub allow_local_ai: Option<bool>,
    /// Names of the values that were present but unreadable, applied as "deny".
    pub unreadable: Vec<String>,
}

pub const POLICY_KEY: &str = r"SOFTWARE\Policies\Remora";
pub const VALUES: [&str; 7] = [
    "AllowedPlugins",
    "AllowExternalAI",
    "AllowRemoteImages",
    "AutomaticUpdates",
    "AllowedAIServers",
    "AIServer",
    "AllowLocalAI",
];

/// A registry value as found, before interpretation.
#[derive(Clone, Debug, PartialEq)]
pub enum RawValue {
    Missing,
    MultiString(Vec<String>),
    String(String),
    Number(u64),
    /// Present but of a type Remora doesn't use (REG_BINARY…), or unreadable.
    Other,
}

/// `AllowedPlugins`: REG_MULTI_SZ, or a REG_SZ separated by commas, semicolons or spaces.
fn plugin_list(raw: RawValue) -> Result<Option<Vec<String>>, ()> {
    let ids: Vec<String> = match raw {
        RawValue::Missing => return Ok(None),
        RawValue::MultiString(items) => items,
        RawValue::String(text) => text.split([',', ';', ' ', '\n', '\t']).map(str::to_string).collect(),
        RawValue::Number(_) | RawValue::Other => return Err(()),
    };
    Ok(Some(ids.iter().map(|id| id.trim().to_lowercase()).filter(|id| !id.is_empty()).collect()))
}

/// `AllowedAIServers`: REG_MULTI_SZ, or a REG_SZ separated by commas, semicolons or spaces. Paths keep their case;
/// a list of blanks is no list.
fn server_list(raw: RawValue) -> Result<Option<Vec<String>>, ()> {
    let urls: Vec<String> = match raw {
        RawValue::Missing => return Ok(None),
        RawValue::MultiString(items) => items,
        RawValue::String(text) => text.split([',', ';', ' ', '\n', '\t']).map(str::to_string).collect(),
        RawValue::Number(_) | RawValue::Other => return Err(()),
    };
    let urls: Vec<String> = urls.iter().map(|u| u.trim().to_string()).filter(|u| !u.is_empty()).collect();
    Ok((!urls.is_empty()).then_some(urls))
}

/// `AIServer`: one REG_SZ URL. Blank is unset.
fn text(raw: RawValue) -> Result<Option<String>, ()> {
    match raw {
        RawValue::Missing => Ok(None),
        RawValue::String(text) => Ok(Some(text.trim().to_string()).filter(|t| !t.is_empty())),
        RawValue::MultiString(_) | RawValue::Number(_) | RawValue::Other => Err(()),
    }
}

/// A switch: REG_DWORD or REG_QWORD 0 / non-zero, or a REG_SZ "0", "1", "true", "false".
fn switch(raw: RawValue) -> Result<Option<bool>, ()> {
    match raw {
        RawValue::Missing => Ok(None),
        RawValue::Number(n) => Ok(Some(n != 0)),
        RawValue::String(text) => match text.trim().to_lowercase().as_str() {
            "1" | "true" => Ok(Some(true)),
            "0" | "false" => Ok(Some(false)),
            _ => Err(()),
        },
        RawValue::MultiString(_) | RawValue::Other => Err(()),
    }
}

impl Managed {
    /// Read at launch and again at each refresh, so a Group Policy or Intune change applies without a restart.
    #[cfg(windows)]
    pub fn read() -> Self {
        use winreg::enums::*;
        let machine = Self::read_hive(HKEY_LOCAL_MACHINE);
        let user = Self::read_hive(HKEY_CURRENT_USER);
        Managed::from_values(|name| Self::layered(machine(name), user(name)))
    }

    /// One hive's values, as found. A key that exists but can't be opened makes every value unreadable.
    #[cfg(windows)]
    fn read_hive(hive: winreg::HKEY) -> impl Fn(&str) -> RawValue {
        use std::io::ErrorKind;
        use winreg::enums::*;
        let key = match winreg::RegKey::predef(hive).open_subkey(POLICY_KEY) {
            Ok(key) => Some(Ok(key)),
            Err(e) if e.kind() == ErrorKind::NotFound => None,
            Err(_) => Some(Err(())),
        };
        move |name: &str| match &key {
            None => RawValue::Missing,
            Some(Err(())) => RawValue::Other,
            Some(Ok(key)) => match key.get_raw_value(name) {
                Err(e) if e.kind() == ErrorKind::NotFound => RawValue::Missing,
                Err(_) => RawValue::Other,
                Ok(raw) => match raw.vtype {
                    REG_MULTI_SZ => {
                        key.get_value::<Vec<String>, _>(name).map_or(RawValue::Other, RawValue::MultiString)
                    }
                    REG_SZ | REG_EXPAND_SZ => {
                        key.get_value::<String, _>(name).map_or(RawValue::Other, RawValue::String)
                    }
                    REG_DWORD => key.get_value::<u32, _>(name).map_or(RawValue::Other, |n| RawValue::Number(n.into())),
                    REG_QWORD => key.get_value::<u64, _>(name).map_or(RawValue::Other, RawValue::Number),
                    _ => RawValue::Other,
                },
            },
        }
    }

    /// The machine's value when it has one, else the user's: an administrator's setting always wins.
    pub fn layered(machine: RawValue, user: RawValue) -> RawValue {
        if machine == RawValue::Missing {
            user
        } else {
            machine
        }
    }

    /// No registry outside Windows: nothing is managed.
    #[cfg(not(windows))]
    pub fn read() -> Self {
        Managed::default()
    }

    /// Interprets the values. Lenient where the intent is clear (a REG_SZ list "github, slack", a REG_SZ "0"),
    /// strict otherwise.
    pub fn from_values(value: impl Fn(&str) -> RawValue) -> Self {
        let mut unreadable = vec![];
        let allowed_plugins = match plugin_list(value("AllowedPlugins")) {
            Ok(list) => list,
            Err(()) => {
                unreadable.push("AllowedPlugins".to_string());
                Some(vec![])
            }
        };
        let allowed_ai_servers = match server_list(value("AllowedAIServers")) {
            Ok(list) => list,
            Err(()) => {
                unreadable.push("AllowedAIServers".to_string());
                Some(vec![])
            }
        };
        let ai_server = match text(value("AIServer")) {
            Ok(server) => server,
            Err(()) => {
                unreadable.push("AIServer".to_string());
                Some(String::new())
            }
        };
        let mut flag = |name: &str| match switch(value(name)) {
            Ok(on) => on,
            Err(()) => {
                unreadable.push(name.to_string());
                Some(false)
            }
        };
        let allow_external_ai = flag("AllowExternalAI");
        let allow_remote_images = flag("AllowRemoteImages");
        // Unreadable means off: an update the organization didn't plan is the riskier side.
        let automatic_updates = flag("AutomaticUpdates");
        let allow_local_ai = flag("AllowLocalAI");
        Managed {
            allowed_plugins,
            allow_external_ai,
            allow_remote_images,
            automatic_updates,
            allowed_ai_servers,
            ai_server,
            allow_local_ai,
            unreadable,
        }
    }

    /// False when the organization turned updating off: no check at all, not even Check now.
    pub fn updates_allowed(&self) -> bool {
        self.automatic_updates != Some(false)
    }

    /// Whether Remora checks for updates by itself: the organization's choice, else the user's.
    pub fn checks_for_updates(&self, preferences: &Preferences) -> bool {
        self.automatic_updates.unwrap_or(preferences.check_for_updates)
    }

    /// True when the organization set at least one key: the Privacy settings are then read-only.
    pub fn is_managed(&self) -> bool {
        self.allowed_plugins.is_some()
            || self.allow_external_ai.is_some()
            || self.allow_remote_images.is_some()
            || self.allowed_ai_servers.is_some()
            || self.ai_server.is_some()
            || self.allow_local_ai.is_some()
    }

    pub fn policy(&self, preferences: &Preferences) -> CompliancePolicy {
        let allowed = self.allowed_plugins.as_ref().or(preferences.allowed_plugins.as_ref());
        CompliancePolicy {
            allowed_plugins: allowed.map(|ids| ids.iter().cloned().collect::<HashSet<_>>()),
            allow_external_ai: self.allow_external_ai.unwrap_or(preferences.allow_external_ai),
            allow_remote_images: self.allow_remote_images.unwrap_or(preferences.allow_remote_images),
            allowed_ai_servers: self.allowed_ai_servers.clone(),
            ai_server: self.ai_server.clone(),
            allow_local_ai: self.allow_local_ai,
            external_ai_managed_off: self.allow_external_ai == Some(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_keys_win_over_the_user() {
        let user = Preferences {
            allowed_plugins: Some(vec!["slack".into()]),
            allow_external_ai: true,
            ..Preferences::default()
        };
        assert_eq!(Managed::default().policy(&user).allowed_plugins, Some(HashSet::from(["slack".to_string()])));
        assert!(!Managed::default().is_managed());

        let it = Managed {
            allowed_plugins: Some(vec!["github".into()]),
            allow_external_ai: Some(false),
            allow_remote_images: None,
            automatic_updates: None,
            ..Managed::default()
        };
        let policy = it.policy(&user);
        assert!(it.is_managed());
        assert_eq!(policy.allowed_plugins, Some(HashSet::from(["github".to_string()])));
        assert!(!policy.allow_external_ai, "the organization's false beats the user's true");
        assert!(policy.allow_remote_images, "unset keys keep the user's choice");
    }

    fn values(entries: &[(&str, RawValue)]) -> impl Fn(&str) -> RawValue {
        let entries: Vec<(String, RawValue)> = entries.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        move |name| entries.iter().find(|(k, _)| k == name).map_or(RawValue::Missing, |(_, v)| v.clone())
    }

    #[test]
    fn well_typed_values_are_read() {
        let it = Managed::from_values(values(&[
            ("AllowedPlugins", RawValue::MultiString(vec!["GitHub".into(), " slack ".into(), String::new()])),
            ("AllowExternalAI", RawValue::Number(0)),
            ("AllowRemoteImages", RawValue::Number(1)),
        ]));
        assert_eq!(it.allowed_plugins, Some(vec!["github".to_string(), "slack".to_string()]));
        assert_eq!((it.allow_external_ai, it.allow_remote_images), (Some(false), Some(true)));
        assert!(it.unreadable.is_empty());
        assert_eq!(Managed::from_values(values(&[])), Managed::default());
    }

    #[test]
    fn strings_are_read_when_the_intent_is_clear() {
        let it = Managed::from_values(values(&[
            ("AllowedPlugins", RawValue::String("github, gitlab;slack".into())),
            ("AllowExternalAI", RawValue::String("0".into())),
            ("AllowRemoteImages", RawValue::String(" TRUE ".into())),
        ]));
        assert_eq!(it.allowed_plugins, Some(vec!["github".to_string(), "gitlab".to_string(), "slack".to_string()]));
        assert_eq!((it.allow_external_ai, it.allow_remote_images), (Some(false), Some(true)));
        assert!(it.unreadable.is_empty());
    }

    /// A wrongly typed value used to read as "not set", which allowed everything.
    #[test]
    fn unreadable_values_fail_closed() {
        let user = Preferences {
            allowed_plugins: None,
            allow_external_ai: true,
            allow_remote_images: true,
            ..Preferences::default()
        };
        let it = Managed::from_values(values(&[
            ("AllowedPlugins", RawValue::Number(1)),
            ("AllowExternalAI", RawValue::String("yes please".into())),
            ("AllowRemoteImages", RawValue::Other),
        ]));
        assert_eq!(it.unreadable, ["AllowedPlugins", "AllowExternalAI", "AllowRemoteImages"]);
        assert!(it.is_managed());
        let policy = it.policy(&user);
        assert_eq!(policy.allowed_plugins, Some(HashSet::new()), "no plugin is allowed");
        assert!(!policy.allow_external_ai && !policy.allow_remote_images);

        let locked = Managed::from_values(|_| RawValue::Other);
        assert_eq!(locked.unreadable.len(), VALUES.len());
        assert_eq!(locked.allowed_plugins, Some(vec![]));
        assert!(!locked.updates_allowed(), "an unreadable AutomaticUpdates turns updating off");
    }

    /// A user-scoped policy applies where the machine sets nothing; the machine's always wins.
    #[test]
    fn the_machine_wins_over_the_user() {
        let machine = values(&[("AllowExternalAI", RawValue::Number(0))]);
        let user = values(&[("AllowExternalAI", RawValue::Number(1)), ("AllowRemoteImages", RawValue::Number(0))]);
        let it = Managed::from_values(|name| Managed::layered(machine(name), user(name)));
        assert_eq!(it.allow_external_ai, Some(false), "the machine's");
        assert_eq!(it.allow_remote_images, Some(false), "the user's, where the machine says nothing");
        assert_eq!(it.allowed_plugins, None);
    }

    /// Updates follow the user unless the organization set AutomaticUpdates.
    #[test]
    fn the_organization_decides_updates() {
        let on = Preferences { check_for_updates: true, ..Preferences::default() };
        let none = Managed::default();
        assert!(
            none.updates_allowed() && none.checks_for_updates(&on) && !none.checks_for_updates(&Preferences::default())
        );

        let off = Managed::from_values(values(&[("AutomaticUpdates", RawValue::Number(0))]));
        assert!(!off.updates_allowed() && !off.checks_for_updates(&on));
        assert!(!off.is_managed(), "the Privacy settings stay the user's");

        let forced = Managed::from_values(values(&[("AutomaticUpdates", RawValue::Number(1))]));
        assert!(forced.updates_allowed() && forced.checks_for_updates(&Preferences::default()));
    }

    /// AI-23: the OpenAI-compatible assistant's keys, read and failed closed like the others.
    #[test]
    fn ai_server_keys_are_read_and_fail_closed() {
        let it = Managed::from_values(values(&[
            (
                "AllowedAIServers",
                RawValue::MultiString(vec![" https://acme.openai.azure.com/OpenAI/ ".into(), String::new()]),
            ),
            ("AIServer", RawValue::String("https://acme.openai.azure.com/OpenAI/v1".into())),
            ("AllowLocalAI", RawValue::Number(1)),
        ]));
        assert_eq!(
            it.allowed_ai_servers,
            Some(vec!["https://acme.openai.azure.com/OpenAI/".to_string()]),
            "paths keep their case"
        );
        assert_eq!(it.ai_server.as_deref(), Some("https://acme.openai.azure.com/OpenAI/v1"));
        assert_eq!(it.allow_local_ai, Some(true));
        assert!(it.is_managed() && it.unreadable.is_empty());
        let policy = it.policy(&Preferences::default());
        assert_eq!(policy.ai_server.as_deref(), Some("https://acme.openai.azure.com/OpenAI/v1"));
        assert_eq!(policy.allow_local_ai, Some(true));

        let blanks = Managed::from_values(values(&[
            ("AllowedAIServers", RawValue::String(" , ".into())),
            ("AIServer", RawValue::String("  ".into())),
        ]));
        assert_eq!((blanks.allowed_ai_servers, blanks.ai_server), (None, None), "blank means unset");

        let broken = Managed::from_values(values(&[
            ("AllowedAIServers", RawValue::Number(1)),
            ("AIServer", RawValue::MultiString(vec!["https://x".into()])),
            ("AllowLocalAI", RawValue::String("maybe".into())),
        ]));
        assert_eq!(broken.unreadable, ["AllowedAIServers", "AIServer", "AllowLocalAI"]);
        assert_eq!(broken.allowed_ai_servers, Some(vec![]), "no server allowed");
        assert_eq!(broken.ai_server.as_deref(), Some(""), "a server that can't be used");
        assert_eq!(broken.allow_local_ai, Some(false));

        let off = Managed::from_values(values(&[("AllowExternalAI", RawValue::Number(0))]));
        assert!(off.policy(&Preferences::default()).external_ai_managed_off);
        assert!(
            !Managed::default()
                .policy(&Preferences { allow_external_ai: false, ..Preferences::default() })
                .external_ai_managed_off
        );
    }
}
