use crate::Preferences;
use remora_core::{AssistantPolicy, CompliancePolicy};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use ts_rs::TS;

/// The organization's policy, from `HKLM\SOFTWARE\Policies\Remora` (Group Policy or Intune), and
/// `HKCU\SOFTWARE\Policies\Remora` for user-scoped policies: a value set in HKLM wins. On Linux, from
/// `/etc/remora/policy.json` (Ansible, Puppet, Salt…), with the same names.
/// `AllowedPlugins` (REG_MULTI_SZ of plugin IDs), `AllowExternalAI` and `AllowRemoteImages` (REG_DWORD 0/1), and
/// `AutomaticUpdates` (REG_DWORD 0/1): 1 checks every day, locked on; 0 turns updating off entirely, Check now included.
/// For the OpenAI-compatible assistant (AI-23): `AllowedAIServers` (REG_MULTI_SZ of URL prefixes), `AIServer`
/// (REG_SZ, the server, locked in Settings) and `AllowLocalAI` (REG_DWORD 0/1, a server on this computer).
/// For every assistant, as on macOS: `AIExcludedSources` (REG_MULTI_SZ of plugin IDs, or `reminders`), added to the
/// user's own choice and locked in Settings, and `AllowedAIModels` (REG_MULTI_SZ of model IDs). And the Mac's other
/// settings: `HiddenContentSources` (REG_MULTI_SZ, added to the user's), `RefreshMinutes` (REG_DWORD, 1 to 60),
/// `OpenInApps` (REG_DWORD 0/1) and `ClaudeCodePath` (REG_SZ). A source's settings (REG_SZ, `SETTINGS`): what
/// the organization registered for Microsoft 365, `MicrosoftTenant` and `MicrosoftClientID`, locked in the form.
/// A key that is set wins over the user's own choice.
///
/// It fails closed: a value Remora can't read (wrong type, unreadable key) is applied as strictly as possible (no
/// plugin, no external AI, no remote images) and listed in `unreadable`, so Settings can say so. An admin who made a
/// typing mistake gets a locked-down app and a message, never a silently open one.
#[derive(Clone, Debug, Default, PartialEq, Serialize, TS)]
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
    /// Sources whose items never reach the assistant, on top of the user's.
    pub ai_excluded_sources: Option<Vec<String>>,
    /// The models the assistant may use, by any provider's ids.
    pub allowed_ai_models: Option<Vec<String>>,
    /// Sources whose notifications don't show what was written, on top of the user's. `*`: every source (an
    /// unreadable value).
    pub hidden_content_sources: Option<Vec<String>>,
    /// The refresh interval, 1 to 60 minutes; another number is ignored.
    pub refresh_minutes: Option<u32>,
    pub open_in_apps: Option<bool>,
    /// Where Claude Code is, locked in Settings. Empty: unreadable, so Claude Code is refused.
    pub claude_code_path: Option<String>,
    /// Sources' settings by policy name (`ConfigField::managed_key`), replacing what the user typed. Empty:
    /// unreadable, so the source can't sign in.
    pub settings: BTreeMap<String, String>,
    /// Names of the values that were present but unreadable, applied as "deny".
    pub unreadable: Vec<String>,
}

