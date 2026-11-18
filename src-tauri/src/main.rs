#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod accounts;
mod audio;
mod desktop;
mod media;
mod net;
mod settings;
mod volume;

use std::sync::Mutex;

use serde::Serialize;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub(crate) struct AppState {
    pub(crate) settings: Mutex<settings::Settings>,
    peek: Mutex<bool>,
    hwnd: Mutex<isize>,
    wall_item: Mutex<Option<CheckMenuItem<tauri::Wry>>>,
}

#[derive(Serialize, Clone)]
struct UiState {
    wallpaper_mode: bool,
    autostart: bool,
    peek: bool,
}

fn ui_state(app: &AppHandle) -> UiState {
    let st = app.state::<AppState>();
    let wallpaper_mode = st.settings.lock().unwrap().wallpaper_mode;
    let peek = *st.peek.lock().unwrap();
    UiState { wallpaper_mode, autostart: settings::autostart_enabled(), peek }
}

/// Apply the wanted placement (peek > wallpaper > desktop).
fn apply_mode(app: &AppHandle) {
    let st = app.state::<AppState>();
    let h = *st.hwnd.lock().unwrap();
    if h == 0 {
        return;
    }
    let peek = *st.peek.lock().unwrap();
    let wall = st.settings.lock().unwrap().wallpaper_mode;
    let mode = if peek {
        desktop::Mode::Peek
    } else if wall {
        desktop::Mode::Wallpaper
    } else {
        desktop::Mode::Desktop
    };
    if let Err(e) = desktop::set_mode(h, mode) {
        let _ = desktop::set_mode(h, desktop::Mode::Desktop);
        let _ = app.emit("toast", format!("Couldn't go behind icons: {e}"));
    }
    if peek {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.set_focus();
        }
    }
    let _ = app.emit("mode-changed", ui_state(app));
}

fn set_peek(app: &AppHandle, on: bool) {
    {
        let st = app.state::<AppState>();
        let mut p = st.peek.lock().unwrap();
        if *p == on {
            return;
        }
        *p = on;
    }
    apply_mode(app);
}

fn set_wallpaper_mode_inner(app: &AppHandle, on: bool) {
    {
        let st = app.state::<AppState>();
        let mut s = st.settings.lock().unwrap();
        s.wallpaper_mode = on;
        settings::save(app, &s);
        let item = st.wall_item.lock().unwrap().clone();
        if let Some(item) = item {
            let _ = item.set_checked(on);
        }
    }
    apply_mode(app);
}

pub(crate) fn end_peek(app: &AppHandle) {
    set_peek(app, false);
    let _ = app.emit("peek-ended", ());
}

fn open_search(app: &AppHandle) {
    set_peek(app, true);
    let _ = app.emit("hotkey-search", ());
}

// ───────────────────────── commands ─────────────────────────

#[tauri::command]
fn get_state(app: AppHandle) -> UiState {
    ui_state(&app)
}

#[tauri::command]
fn set_wallpaper_mode(app: AppHandle, on: bool) -> UiState {
    set_wallpaper_mode_inner(&app, on);
    ui_state(&app)
}

#[tauri::command]
fn peek(app: AppHandle, on: bool) {
    set_peek(&app, on);
}

