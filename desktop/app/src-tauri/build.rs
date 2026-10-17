// Every command the window can call is declared, which makes Tauri generate an `allow-<command>`
// permission for each; capabilities/default.json grants them to the main window, and nothing else is callable.
// A test in src/lib.rs checks this list, the handler and the capability stay the same.
pub const COMMANDS: &[&str] = &[
    "inbox",
    "refresh",
    "toggle_done",
    "toggle_pin",
    "move_to_reply",
    "start",
    "stop",
    "clear_done",
    "undo",
    "snooze",
    "unsnooze",
    "presets",
    "add_reminder",
    "stop_repeating",
    "open_item",
    "open_setup",
    "sources",
    "accounts",
    "connect",
    "reconnect",
    "sign_in",
    "cancel_sign_in",
    "rename_account",
    "disconnect",
    "settings",
    "set_preferences",
    "set_autostart",
    "erase_local_data",
    "update_status",
    "check_for_updates",
    "install_update",
    "snooze_advice",
    "dismiss_insight",
    "spread",
    "align_returns",
    "snooze_many",
    "sweep",
    "waiting_draft",
    "review_queue",
    "make_brief",
    "summarize",
    "triage",
    "apply_triage",
    "discard_triage",
    "hide",
    "quit",
    "open_notices",
    "open_agents_guide",
    "open_notification_settings",
];

/// THIRD-PARTY-NOTICES.txt is written by scripts/make-notices.py in CI and releases; a local build ships a note
/// saying so, since the bundle needs the file.
fn notices() {
    let path = std::path::Path::new("notices/THIRD-PARTY-NOTICES.txt");
    if !path.exists() {
        std::fs::create_dir_all("notices").expect("the notices folder");
        std::fs::write(
            path,
            "This development build doesn't list third-party licences: releases do (scripts/make-notices.py).\n",
        )
        .expect("the placeholder notices");
    }
}

fn main() {
    notices();
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("Tauri build");
}
