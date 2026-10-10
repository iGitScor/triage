//! Remora for Windows: the tray app. The inbox logic lives in `remora_app`; this crate only does what needs
//! the system: the tray icon and its popup, notifications, opening links, start at login, the reminder
//! shortcut, and the commands the interface (../src) calls.

mod avatars;
mod commands;
mod i18n;
mod updates;

use remora_app::tray::{notification_text, TrayImage, TraySummary};
use remora_app::{demo, paths, CredentialVault, Inbox, JsonStore, Managed, MemoryVault, Secrets, Vault};
use remora_core::Notice;
use remora_plugins::{HttpClient, ReqwestClient};
use std::sync::Arc;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
#[cfg(not(windows))]
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_positioner::{Position, WindowExt};
use tokio::sync::{Mutex, Notify};

pub struct AppState {
    pub inbox: Mutex<Inbox>,
    /// Held while the tokens change, from reading them to saving them, so two changes never race.
    pub vault_writes: Mutex<()>,
    /// The inbox's files, flushed when the app quits.
    pub store: Option<JsonStore>,
    pub http: Arc<dyn HttpClient>,
    pub refresh_now: Notify,
    pub demo: bool,
    pub updates: updates::Updates,
}

/// The shortcut that opens a new reminder from anywhere (tray icons can't be dragged on Windows).
pub const REMINDER_SHORTCUT: &str = "CommandOrControl+Alt+R";

const TRAY_QUIET: &[u8] = include_bytes!("../icons/tray-quiet.png");
const TRAY_ATTENTION: &[u8] = include_bytes!("../icons/tray-attention.png");
/// Focus on a task in progress: the inverted icon, then the frames of the fish's wiggle.
const TRAY_FOCUS: [&[u8]; 5] = [
    include_bytes!("../icons/tray-focus-0.png"),
    include_bytes!("../icons/tray-focus-1.png"),
    include_bytes!("../icons/tray-focus-2.png"),
    include_bytes!("../icons/tray-focus-3.png"),
    include_bytes!("../icons/tray-focus-4.png"),
];

/// What the tray last showed: whether it's focused, and the one task in progress (id, title) if there's
/// exactly one. The menu is rebuilt and the fish swims only from this.
#[derive(Clone, Default, PartialEq)]
struct TrayLook {
    focused: bool,
    task: Option<(String, String)>,
    /// A newer version found by the updater: the menu offers it.
    update: Option<String>,
}

static TRAY_LOOK: std::sync::Mutex<Option<TrayLook>> = std::sync::Mutex::new(None);

