//! The privacy policy as the inbox applies it: your Privacy settings with the organization's in place. What may be
//! connected and reached, which settings the organization replaces, what the assistant may read. Made from the
//! current preferences each time it is asked. Ported from the macOS app's `InboxPolicy`.

use super::{any_manifest, CLAUDE_CODE_PATH_UNREADABLE};
use crate::{Managed, Preferences};
use remora_core::{Account, AssistantPolicy, CompliancePolicy, Message, PluginManifest, AI_SERVER_PLUGIN};
use remora_plugins::{claude, registry};
use std::collections::HashMap;

pub struct InboxPolicy<'a> {
    preferences: &'a Preferences,
    managed: &'a Managed,
}

impl<'a> InboxPolicy<'a> {
    pub fn new(preferences: &'a Preferences, managed: &'a Managed) -> Self {
        InboxPolicy { preferences, managed }
    }

    pub fn compliance(&self) -> CompliancePolicy {
        self.managed.policy(self.preferences)
    }

    /// The user's preferences with the organization's settings in place: what Remora acts on.
    pub fn effective_preferences(&self) -> Preferences {
        self.managed.effective(self.preferences)
    }

    /// What the assistant may read and use: everything but the sources you or your organization keep from it, with the
    /// organization's models.
    pub fn assistant_policy(&self) -> AssistantPolicy {
        self.managed.assistant_policy(self.preferences)
    }

    /// The organization allows update checks, and the user wants them (or the organization turned them on).
    pub fn checks_for_updates(&self) -> bool {
        self.managed.checks_for_updates(self.preferences)
    }

    /// Whether a source's tool is allowed.
    pub fn allows(&self, plugin_id: &str) -> bool {
        registry::manifest(plugin_id).is_some_and(|m| self.compliance().allows(&m))
    }

    /// An account with the organization's settings in place of its own: `AIServer` for the OpenAI-compatible
    /// assistant, and each field's `managed_key` (`MicrosoftTenant`).
    pub fn with_policy_settings(&self, account: &Account) -> Account {
        let mut account = account.clone();
        if account.plugin_id == AI_SERVER_PLUGIN {
            if let Some(forced) = &self.compliance().ai_server {
                account.settings.insert("host".into(), forced.clone());
            }
        }
        for field in any_manifest(&account.plugin_id).map(|m| m.fields).unwrap_or_default() {
            if let Some(forced) = field.managed_key.and_then(|key| self.managed.settings.get(&key)) {
                account.settings.insert(field.key, forced.clone());
            }
        }
        account
    }

    /// An assistant account (its settings already the policy's) with its models kept within the allowed ones, and the
    /// organization's Claude Code path, which still passes the plugin's own checks. One that can't be read turns
    /// Claude Code off.
    pub fn constrained(&self, account: &Account, manifest: &PluginManifest) -> Result<Account, Message> {
        let defaults: HashMap<String, String> =
            manifest.fields.iter().map(|f| (f.key.clone(), f.default_value.clone())).collect();
        let mut constrained = account.clone();
        constrained.settings = self.assistant_policy().constrained(&account.settings, &defaults);
        if constrained.plugin_id != claude::code::ID {
            return Ok(constrained);
        }
        match self.managed.claude_code_path.as_deref() {
            Some("") => Err(CLAUDE_CODE_PATH_UNREADABLE.into()),
            Some(path) => {
                constrained.settings.insert("path".into(), path.to_string());
                Ok(constrained)
            }
            None => Ok(constrained),
        }
    }
}
