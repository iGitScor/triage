//! The few strings Rust shows itself (tray menu, tooltip, notifications), in French when the system is.
//! Same dictionary as the interface: src/i18n/fr.json, generated from macos/scripts/translations_fr.py.

use remora_app::tray::Translator;
use std::sync::OnceLock;

static TRANSLATOR: OnceLock<Translator> = OnceLock::new();

/// English text → the system's language, read once.
pub fn translator() -> &'static Translator {
    TRANSLATOR
        .get_or_init(|| Translator::new(sys_locale::get_locale().as_deref(), include_str!("../../src/i18n/fr.json")))
}
