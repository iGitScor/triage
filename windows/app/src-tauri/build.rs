// Every command the window can call is declared, which makes Tauri generate an `allow-<command>`
// permission for each; capabilities/default.json grants them to the main window, and nothing else is callable.
// A test in src/lib.rs checks this list, the handler and the capability stay the same.
pub const COMMANDS: &[&str] = &[
    "inbox",
    "refresh",
    "toggle_done",
    "toggle_pin",
    "start",
    "stop",
    "clear_done",
    "undo",
    "snooze",
    "unsnooze",
    "presets",
    "slider_date",
    "slider_progress",
    "add_reminder",
    "open_item",
    "open_setup",
    "sources",
    "accounts",
    "connect",
    "reconnect",
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
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("Tauri build");
}