fn tray_look() -> TrayLook {
    TRAY_LOOK.lock().ok().and_then(|look| look.clone()).unwrap_or_default()
}

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    let demo_mode = args.iter().any(|a| a == "--demo");
    let window_mode = demo_mode || args.iter().any(|a| a == "--window");

    if let Some(legacy) = paths::legacy_data_dir() {
        if let Err(error) = paths::migrate(&legacy, &paths::data_dir()) {
            eprintln!("Remora: could not move the data saved by 0.3.0: {error}");
        }
    }
    let mut inbox = if demo_mode {
        Inbox::in_memory(Arc::new(MemoryVault::default()), Managed::default())
    } else {
        Inbox::open(JsonStore::new(paths::data_dir()), Arc::new(CredentialVault), Managed::read())
    };
    if demo_mode {
        demo::load(&mut inbox, chrono::Utc::now());
    }
    let state = AppState {
        store: inbox.store(),
        inbox: Mutex::new(inbox),
        vault_writes: Mutex::new(()),
        http: Arc::new(ReqwestClient::new()),
        refresh_now: Notify::new(),
        demo: demo_mode,
        updates: Default::default(),
    };

    tauri::Builder::default()
        // The interface speaks the tray's language: the system locale Rust read, not the webview's own.
        .append_invoke_initialization_script(format!("window.__REMORA_LANG__ = {:?};", i18n::translator().lang()))
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show(&main_window(app), false)))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .register_asynchronous_uri_scheme_protocol("avatar", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn(async move { responder.respond(avatars::serve(app, request).await) });
        })
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _, event| {
                    if event.state() == ShortcutState::Pressed {
                        new_reminder(app);
                    }
                })
                .build(),
        )
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::inbox,
            commands::refresh,
            commands::toggle_done,
            commands::toggle_pin,
            commands::start,
            commands::stop,
            commands::clear_done,
            commands::undo,
            commands::snooze,
            commands::unsnooze,
            commands::presets,
            commands::slider_date,
            commands::slider_progress,
            commands::add_reminder,
            commands::open_item,
            commands::open_setup,
            commands::sources,
            commands::accounts,
            commands::connect,
            commands::reconnect,
            commands::rename_account,
            commands::disconnect,
            commands::settings,
            commands::set_preferences,
            commands::set_autostart,
            commands::erase_local_data,
            commands::update_status,
            commands::check_for_updates,
            commands::install_update,
            commands::snooze_advice,
            commands::dismiss_insight,
            commands::spread,
            commands::align_returns,
            commands::snooze_many,
            commands::sweep,
            commands::waiting_draft,
            commands::review_queue,
            commands::make_brief,
            commands::summarize,
            commands::triage,
            commands::apply_triage,
            commands::discard_triage,
            commands::hide,
            commands::quit,
        ])
        .setup(move |app| {
            let window = main_window(app.handle());
            if window_mode {
                window.set_decorations(true)?;
                window.set_skip_taskbar(false)?;
                window.set_always_on_top(false)?;
                window.show()?;
            } else {
                let hidden = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::Focused(false) = event {
                        let _ = hidden.hide();
                    }
                });
            }
            build_tray(app.handle())?;
            if let Err(error) = app.global_shortcut().register(REMINDER_SHORTCUT) {
                eprintln!("Remora: {REMINDER_SHORTCUT} is taken by another app: {error}");
            }
            tauri::async_runtime::spawn(background(app.handle().clone()));
            tauri::async_runtime::spawn(swim(app.handle().clone()));
            if !demo_mode {
                tauri::async_runtime::spawn(updates::daily(app.handle().clone()));
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Remora")
        .run(|app, event| match event {
            // A tray app keeps running when its popup closes.
            tauri::RunEvent::ExitRequested { api, code: None, .. } => api.prevent_exit(),
            // Files are written in the background: the last saves land before the process ends.
            tauri::RunEvent::Exit => {
                if let Some(store) = &app.state::<AppState>().store {
                    store.flush();
                }
            }
            _ => {}
        });
}

fn main_window(app: &AppHandle) -> WebviewWindow {
    app.get_webview_window("main").expect("the main window is declared in tauri.conf.json")
}

/// Shows the popup next to the tray icon: above it on Windows (taskbar at the bottom), below it elsewhere.
fn show(window: &WebviewWindow, at_tray: bool) {
    if at_tray && !window.is_decorated().unwrap_or(false) {
        let _ = window.move_window(if cfg!(windows) { Position::TrayBottomCenter } else { Position::TrayCenter });
    }
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

fn toggle(app: &AppHandle) {
    let window = main_window(app);
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        show(&window, true);
        // The inbox opens on what you're working on, if anything.
        let _ = app.emit("opened", ());
    }
}

fn new_reminder(app: &AppHandle) {
    show(&main_window(app), true);
    let _ = app.emit("show", "reminder");
}

/// The tray menu. With exactly one task in progress, it starts with that task, Done and Stop.
fn tray_menu(app: &AppHandle, task: Option<&str>, update: Option<&str>) -> tauri::Result<Menu<tauri::Wry>> {
    let t = |text: &str| i18n::translator().t(text);
    let menu = Menu::new(app)?;
    if let Some(title) = task {
        menu.append(&MenuItem::with_id(app, "task", title, false, None::<&str>)?)?;
        menu.append(&MenuItem::with_id(app, "task-done", t("Done"), true, None::<&str>)?)?;
        menu.append(&MenuItem::with_id(app, "task-stop", t("Stop"), true, None::<&str>)?)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    menu.append(&MenuItem::with_id(app, "open", t("Open Remora"), true, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "refresh", t("Refresh"), true, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "reminder", t("New reminder"), true, Some(REMINDER_SHORTCUT))?)?;
    if let Some(version) = update {
        let label = t("Update to Remora %@…").replace("%@", version);
        menu.append(&MenuItem::with_id(app, "update", label, true, None::<&str>)?)?;
    }
    menu.append(&MenuItem::with_id(app, "settings", t("Settings"), true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "quit", t("Quit"), true, None::<&str>)?)?;
    Ok(menu)
}

/// Done or Stop on the one task in progress, from the tray menu.
fn finish_task(app: &AppHandle, done: bool) {
    let Some((id, _)) = tray_look().task else { return };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        {
            let state = app.state::<AppState>();
            let mut inbox = state.inbox.lock().await;
            if done {
                inbox.toggle_done(&id, chrono::Utc::now());
            } else {
                inbox.stop(&id);
            }
        }
        changed(&app).await;
    });
}

