//! Widget windows: transparent, frameless WebView2 windows owned by the
//! desktop and kept just above the wallpaper (same Win32 code as the egui
//! edition). Positions persist in `config/opal-tauri.json`.

use std::time::Duration;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Window, WindowEvent};

use crate::config::{Layout, Placement};
use crate::{AppState, WIDGETS};

pub fn layout_file() -> std::path::PathBuf {
    crate::config::config_file("opal-tauri.json")
}

pub fn load_layout() -> Layout {
    std::fs::read_to_string(layout_file())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Widgets that start visible on a fresh install.
pub fn starts_open(id: &str) -> bool {
    matches!(id, "launcher" | "clock" | "notes" | "calendar" | "music")
}

/// Primary work area in logical pixels (x, y, w, h).
fn work_area(app: &AppHandle) -> (f64, f64, f64, f64) {
    let scale = app.primary_monitor().ok().flatten().map(|m| m.scale_factor()).unwrap_or(1.0);
    match crate::win::work_area() {
        Some((x, y, w, h)) => (x as f64 / scale, y as f64 / scale, w as f64 / scale, h as f64 / scale),
        None => (0.0, 0.0, 1600.0, 860.0),
    }
}

pub fn default_placement(app: &AppHandle, id: &str) -> Placement {
    let (wx, wy, ww, wh) = work_area(app);
    let m = 28.0;
    let (w, h): (f64, f64) = match id {
        "launcher" => (400.0, 290.0),
        "clock" => (360.0, 240.0),
        "notes" => (460.0, 400.0),
        "calendar" => (340.0, 360.0),
        "music" => (380.0, 160.0),
        "weather" => (360.0, 250.0),
        "system" => (340.0, 280.0),
        "pomodoro" => (300.0, 340.0),
        "photos" => (420.0, 300.0),
        "lunar" => (360.0, 260.0),
        "countdown" => (320.0, 320.0),
        _ => (340.0, 380.0),
    };
    let right = wx + ww - m;
    let (x, y) = match id {
        "launcher" => (wx + m, wy + m),
        "clock" => (right - w, wy + m),
        "calendar" => (right - w, wy + m + 260.0),
        "notes" => (right - 380.0 - w, wy + m),
        "music" => (wx + m, wy + wh - m - h),
        "weather" => (wx + m + 420.0, wy + m),
        "system" => (wx + m + 420.0, wy + m + 270.0),
        "pomodoro" => (wx + ww / 2.0 - w / 2.0, wy + wh / 2.0 - h / 2.0),
        "photos" => (wx + m, wy + m + 310.0),
        "lunar" => (wx + ww / 2.0 - w / 2.0, wy + m),
        "countdown" => (wx + ww / 2.0 - w / 2.0, wy + m + 280.0),
        _ => (wx + ww / 2.0 + 200.0, wy + wh / 2.0 - h / 2.0),
    };
    Placement { open: starts_open(id), x: x as f32, y: y as f32, w: w as f32, h: h as f32 }
}

pub fn placement(app: &AppHandle, id: &str) -> Placement {
    let found = {
        let st = app.state::<AppState>();
        let store = st.0.lock().unwrap();
        store.layout.widgets.get(id).copied()
    };
    found.unwrap_or_else(|| default_placement(app, id))
}

pub fn is_open(app: &AppHandle, id: &str) -> bool {
    placement(app, id).open
}

fn label(id: &str) -> String {
    format!("w-{id}")
}

/// Make a window a desktop widget: tool window owned by the desktop, sitting
/// just above the wallpaper.
fn attach_to_desktop(win: &WebviewWindow) {
    if let Ok(h) = win.hwnd() {
        crate::win::own_by_desktop(h.0 as isize);
        crate::win::pin_to_desktop(h.0 as isize);
    }
}

pub fn open_widget(app: &AppHandle, id: &str) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window(&label(id)) {
        let _ = w.show();
        return Ok(());
    }
    let p = placement(app, id);
    let name = WIDGETS.iter().find(|w| w.0 == id).map(|w| w.1).unwrap_or(id);
    let win = WebviewWindowBuilder::new(app, label(id), WebviewUrl::App(format!("index.html?w={id}").into()))
        .title(format!("Opal · {name}"))
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .skip_taskbar(true)
        .resizable(true)
        .min_inner_size(180.0, 110.0)
        .inner_size(p.w as f64, p.h as f64)
        .position(p.x as f64, p.y as f64)
        .visible(true)
        .build()?;
    attach_to_desktop(&win);
    Ok(())
}