#[tauri::command]
async fn get_volume() -> Result<(f32, bool), String> {
    tauri::async_runtime::spawn_blocking(volume::get).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn set_volume(value: f32) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || volume::set(value)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn set_mute(on: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || volume::mute(on)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn reveal_taskbar(app: AppHandle) {
    let h = *app.state::<AppState>().hwnd.lock().unwrap();
    desktop::reveal_taskbar(h);
}

#[derive(Serialize)]
struct MonitorsOut {
    monitors: Vec<desktop::Monitor>,
    current: String,
}

#[tauri::command]
fn list_monitors(app: AppHandle) -> MonitorsOut {
    let current = app.state::<AppState>().settings.lock().unwrap().monitor.clone();
    MonitorsOut { monitors: desktop::monitors(), current }
}

fn set_monitor_inner(app: &AppHandle, id: String) {
    {
        let st = app.state::<AppState>();
        let mut s = st.settings.lock().unwrap();
        s.monitor = id.clone();
        settings::save(app, &s);
    }
    desktop::set_target(&id);
    apply_mode(app);
}

#[tauri::command]
fn set_monitor(app: AppHandle, id: String) {
    set_monitor_inner(&app, id);
}

#[tauri::command]
fn open_external(url: String) {
    desktop::open_external(&url);
}

#[tauri::command]
async fn media_control(action: String, value: Option<f64>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || media::control(action, value))
        .await
        .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
struct SearchOut {
    source: String,
    needs_login: bool,
    error: Option<String>,
    tracks: Vec<net::Track>,
}

/// Search on the chosen platform.
#[tauri::command]
async fn search(app: AppHandle, platform: String, query: String) -> SearchOut {
    let ok = |source: &str, r: Result<Vec<net::Track>, String>| match r {
        Ok(tracks) => SearchOut { source: source.into(), needs_login: false, error: None, tracks },
        Err(e) => SearchOut { source: source.into(), needs_login: false, error: Some(e), tracks: vec![] },
    };
    match platform.as_str() {
        "ytmusic" | "auto" => ok("ytmusic", net::ytm_search(&query).await),
        "youtube" => ok(
            "youtube",
            net::yt_search(&query).await.map(|v| {
                v.into_iter()
                    .map(|mut t| {
                        t.url = format!("https://www.youtube.com/watch?v={}", t.id);
                        t.source = "youtube".into();
                        t
                    })
                    .collect()
            }),
        ),
        "soundcloud" => ok("soundcloud", net::sc_search(&query).await),
        "spotify" | "spotifyweb" => match accounts::spotify_search(&app, &query).await {
            Ok(tracks) => SearchOut { source: "spotify".into(), needs_login: false, error: None, tracks },
            Err(e) => {
                let mut out = ok("apple", net::search_music(&query).await);
                out.needs_login = e == "not_connected";
                if !out.needs_login {
                    out.error = Some(e);
                }
                out
            }
        },
        _ => ok("apple", net::search_music(&query).await),
    }
}

#[tauri::command]
async fn account_status(app: AppHandle) -> accounts::Accounts {
    accounts::status(&app).await
}

#[tauri::command]
async fn account_login(app: AppHandle, platform: String) -> Result<(), String> {
    accounts::login(app, platform).await
}

#[tauri::command]
async fn account_logout(app: AppHandle, platform: String) -> Result<(), String> {
    accounts::logout(app, platform).await
}

#[tauri::command]
async fn spotify_connect(app: AppHandle, client_id: String) -> Result<(), String> {
    accounts::spotify_connect(app, client_id).await
}

#[tauri::command]
async fn spotify_play(app: AppHandle, uri: String) -> Result<String, String> {
    accounts::spotify_play(&app, &uri).await
}

#[tauri::command]
async fn youtube_search(query: String) -> Result<Vec<String>, String> {
    net::youtube_search(&query).await
}

#[tauri::command]
async fn lyrics(artist: String, title: String, duration: Option<f64>) -> Result<Option<net::Lyrics>, String> {
    net::lyrics(&artist, &title, duration).await
}

#[tauri::command]
async fn lyrics_search(query: String) -> Result<Vec<net::Lyrics>, String> {
    net::lyrics_search(&query).await
}

#[tauri::command]
async fn lyrics_page(url: String) -> Result<String, String> {
    net::genius_page(&url).await
}

#[tauri::command]
async fn translate(lines: Vec<String>, target: String) -> Result<net::Translated, String> {
    net::translate(lines, &target).await
}

#[tauri::command]
async fn fetch_image(url: String) -> Result<String, String> {
    net::fetch_image(&url).await
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

// ───────────────────────── main ─────────────────────────

fn main() {
    desktop::set_app_id();

    let hotkey = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::Space);
    let hotkey_ui = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyH);

    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, sc, ev| {
                    if ev.state() != ShortcutState::Pressed {
                        return;
                    }
                    if sc == &hotkey {
                        open_search(app);
                    } else if sc == &hotkey_ui {
                        let _ = app.emit("hotkey-action", "h");
                    }
                })
                .build(),
        )
        .manage(AppState {
            settings: Mutex::new(settings::Settings::default()),
            peek: Mutex::new(false),
            hwnd: Mutex::new(0),
            wall_item: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            set_wallpaper_mode,
            peek,
            open_external,
            reveal_taskbar,
            get_volume,
            set_volume,
            set_mute,
            list_monitors,
            set_monitor,
            media_control,
            search,
            account_status,
            account_login,
            account_logout,
            spotify_connect,
            spotify_play,
            youtube_search,
            lyrics,
            lyrics_search,
            fetch_image,
            lyrics_page,
            translate,
            quit_app
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            // settings
            let s = settings::load(&handle);
            let wall_on = s.wallpaper_mode;
            desktop::set_target(&s.monitor);
            *app.state::<AppState>().settings.lock().unwrap() = s;

            // window → desktop layer
            let win = app.get_webview_window("main").expect("main window");
            let raw = win.hwnd().map(|h| h.0 as isize).unwrap_or(0);
            *app.state::<AppState>().hwnd.lock().unwrap() = raw;
            if raw != 0 {
                desktop::install(raw);
                desktop::spawn_watcher(raw);
            }
            apply_mode(&handle);

            // leave peek when focus goes elsewhere
            let h2 = handle.clone();
            win.on_window_event(move |e| {
                if let WindowEvent::Focused(false) = e {
                    if *h2.state::<AppState>().peek.lock().unwrap() {
                        set_peek(&h2, false);
                        let _ = h2.emit("peek-ended", ());
                    }
                }
            });

            // tray
            let search_i = MenuItem::with_id(app, "search", "Search music   Ctrl+Alt+Space", true, None::<&str>)?;
            let wall_i = CheckMenuItem::with_id(app, "wallpaper", "Behind desktop icons", true, wall_on, None::<&str>)?;
            let auto_i = CheckMenuItem::with_id(
                app,
                "autostart",
                "Start with Windows",
                true,
                settings::autostart_enabled(),
                None::<&str>,
            )?;
            let display_i = MenuItem::with_id(app, "display", "Move to next display", true, None::<&str>)?;
            let reload_i = MenuItem::with_id(app, "reload", "Reload skin", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit Wallplayer", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(app, &[&search_i, &wall_i, &display_i, &auto_i, &sep, &reload_i, &quit_i])?;
            *app.state::<AppState>().wall_item.lock().unwrap() = Some(wall_i.clone());

            let auto_c = auto_i.clone();
            let mut tray = TrayIconBuilder::with_id("wallplayer")
                .tooltip("Wallplayer")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, ev| match ev.id().as_ref() {
                    "search" => open_search(app),
                    "wallpaper" => {
                        let on = !app.state::<AppState>().settings.lock().unwrap().wallpaper_mode;
                        set_wallpaper_mode_inner(app, on);
                    }
                    "autostart" => {
                        let on = !settings::autostart_enabled();
                        let _ = settings::set_autostart(on);
                        let _ = auto_c.set_checked(settings::autostart_enabled());
                    }
                    "display" => {
                        let list = desktop::monitors();
                        if list.len() > 1 {
                            let cur = app.state::<AppState>().settings.lock().unwrap().monitor.clone();
                            let i = list.iter().position(|m| m.id == cur).or_else(|| list.iter().position(|m| m.primary)).unwrap_or(0);
                            let next = list[(i + 1) % list.len()].id.clone();
                            set_monitor_inner(app, next);
                            let _ = app.emit("monitor-changed", ());
                        }
                    }
                    "reload" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.eval("location.reload()");
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, ev| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = ev
                    {
                        open_search(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            // hotkey
            for hk in [hotkey, hotkey_ui] {
                if let Err(e) = app.global_shortcut().register(hk) {
                    eprintln!("hotkey: {e}");
                }
            }

            // native feeds
            media::spawn(handle.clone());
            audio::spawn(handle.clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Wallplayer");
}
