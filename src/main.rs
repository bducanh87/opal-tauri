#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Opal, Tauri edition. The Rust core (configuration files, lunar
//! calendar, program search, Win32 desktop pinning, weather, ICS, system
//! monitor, media sessions and the theme / design catalogue) started as a
//! copy of the egui edition (D:\Project\opal) and now lives here. The
//! interface is plain HTML/CSS/JS in `ui/`, one transparent WebView2 window
//! per widget.

#[allow(dead_code)]
mod config;
#[allow(dead_code)]
mod lunar;
#[allow(dead_code)]
mod programs;
#[allow(dead_code)]
mod theme;
#[allow(dead_code)]
mod win;
#[allow(dead_code)]
mod ics;
#[allow(dead_code)]
mod weather_api;
#[allow(dead_code)]
mod sysmon;

mod audio;
mod commands;
mod media;
mod windows_mgr;
mod ytm;

use std::sync::Mutex;

/// Shared, persisted data plus runtime caches.
pub struct Store {
    pub state: config::State,
    pub notes: config::NotesFile,
    pub calendar: config::CalendarConfig,
    pub layout: config::Layout,
    pub extras: config::Extras,
    pub programs: Vec<programs::Program>,
    pub icons: std::collections::HashMap<String, Option<String>>,
    pub layout_dirty: bool,
}

pub struct AppState(pub Mutex<Store>);

pub const WIDGETS: [(&str, &str); 12] = [
    ("launcher", "App Dock"),
    ("clock", "Clock"),
    ("notes", "Sticky notes"),
    ("calendar", "Calendar"),
    ("music", "Music"),
    ("weather", "Weather"),
    ("system", "System"),
    ("pomodoro", "Focus timer"),
    ("photos", "Photos"),
    ("lunar", "Âm lịch"),
    ("todo", "To-do"),
    ("countdown", "Đếm ngược"),
];

fn main() {
    // One edition at a time: the egui and Tauri builds share a lock.
    match win::claim_instance("Local\\OpalTauri") {
        win::Instance::First => {}
        win::Instance::SameRunning => return,
        win::Instance::OtherRunning => {
            win::message_box(
                "Opal",
                "Opal bản egui đang chạy.\n\nMỗi lúc chỉ chạy một bản: thoát bản egui (chuột phải icon Opal ở khay hệ thống → Quit Opal) rồi mở lại bản này.",
            );
            return;
        }
    }
    let mut state = config::State::load();
    if !state.settings.seeded {
        seed_defaults(&mut state);
        state.save();
    }
    let store = Store {
        state,
        notes: config::NotesFile::load(),
        calendar: config::CalendarConfig::load(),
        layout: config::Layout::load(),
        extras: config::Extras::load(),
        programs: Vec::new(),
        icons: Default::default(),
        layout_dirty: false,
    };

    tauri::Builder::default()
        .manage(AppState(Mutex::new(store)))
        .manage(media::Media::start())
        .manage(commands::SystemMonitor::default())
        .manage(ytm::Ytm::default())
        .manage(audio::Audio::default())
        .invoke_handler(tauri::generate_handler![
            commands::boot,
            commands::get_state,
            commands::save_settings,
            commands::launch_item,
            commands::add_paths,
            commands::pick_and_add,
            commands::add_place,
            commands::remove_item,
            commands::rename_item,
            commands::move_item,
            commands::reveal_item,
            commands::icon,
            commands::search_programs,
            commands::open_program,
            commands::pin_program,
            commands::get_notes,
            commands::save_notes,
            commands::get_extras,
            commands::save_extras,
            commands::pick_folder,
            commands::list_photos,
            commands::photo,
            commands::lunar_today,
            commands::countdowns,
            commands::weather,
            commands::system_stats,
            commands::media_now,
            commands::media_command,
            commands::open_player,
            commands::wallpaper,
            commands::widgets_list,
            commands::set_widget_open,
            commands::open_widget_settings,
            commands::open_widgets_panel,
            commands::close_self,
            commands::widget_touch,
            commands::audio_levels,
            commands::config_folder,
            commands::quit,
            commands::calendar_get,
            commands::calendar_save,
            commands::beep,
            ytm::ytm_report,
            ytm::ytm_now,
            ytm::ytm_open,
            ytm::ytm_control,
        ])
        .setup(|app| {
            windows_mgr::setup(app.handle())?;
            Ok(())
        })
        .on_window_event(windows_mgr::on_window_event)
        .run(tauri::generate_context!())
        .expect("failed to start Opal");
}

/// First run: a few familiar shortcuts, like the egui edition.
fn seed_defaults(state: &mut config::State) {
    use std::path::Path;
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let candidates = [
        (r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe".to_string(), "Edge", "app"),
        (format!(r"{windir}\explorer.exe"), "File Explorer", "app"),
        ("ms-settings:".to_string(), "Settings", "url"),
        (format!(r"{windir}\System32\notepad.exe"), "Notepad", "app"),
        (format!(r"{windir}\System32\calc.exe"), "Calculator", "app"),
    ];
    if state.items.is_empty() {
        for (path, name, kind) in candidates {
            if kind == "url" || Path::new(&path).exists() {
                state.items.push(config::Item::new(name.into(), path, kind));
            }
        }
    }
    state.settings.seeded = true;
}