/// Extra log lines while OPAL_RAISE_DEBUG is set (z-order troubleshooting).
pub fn debug(msg: &str) {
    if std::env::var_os("OPAL_RAISE_DEBUG").is_some() {
        log(msg);
    }
}

/// Last time each widget was used (pointer, keys, focus, move, resize).
static LAST_USE: std::sync::Mutex<Option<std::collections::HashMap<String, std::time::Instant>>> = std::sync::Mutex::new(None);

pub fn touch(label: &str) {
    LAST_USE.lock().unwrap().get_or_insert_with(Default::default).insert(label.to_string(), std::time::Instant::now());
}

fn idle_for(label: &str) -> Duration {
    LAST_USE
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(label))
        .map(|t| t.elapsed())
        .unwrap_or(Duration::MAX)
}

/// Settings "Sink back after" (seconds without use), default 10.
fn sink_delay(app: &AppHandle) -> f64 {
    let st = app.state::<AppState>();
    let store = st.0.lock().unwrap();
    store.state.settings.extra.get("sinkDelay").and_then(serde_json::Value::as_f64).unwrap_or(10.0).clamp(0.0, 60.0)
}

/// Append a line to config/opal-tauri.log (why widgets open and close).
pub fn log(msg: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(crate::config::config_file("opal-tauri.log")) {
        let _ = writeln!(f, "{} {msg}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    }
}

pub fn set_open(app: &AppHandle, id: &str, open: bool) {
    {
        let p = placement(app, id);
        let st = app.state::<AppState>();
        let mut store = st.0.lock().unwrap();
        store.layout.widgets.insert(id.to_string(), Placement { open, ..p });
        store.layout_dirty = true;
    }
    if open {
        let _ = open_widget(app, id);
    } else if let Some(w) = app.get_webview_window(&label(id)) {
        let _ = w.destroy();
    }
    let _ = tauri::Emitter::emit(app, "widgets-changed", ());
}

/// A panel window (widget settings / widgets manager): normal focusable
/// glass window near the widget, not pinned to the desktop.
pub fn open_panel(app: &AppHandle, key: &str, query: &str, size: (f64, f64), near: Option<&str>) -> tauri::Result<()> {
    let lbl = format!("p-{key}");
    if let Some(w) = app.get_webview_window(&lbl) {
        bring_forward(&w);
        return Ok(());
    }
    let (wx, wy, ww, wh) = work_area(app);
    let mut x = wx + ww / 2.0 - size.0 / 2.0;
    let mut y = wy + wh / 2.0 - size.1 / 2.0;
    if let Some(id) = near {
        let p = placement(app, id);
        x = p.x as f64 + p.w as f64 + 14.0;
        if x + size.0 > wx + ww {
            x = (p.x as f64 - size.0 - 14.0).max(wx);
        }
        y = (p.y as f64).min(wy + wh - size.1).max(wy);
    }
    let h = size.1.min(wh - 16.0);
    let win = WebviewWindowBuilder::new(app, lbl, WebviewUrl::App(format!("index.html?{query}").into()))
        .title("Opal")
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .skip_taskbar(true)
        .resizable(true)
        .inner_size(size.0, h)
        .position(x, y)
        .always_on_top(panels_on_top(app))
        .focused(true)
        .build()?;
    // An ordinary window (not owned by the desktop like the widgets, which
    // made it drop behind everything right after opening from the tray): it
    // comes up in front and stays until another window is clicked.
    bring_forward(&win);
    Ok(())
}

fn bring_forward(w: &WebviewWindow) {
    let _ = w.unminimize();
    let _ = w.show();
    if let Ok(h) = w.hwnd() {
        crate::win::raise(h.0 as isize);
    }
    let _ = w.set_focus();
}

/// Settings option "Panels float on top": the Widgets / Settings panels
/// stay above app windows instead of sitting with the widgets.
pub fn panels_on_top(app: &AppHandle) -> bool {
    let st = app.state::<AppState>();
    let store = st.0.lock().unwrap();
    store.state.settings.extra.get("panelsOnTop").and_then(serde_json::Value::as_bool).unwrap_or(false)
}

/// Apply the option to panels that are already open.
pub fn apply_panels_on_top(app: &AppHandle) {
    let on = panels_on_top(app);
    for (label, w) in app.webview_windows() {
        if label.starts_with("p-") {
            let _ = w.set_always_on_top(on);
        }
    }
}

pub fn on_window_event(window: &Window, event: &WindowEvent) {
    let lbl = window.label().to_string();
    if lbl == crate::ytm::LABEL {
        match event {
            // Closing the player only hides it, so the music keeps playing
            // (reopen from the music widget or the tray; "Stop YouTube
            // Music" in the tray really closes it).
            WindowEvent::CloseRequested { api, .. } if !QUITTING.load(std::sync::atomic::Ordering::SeqCst) => {
                api.prevent_close();
                let _ = window.hide();
            }
            WindowEvent::Destroyed => crate::ytm::closed(window.app_handle()),
            _ => {}
        }
        return;
    }
    let Some(id) = lbl.strip_prefix("w-") else { return };
    let app = window.app_handle();
    if matches!(event, WindowEvent::Focused(_) | WindowEvent::Moved(_) | WindowEvent::Resized(_)) {
        touch(&lbl);
    }
    if let WindowEvent::Focused(f) = event {
        if *f {
            if let Ok(h) = window.hwnd() {
                let ok = crate::win::raise(h.0 as isize);
                debug(&format!("{lbl}: focused -> raise {ok}"));
            }
        } else {
            debug(&format!("{lbl}: lost focus"));
        }
    }
    match event {
        WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
            let scale = window.scale_factor().unwrap_or(1.0);
            let (Ok(pos), Ok(size)) = (window.outer_position(), window.inner_size()) else { return };
            if size.width < 50 {
                return;
            }
            let pos: LogicalPosition<f64> = pos.to_logical(scale);
            let size: LogicalSize<f64> = size.to_logical(scale);
            let st = app.state::<AppState>();
            let mut store = st.0.lock().unwrap();
            store.layout.widgets.insert(
                id.to_string(),
                Placement { open: true, x: pos.x as f32, y: pos.y as f32, w: size.width as f32, h: size.height as f32 },
            );
            store.layout_dirty = true;
        }
        WindowEvent::CloseRequested { .. } => {
            // Alt+F4 on a focused widget hides it. Closes the user did not
            // ask for (quitting, sign-out, shutdown, Explorer restarts) leave
            // the widget marked open so it comes back next start.
            let focused = window.is_focused().unwrap_or(false);
            if QUITTING.load(std::sync::atomic::Ordering::SeqCst) || !focused {
                log(&format!("{id}: closed by the system, kept open for next start"));
                return;
            }
            log(&format!("{id}: hidden with Alt+F4"));
            let st = app.state::<AppState>();
            let mut store = st.0.lock().unwrap();
            if let Some(p) = store.layout.widgets.get_mut(id) {
                p.open = false;
            }
            store.layout_dirty = true;
        }
        _ => {}
    }
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    // Positions of this edition live beside the shared data files.
    {
        let st = app.state::<AppState>();
        st.0.lock().unwrap().layout = load_layout();
    }
    // Program catalogue for search (Start menu + Store apps).
    {
        let app2 = app.clone();
        std::thread::spawn(move || {
            let rx = crate::programs::load_async();
            while let Ok(list) = rx.recv() {
                let st = app2.state::<AppState>();
                st.0.lock().unwrap().programs = list;
            }
        });
    }
    for (id, _) in WIDGETS {
        if is_open(app, id) {
            open_widget(app, id)?;
        }
    }
    tray(app)?;

    // Same switches as the egui edition: --widgets, --settings,
    // --widget-settings <id>.
    let args: Vec<String> = std::env::args().collect();
    for (i, a) in args.iter().enumerate() {
        match a.as_str() {
            "--youtube-music" => {
                let app2 = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = crate::ytm::ytm_open(app2).await;
                });
            }
            "--widgets" => {
                let _ = open_panel(app, "widgets", "panel=widgets", (460.0, 820.0), None);
            }
            "--settings" => {
                let _ = open_panel(app, "settings", "panel=settings&w=launcher", (480.0, 640.0), Some("launcher"));
            }
            "--widget-settings" => {
                if let Some(id) = args.get(i + 1).filter(|id| WIDGETS.iter().any(|(w, _)| w == id)) {
                    let _ = open_panel(app, "settings", &format!("panel=settings&w={id}"), (480.0, 640.0), Some(id));
                }
            }
            _ => {}
        }
    }

    // Keep widgets just above the wallpaper, and save positions. A widget
    // in use stays where Windows put it (in front) until it has been left
    // alone for the "sink back" delay.
    let app2 = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(500));
        let delay = Duration::from_secs_f64(sink_delay(&app2));
        for (id, _) in WIDGETS {
            let lbl = label(id);
            if let Some(w) = app2.get_webview_window(&lbl) {
                let idle = idle_for(&lbl);
                if idle < Duration::from_secs(30) {
                    debug(&format!("{lbl}: idle {:.1}s, delay {:.1}s -> {}", idle.as_secs_f32(), delay.as_secs_f32(), if idle >= delay { "sink" } else { "stay" }));
                }
                if idle >= delay {
                    if let Ok(h) = w.hwnd() {
                        crate::win::pin_to_desktop(h.0 as isize);
                    }
                }
            }
        }
        let st = app2.state::<AppState>();
        let mut store = st.0.lock().unwrap();
        if store.layout_dirty {
            if crate::config::write_json(&layout_file(), &store.layout) {
                store.layout_dirty = false;
            }
        }
    });
    Ok(())
}

