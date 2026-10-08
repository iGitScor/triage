//! The few strings Rust shows itself (tray menu, tooltip, notifications), in French when the system is.
//! Same dictionary as the interface: src/i18n/fr.json, generated from macos/scripts/translations_fr.py.

use std::collections::HashMap;
use std::sync::OnceLock;

static FR: OnceLock<HashMap<String, String>> = OnceLock::new();

fn french() -> bool {
    sys_locale::get_locale().is_some_and(|l| l.to_lowercase().starts_with("fr"))
}

/// English text → the system's language.
pub fn translator() -> impl Fn(&str) -> String {
    let fr = french().then(|| FR.get_or_init(|| serde_json::from_str(include_str!("../../src/i18n/fr.json")).unwrap_or_default()));
    move |text: &str| fr.and_then(|d| d.get(text)).cloned().unwrap_or_else(|| text.to_string())
}
