//! Remora for Windows: the tray app. The inbox logic lives in `remora_app`; this crate only does what needs
//! the system: the tray icon and its popup, notifications, opening links, start at login, the reminder
//! shortcut, and the commands the interface (../src) calls.

mod commands;
mod i18n;

use remora_app::{demo, paths, CredentialVault, Inbox, JsonStore, Managed, MemoryVault, TrayCount};
use remora_core::Notice;
use remora_plugins::{HttpClient, ReqwestClient};
use std::sync::Arc;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_positioner::{Position, WindowExt};
use tokio::sync::{Mutex, Notify};

pub struct AppState {
    pub inbox: Mutex<Inbox>,
    pub http: Arc<dyn HttpClient>,
    pub refresh_now: Notify,
    pub demo: bool,
}

/// The shortcut that opens a new reminder from anywhere (tray icons can't be dragged on Windows).
pub const REMINDER_SHORTCUT: &str = "CommandOrControl+Alt+R";

const TRAY_QUIET: &[u8] = include_bytes!("../icons/tray-quiet.png");
const TRAY_ATTENTION: &[u8] = include_bytes!("../icons/tray-attention.png");

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
    let state = AppState { inbox: Mutex::new(inbox), http: Arc::new(ReqwestClient::new()), refresh_now: Notify::new(), demo: demo_mode };

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show(&main_window(app), false)))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
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
            commands::clear_done,
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
            commands::rename_account,
            commands::disconnect,
            commands::settings,
            commands::set_preferences,
            commands::set_autostart,
            commands::erase_local_data,
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
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Remora")
        .run(|_, event| {
            // A tray app keeps running when its popup closes.
            if let tauri::RunEvent::ExitRequested { api, code: None, .. } = event {
                api.prevent_exit();
            }
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
    }
}

fn new_reminder(app: &AppHandle) {
    show(&main_window(app), true);
    let _ = app.emit("show", "reminder");
}

fn build_tray(app: &AppHandle) -> tauri::Result<TrayIcon> {
    let t = i18n::translator();
    let open = MenuItem::with_id(app, "open", t("Open Remora"), true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, "refresh", t("Refresh"), true, None::<&str>)?;
    let reminder = MenuItem::with_id(app, "reminder", t("New reminder"), true, Some(REMINDER_SHORTCUT))?;
    let settings = MenuItem::with_id(app, "settings", t("Settings"), true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", t("Quit"), true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &refresh, &reminder, &settings, &separator, &quit])?;

    TrayIconBuilder::with_id("main")
        .icon(tauri::image::Image::from_bytes(TRAY_QUIET)?)
        .tooltip("Remora")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show(&main_window(app), true),
            "refresh" => app.state::<AppState>().refresh_now.notify_one(),
            "reminder" => new_reminder(app),
            "settings" => {
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
    loop {
        refresh(&app).await;
        let minutes = app.state::<AppState>().inbox.lock().await.preferences.refresh_minutes.max(1);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(u64::from(minutes) * 60);
        loop {
            let state = app.state::<AppState>();
            tokio::select! {
                _ = state.refresh_now.notified() => break,
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    let notices = state.inbox.lock().await.tick(chrono::Utc::now());
                    if !notices.is_empty() {
                        post(&app, &notices);
                        changed(&app).await;
                    }
                    if tokio::time::Instant::now() >= deadline { break }
                }
            }
        }
    }
}

pub async fn refresh(app: &AppHandle) {
    let state = app.state::<AppState>();
    if !state.demo {
        let jobs = state.inbox.lock().await.fetch_jobs(state.http.clone());
        let results = remora_app::run(jobs).await;
        let notices = state.inbox.lock().await.apply(results, chrono::Utc::now());
        post(app, &notices);
    }
    changed(app).await;
}

/// Tells the interface to reload, and updates the tray icon and its tooltip.
pub async fn changed(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (count, show) = {
        let inbox = state.inbox.lock().await;
        let layout = inbox.layout(chrono::Utc::now(), "");
        match inbox.preferences.tray_count {
            TrayCount::WaitingOnMe => (layout.action_count(), true),
            TrayCount::Everything => (layout.my_turn_items().count() + layout.waiting_count(), true),
            TrayCount::Hidden => (layout.action_count(), false),
        }
    };
    if let Some(tray) = app.tray_by_id("main") {
        let t = i18n::translator();
        let tooltip = match count {
            0 => format!("Remora · {}", t("Nothing needs you")),
            n => format!("Remora · {}", t("%d need you").replace("%d", &n.to_string())),
        };
        let _ = tray.set_tooltip(Some(if show { tooltip } else { "Remora".into() }));
        let icon = if count > 0 && show { TRAY_ATTENTION } else { TRAY_QUIET };
        if let Ok(image) = tauri::image::Image::from_bytes(icon) {
            let _ = tray.set_icon(Some(image));
        }
    }
    let _ = app.emit("inbox-changed", ());
}

fn post(app: &AppHandle, notices: &[Notice]) {
    let t = i18n::translator();
    for notice in notices {
        let body = [notice.subtitle.as_str(), notice.body.as_str()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n");
        if let Err(error) = app.notification().builder().title(t(&notice.title)).body(body).show() {
            eprintln!("Remora: notification failed: {error}");
        }
    }
}