fn build_tray(app: &AppHandle) -> tauri::Result<TrayIcon> {
    let menu = tray_menu(app, None, None)?;

    TrayIconBuilder::with_id("main")
        .icon(tauri::image::Image::from_bytes(TRAY_QUIET)?)
        .tooltip("Remora")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show(&main_window(app), true),
            "refresh" => app.state::<AppState>().refresh_now.notify_one(),
            "reminder" => new_reminder(app),
            "task-done" => finish_task(app, true),
            "task-stop" => finish_task(app, false),
            "settings" | "update" => {
                show(&main_window(app), true);
                let _ = app.emit("show", "settings");
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                toggle(tray.app_handle());
            }
        })
        .build(app)
}

/// Refreshes every N minutes (and on request), and checks snoozes and reminders every 30 seconds.
async fn background(app: AppHandle) {
    // A refresh the user asked for (or a changed interval) doesn't wait out a slow-down; a rate limit still holds.
    let mut manual = false;
    loop {
        refresh(&app, manual).await;
        manual = false;
        let minutes = app.state::<AppState>().inbox.lock().await.preferences.refresh_minutes.max(1);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(u64::from(minutes) * 60);
        loop {
            let state = app.state::<AppState>();
            tokio::select! {
                _ = state.refresh_now.notified() => {
                    manual = true;
                    break;
                }
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    let notices = state.inbox.lock().await.tick(chrono::Utc::now());
                    if !notices.is_empty() {
                        post(&app, &notices);
                        changed(&app).await;
                    } else {
                        // Keeps the elapsed time of a task in progress current.
                        update_tray(&app).await;
                    }
                    if tokio::time::Instant::now() >= deadline { break }
                }
            }
        }
    }
}

pub async fn refresh(app: &AppHandle, manual: bool) {
    let state = app.state::<AppState>();
    if !state.demo {
        // The policy is read again each time: a change from Group Policy or Intune applies without a restart.
        {
            let mut inbox = state.inbox.lock().await;
            inbox.managed = remora_app::Managed::read();
            inbox.enforce_ai_policy();
        }
        // The tokens are read once per launch, outside the lock.
        let vault = {
            let inbox = state.inbox.lock().await;
            inbox.needs_secrets().then(|| inbox.vault())
        };
        if let Some(vault) = vault {
            if let Ok(Ok(secrets)) = tauri::async_runtime::spawn_blocking(move || vault.load()).await {
                state.inbox.lock().await.provide_secrets(secrets);
            }
        }
        let jobs = state.inbox.lock().await.fetch_jobs(state.http.clone(), chrono::Utc::now(), manual);
        let results = remora_app::run(jobs).await;
        let notices = state.inbox.lock().await.apply(results, chrono::Utc::now());
        post(app, &notices);
    }
    changed(app).await;
}

/// Saves every account's tokens off the async runtime and without the inbox lock: the Credential Manager can be slow.
/// The caller holds `vault_writes` from `secrets_after` to the `…_saved` call.
pub async fn save_secrets(vault: Arc<dyn Vault>, secrets: Secrets) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || vault.save(&secrets)).await.map_err(|e| e.to_string())?
}

/// Tells the interface to reload, and updates the tray.
pub async fn changed(app: &AppHandle) {
    update_tray(app).await;
    let _ = app.emit("inbox-changed", ());
}