pub const POLICY_KEY: &str = r"SOFTWARE\Policies\Remora";
/// Linux: the same values as a root-owned JSON file (LINUX-05).
pub const POLICY_FILE: &str = "/etc/remora/policy.json";
/// The sources' settings an organization may set (`ConfigField::managed_key`).
pub const SETTINGS: [&str; 2] = ["MicrosoftTenant", "MicrosoftClientID"];
pub const VALUES: [&str; 15] = [
    "AllowedPlugins",
    "AllowExternalAI",
    "AllowRemoteImages",
    "AutomaticUpdates",
    "AllowedAIServers",
    "AIServer",
    "AllowLocalAI",
    "AIExcludedSources",
    "AllowedAIModels",
    "HiddenContentSources",
    "RefreshMinutes",
    "OpenInApps",
    "ClaudeCodePath",
    "MicrosoftTenant",
    "MicrosoftClientID",
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

/// `AllowedAIServers` and `AllowedAIModels`: REG_MULTI_SZ, or a REG_SZ separated by commas, semicolons or spaces.
/// Entries keep their case; a list of blanks is no list.
fn text_list(raw: RawValue) -> Result<Option<Vec<String>>, ()> {
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

    /// Outside Windows: the policy file, read at launch and at each refresh like the registry.
    #[cfg(not(windows))]
    pub fn read() -> Self {
        Self::read_file(std::path::Path::new(POLICY_FILE), true)
    }

    /// The policy file (Linux): the registry's values as JSON, `{"AllowedPlugins": ["github"], "AllowExternalAI":
    /// false}`. No file, nothing managed. A file Remora can't trust or read is applied as strictly as possible, like
    /// an unreadable registry key: one that isn't root's (`owned_by_root`), that others can write, or isn't JSON.
    pub fn read_file(path: &std::path::Path, owned_by_root: bool) -> Self {
        let locked = || Managed::from_values(|_| RawValue::Other);
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Managed::default(),
            Err(_) => return locked(),
        };
        if !Self::trusted(path, owned_by_root) {
            return locked();
        }
        match serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&text) {
            Ok(values) => Managed::from_values(|name| values.get(name).map_or(RawValue::Missing, Self::raw)),
            Err(_) => locked(),
        }
    }

    /// Only an administrator may set the policy: the file belongs to root and nobody else can write it.
    #[cfg(unix)]
    fn trusted(path: &std::path::Path, owned_by_root: bool) -> bool {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(path).is_ok_and(|m| (!owned_by_root || m.uid() == 0) && m.mode() & 0o022 == 0)
    }

    #[cfg(not(unix))]
    fn trusted(_: &std::path::Path, _: bool) -> bool {
        true
    }

    /// A JSON value as the registry would hold it: a list of strings is REG_MULTI_SZ, a switch a REG_DWORD.
    fn raw(value: &serde_json::Value) -> RawValue {
        use serde_json::Value;
        match value {
            Value::Array(items) => items
                .iter()
                .map(|item| item.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()
                .map_or(RawValue::Other, RawValue::MultiString),
            Value::String(text) => RawValue::String(text.clone()),
            Value::Bool(on) => RawValue::Number(u64::from(*on)),
            Value::Number(n) => n.as_u64().map_or(RawValue::Other, RawValue::Number),
            Value::Null | Value::Object(_) => RawValue::Other,
        }
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
        let allowed_ai_servers = match text_list(value("AllowedAIServers")) {
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
        // An AI list that can't be read turns the assistant off (see `policy`): the lists themselves stay unset.
        let ai_excluded_sources = plugin_list(value("AIExcludedSources")).unwrap_or_else(|()| {
            unreadable.push("AIExcludedSources".to_string());
            None
        });
        let allowed_ai_models = text_list(value("AllowedAIModels")).unwrap_or_else(|()| {
            unreadable.push("AllowedAIModels".to_string());
            None
        });
        let hidden_content_sources = plugin_list(value("HiddenContentSources")).unwrap_or_else(|()| {
            unreadable.push("HiddenContentSources".to_string());
            Some(vec!["*".to_string()])
        });
        let refresh_minutes = match value("RefreshMinutes") {
            RawValue::Missing => None,
            RawValue::Number(n) => u32::try_from(n).ok().filter(|n| (1..=60).contains(n)),
            RawValue::String(text) => text.trim().parse().ok().filter(|n| (1..=60).contains(n)),
            RawValue::MultiString(_) | RawValue::Other => {
                unreadable.push("RefreshMinutes".to_string());
                None
            }
        };
        let claude_code_path = text(value("ClaudeCodePath")).unwrap_or_else(|()| {
            unreadable.push("ClaudeCodePath".to_string());
            Some(String::new())
        });
        let mut settings = BTreeMap::new();
        for name in SETTINGS {
            match text(value(name)) {
                Ok(Some(setting)) => {
                    settings.insert(name.to_string(), setting);
                }
                Ok(None) => {}
                Err(()) => {
                    unreadable.push(name.to_string());
                    settings.insert(name.to_string(), String::new());
                }
            }
        }
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
        let open_in_apps = flag("OpenInApps");
        Managed {
            allowed_plugins,
            allow_external_ai,
            allow_remote_images,
            automatic_updates,
            allowed_ai_servers,
            ai_server,
            allow_local_ai,
            ai_excluded_sources,
            allowed_ai_models,
            hidden_content_sources,
            refresh_minutes,
            open_in_apps,
            claude_code_path,
            settings,
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
            || self.ai_excluded_sources.is_some()
            || self.allowed_ai_models.is_some()
            || self.ai_lists_unreadable()
    }

    /// An `AIExcludedSources` or `AllowedAIModels` Remora can't read: no source or model can be trusted to stay out,
    /// so no assistant runs, on this computer or elsewhere.
    fn ai_lists_unreadable(&self) -> bool {
        self.unreadable.iter().any(|name| name == "AIExcludedSources" || name == "AllowedAIModels")
    }

    /// What Remora acts on: the user's preferences with the organization's settings in place. Hidden content is
    /// only ever added to.
    pub fn effective(&self, preferences: &Preferences) -> Preferences {
        let mut effective = preferences.clone();
        for id in self.hidden_content_sources.iter().flatten() {
            if !effective.hidden_content_plugins.contains(id) {
                effective.hidden_content_plugins.push(id.clone());
            }
        }
        effective.refresh_minutes = self.refresh_minutes.unwrap_or(effective.refresh_minutes);
        effective.open_in_apps = self.open_in_apps.unwrap_or(effective.open_in_apps);
        effective
    }

    /// What the assistant may read and use: the user's exclusions plus the organization's, and the organization's
    /// models.
    pub fn assistant_policy(&self, preferences: &Preferences) -> AssistantPolicy {
        let excluded = preferences.assistant_excluded_sources.iter().chain(self.ai_excluded_sources.iter().flatten());
        AssistantPolicy::new(excluded.cloned().collect(), self.allowed_ai_models.clone())
    }

    pub fn policy(&self, preferences: &Preferences) -> CompliancePolicy {
        let allowed = self.allowed_plugins.as_ref().or(preferences.allowed_plugins.as_ref());
        CompliancePolicy {
            allowed_plugins: allowed.map(|ids| ids.iter().cloned().collect::<HashSet<_>>()),
            allow_external_ai: !self.ai_lists_unreadable()
                && self.allow_external_ai.unwrap_or(preferences.allow_external_ai),
            allow_remote_images: self.allow_remote_images.unwrap_or(preferences.allow_remote_images),
            allowed_ai_servers: self.allowed_ai_servers.clone(),
            ai_server: self.ai_server.clone(),
            allow_local_ai: if self.ai_lists_unreadable() { Some(false) } else { self.allow_local_ai },
            external_ai_managed_off: self.allow_external_ai == Some(false) || self.ai_lists_unreadable(),
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

    /// SEC-29: the organization keeps sources from the assistant and limits its models, as on macOS.
    #[test]
    fn a_sources_settings_are_read_and_fail_closed() {
        let it = Managed::from_values(values(&[
            ("MicrosoftTenant", RawValue::String(" acme.onmicrosoft.com ".into())),
            ("MicrosoftClientID", RawValue::String("  ".into())),
        ]));
        assert_eq!(it.settings, BTreeMap::from([("MicrosoftTenant".into(), "acme.onmicrosoft.com".into())]));
        assert!(it.unreadable.is_empty());
        assert!(!it.is_managed(), "the Privacy settings stay the user's");
        let broken = Managed::from_values(values(&[("MicrosoftClientID", RawValue::Number(1))]));
        assert_eq!(broken.settings, BTreeMap::from([("MicrosoftClientID".into(), String::new())]));
        assert_eq!(broken.unreadable, ["MicrosoftClientID"]);
    }

    #[test]
    fn ai_exclusions_and_models_are_read() {
        let it = Managed::from_values(values(&[
            ("AIExcludedSources", RawValue::MultiString(vec!["Slack".into(), " reminders ".into(), String::new()])),
            ("AllowedAIModels", RawValue::String("claude-haiku-5-5, gpt-X".into())),
        ]));
        assert!(it.is_managed() && it.unreadable.is_empty());
        let user = Preferences { assistant_excluded_sources: vec!["linear".into()], ..Preferences::default() };
        let policy = it.assistant_policy(&user);
        assert_eq!(
            policy.excluded_sources,
            HashSet::from(["linear".to_string(), "slack".to_string(), "reminders".to_string()]),
            "added to the user's own"
        );
        assert_eq!(policy.allowed_models, Some(vec!["claude-haiku-5-5".to_string(), "gpt-X".to_string()]));
        assert_eq!(Managed::default().assistant_policy(&user).allowed_models, None, "any model when unset");
    }

    /// An AI list that can't be read turns every assistant off, a local one included.
    #[test]
    fn unreadable_ai_lists_turn_the_assistant_off() {
        let user = Preferences { allow_external_ai: true, ..Preferences::default() };
        for name in ["AIExcludedSources", "AllowedAIModels"] {
            let it =
                Managed::from_values(values(&[(name, RawValue::Number(1)), ("AllowLocalAI", RawValue::Number(1))]));
            assert_eq!(it.unreadable, [name]);
            let policy = it.policy(&user);
            assert!(
                !policy.allow_external_ai && policy.allow_local_ai == Some(false) && policy.external_ai_managed_off
            );
        }
    }

    /// LINUX-05: the policy file, read with the registry's rules: it fails closed when it can't be trusted or read.
    #[test]
    fn the_policy_file_is_read_and_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("policy.json");
        assert_eq!(Managed::read_file(&file, false), Managed::default(), "no file: nothing managed");

        std::fs::write(
            &file,
            r#"{"AllowedPlugins": ["GitHub", "slack"], "AllowExternalAI": false, "AIServer": "https://ai.acme.io/v1",
               "Unknown": 1}"#,
        )
        .unwrap();
        let it = Managed::read_file(&file, false);
        assert_eq!(it.allowed_plugins, Some(vec!["github".to_string(), "slack".to_string()]));
        assert_eq!((it.allow_external_ai, it.ai_server.as_deref()), (Some(false), Some("https://ai.acme.io/v1")));
        assert!(it.unreadable.is_empty());

        std::fs::write(&file, r#"{"AllowExternalAI": "maybe", "AllowedPlugins": [1]}"#).unwrap();
        assert_eq!(Managed::read_file(&file, false).unreadable, ["AllowedPlugins", "AllowExternalAI"]);

        std::fs::write(&file, "not json").unwrap();
        assert_eq!(Managed::read_file(&file, false).unreadable.len(), VALUES.len(), "a broken file locks everything");
    }

    /// Only root's file counts, and not one others can write: a user can't loosen the policy.
    #[cfg(unix)]
    #[test]
    fn a_policy_file_anyone_could_write_locks_everything() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("policy.json");
        std::fs::write(&file, r#"{"AllowExternalAI": true}"#).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        use std::os::unix::fs::MetadataExt;
        if std::fs::metadata(&file).unwrap().uid() != 0 {
            let mine = Managed::read_file(&file, true);
            assert_eq!(mine.unreadable.len(), VALUES.len(), "the test's own file isn't root's");
            assert_eq!(mine.allow_external_ai, Some(false));
        }

        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o666)).unwrap();
        assert_eq!(Managed::read_file(&file, false).unreadable.len(), VALUES.len(), "writable by others");
    }

    /// SEC-20 on Windows and Linux: the Mac's other managed settings, with the same rules.
    #[test]
    fn the_macs_other_settings_are_read() {
        let it = Managed::from_values(values(&[
            ("HiddenContentSources", RawValue::MultiString(vec!["Slack".into()])),
            ("RefreshMinutes", RawValue::Number(15)),
            ("OpenInApps", RawValue::Number(0)),
            ("ClaudeCodePath", RawValue::String(" /opt/claude/bin/claude ".into())),
        ]));
        assert!(it.unreadable.is_empty());
        let user = Preferences { hidden_content_plugins: vec!["github".into()], ..Preferences::default() };
        let effective = it.effective(&user);
        assert_eq!(effective.hidden_content_plugins, ["github", "slack"], "added to the user's");
        assert_eq!((effective.refresh_minutes, effective.open_in_apps), (15, false));
        assert_eq!(it.claude_code_path.as_deref(), Some("/opt/claude/bin/claude"));
        assert!(!it.is_managed(), "the Privacy settings stay the user's");

        let odd = Managed::from_values(values(&[("RefreshMinutes", RawValue::Number(0))]));
        assert_eq!(odd.effective(&Preferences::default()).refresh_minutes, 5, "out of range: ignored");
        assert!(odd.unreadable.is_empty());
    }

    #[test]
    fn the_macs_other_settings_fail_closed() {
        let broken = Managed::from_values(values(&[
            ("HiddenContentSources", RawValue::Number(1)),
            ("RefreshMinutes", RawValue::Other),
            ("OpenInApps", RawValue::String("maybe".into())),
            ("ClaudeCodePath", RawValue::Number(1)),
        ]));
        assert_eq!(broken.unreadable, ["HiddenContentSources", "RefreshMinutes", "ClaudeCodePath", "OpenInApps"]);
        let effective = broken.effective(&Preferences::default());
        assert!(effective.hides_content("slack") && effective.hides_content("linear"), "every source hidden");
        assert!(!effective.open_in_apps && effective.refresh_minutes == 5);
        assert_eq!(broken.claude_code_path.as_deref(), Some(""), "Claude Code refused");
    }
}