fn tray(app: &AppHandle) -> tauri::Result<()> {
    let widgets = MenuItem::with_id(app, "widgets", "Widgets…", true, None::<&str>)?;
    let folder = MenuItem::with_id(app, "folder", "Open config folder", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Opal", true, None::<&str>)?;
    let ytm_show = MenuItem::with_id(app, "ytm-show", "YouTube Music", true, None::<&str>)?;
    let ytm_stop = MenuItem::with_id(app, "ytm-stop", "Stop YouTube Music", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&widgets, &ytm_show, &ytm_stop, &sep, &folder, &sep2, &quit])?;
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?;
    TrayIconBuilder::new()
        .icon(icon)
        .tooltip("Opal (Tauri)")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "widgets" => {
                let _ = open_panel(app, "widgets", "panel=widgets", (460.0, 820.0), None);
            }
            "folder" => crate::win::open_folder(&crate::config::config_dir()),
            "ytm-show" => {
                let app2 = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = crate::ytm::ytm_open(app2).await;
                });
            }
            "ytm-stop" => {
                if let Some(w) = app.get_webview_window(crate::ytm::LABEL) {
                    let _ = w.destroy();
                }
            }
            "quit" => quit_app(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                let _ = open_panel(tray.app_handle(), "widgets", "panel=widgets", (460.0, 820.0), None);
            }
        })
        .build(app)?;
    Ok(())
}

/// Set while Opal is exiting, so closing every window does not record
/// each widget as hidden.
static QUITTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn quit_app(app: &AppHandle) {
    QUITTING.store(true, std::sync::atomic::Ordering::SeqCst);
    {
        let st = app.state::<AppState>();
        let mut store = st.0.lock().unwrap();
        let _ = crate::config::write_json(&layout_file(), &store.layout);
        store.layout_dirty = false;
    }
    app.exit(0);
}