/// The tray icon, tooltip and menu, drawn from `TraySummary`. While a task is in progress and focus is on, only
/// that task shows, with the inverted icon.
pub(crate) async fn update_tray(app: &AppHandle) {
    let now = chrono::Utc::now();
    let summary = TraySummary::of(&*app.state::<AppState>().inbox.lock().await, now);
    let Some(tray) = app.tray_by_id("main") else { return };
    let _ = tray.set_tooltip(Some(summary.tooltip(now, i18n::translator())));
    let icon = match summary.icon() {
        TrayImage::Focus => TRAY_FOCUS[0],
        TrayImage::Attention => TRAY_ATTENTION,
        TrayImage::Quiet => TRAY_QUIET,
    };
    if let Ok(image) = tauri::image::Image::from_bytes(icon) {
        let _ = tray.set_icon(Some(image));
    }

    let look = TrayLook { focused: summary.focused, task: summary.task, update: updates::offered(app).await };
    let previous = TRAY_LOOK.lock().ok().and_then(|mut last| last.replace(look.clone()));
    if previous.as_ref().map(|p| (&p.task, &p.update)) != Some((&look.task, &look.update)) {
        if let Ok(menu) = tray_menu(app, look.task.as_ref().map(|(_, title)| title.as_str()), look.update.as_deref()) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

/// Every 10 seconds while focused, the fish wiggles for a moment, unless Windows animations are off.
async fn swim(app: AppHandle) {
    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;
        if !tray_look().focused || !animations_enabled() {
            continue;
        }
        let Some(tray) = app.tray_by_id("main") else { continue };
        for frame in &TRAY_FOCUS[1..] {
            if let Ok(image) = tauri::image::Image::from_bytes(frame) {
                let _ = tray.set_icon(Some(image));
            }
            tokio::time::sleep(Duration::from_millis(160)).await;
        }
        // Back to the right icon, whatever changed meanwhile.
        update_tray(&app).await;
    }
}

/// Windows' "Animation effects" setting (Accessibility → Visual effects).
#[cfg(windows)]
fn animations_enabled() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION};
    let mut enabled: i32 = 1;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL to the pointer we pass.
    unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&mut enabled as *mut i32).cast(), 0) };
    enabled != 0
}

#[cfg(not(windows))]
fn animations_enabled() -> bool {
    true
}

fn post(app: &AppHandle, notices: &[Notice]) {
    for notice in notices {
        let (title, first, second) = notification_text(notice, i18n::translator());
        if let Err(error) = toast(app, &title, &first, &second, &notice.item_id) {
            eprintln!("Remora: notification failed: {error}");
        }
    }
}

/// A Windows toast that opens its item when clicked, as the inbox does. The notification plugin has no click
/// callback on desktop, so the toast is built directly, with the plugin's attribution: the app's identifier once
/// installed, PowerShell's while developing (an unregistered id would show nothing).
#[cfg(windows)]
fn toast(app: &AppHandle, title: &str, first: &str, second: &str, item_id: &str) -> Result<(), String> {
    use tauri_winrt_notification::Toast;
    let developing = tauri::utils::platform::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.ends_with("target/debug") || dir.ends_with("target/release")))
        .unwrap_or(false);
    let identifier = app.config().identifier.clone();
    let app_id = if developing { Toast::POWERSHELL_APP_ID } else { identifier.as_str() };
    let (handle, id) = (app.clone(), item_id.to_string());
    Toast::new(app_id)
        .title(title)
        .text1(first)
        .text2(second)
        .on_activated(move |_| {
            let (handle, id) = (handle.clone(), id.clone());
            tauri::async_runtime::spawn(async move {
                if let Err(error) = commands::open(&handle, &id, false).await {
                    eprintln!("Remora: opening {id} from a notification failed: {error}");
                }
            });
            Ok(())
        })
        .show()
        .map_err(|error| error.to_string())
}

/// Elsewhere (developing on a Mac or Linux) the plugin shows it; clicking it does nothing.
#[cfg(not(windows))]
fn toast(app: &AppHandle, title: &str, first: &str, second: &str, _item_id: &str) -> Result<(), String> {
    let body = [first, second].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n");
    app.notification().builder().title(title).body(body).show().map_err(|error| error.to_string())
}

/// The commands the window calls are exactly the ones declared in build.rs and granted by the capability.
#[cfg(test)]
mod command_permissions {
    fn handler_commands() -> Vec<String> {
        let source = include_str!("lib.rs");
        let start = source.find("generate_handler![").expect("the handler");
        let end = start + source[start..].find("])").expect("its end");
        source[start..end]
            .split("commands::")
            .skip(1)
            .map(|rest| rest.split(|c: char| !(c.is_alphanumeric() || c == '_')).next().unwrap_or_default().to_string())
            .collect()
    }

    #[test]
    fn every_command_is_declared_and_granted() {
        let commands = handler_commands();
        assert!(commands.len() > 20, "the handler was read");
        let declared = include_str!("../build.rs");
        let capability = include_str!("../capabilities/default.json");
        for command in &commands {
            assert!(declared.contains(&format!("\"{command}\"")), "{command} is not declared in build.rs");
            assert!(
                capability.contains(&format!("\"allow-{}\"", command.replace('_', "-"))),
                "{command} is not granted in capabilities/default.json"
            );
        }
    }
}
