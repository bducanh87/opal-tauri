//! Tauri commands called from the web UI. Logic lives in the shared core
//! (config, lunar, programs, ics, weather_api, sysmon, win, theme).

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

use crate::config::{self, Item, Note};
use crate::media::{Command, Media, Snapshot};
use crate::theme::{self, Base, Radial};
use crate::{AppState, WIDGETS, windows_mgr};

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn emit_state(app: &AppHandle) {
    let _ = app.emit("state-changed", ());
}

// ------------------------------------------------------------------ boot --

fn hex(c: u32) -> String {
    format!("#{c:06x}")
}

/// The theme / design surface as a CSS background, matching theme.rs.
fn css_background(base: Base, radials: &[Radial]) -> String {
    let mut layers: Vec<String> = radials
        .iter()
        .map(|r| format!("radial-gradient(ellipse at {:.0}% {:.0}%, {}, transparent {:.0}%)", r.x * 100.0, r.y * 100.0, hex(r.color), r.stop * 100.0))
        .collect();
    layers.push(match base {
        Base::Solid(c) => format!("linear-gradient({0}, {0})", hex(c)),
        Base::Linear(deg, a, b) => format!("linear-gradient({deg:.0}deg, {}, {})", hex(a), hex(b)),
    });
    layers.join(", ")
}

#[derive(Serialize)]
struct ThemeDto {
    id: &'static str,
    name: &'static str,
    css: String,
    light: bool,
}

#[derive(Serialize)]
struct DesignDto {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    /// Own surface (paper, film…) when the design brings one.
    surface: Option<String>,
    light: bool,
}

fn themes() -> Vec<ThemeDto> {
    theme::THEMES
        .iter()
        .map(|t| ThemeDto { id: t.id, name: t.name, css: css_background(t.base, t.radials), light: t.light })
        .collect()
}

fn designs(widget: &str) -> Vec<DesignDto> {
    theme::designs(widget)
        .iter()
        .map(|d| {
            let surface = theme::design_surface(widget, d.id);
            DesignDto {
                id: d.id,
                name: d.name,
                description: d.description,
                light: surface.map(|s| s.2).unwrap_or(false),
                surface: surface.map(|(b, r, _)| css_background(b, r)),
            }
        })
        .collect()
}

#[tauri::command]
pub fn boot(window: WebviewWindow, st: State<AppState>, widget: Option<String>) -> Value {
    let store = st.0.lock().unwrap();
    let w = widget.unwrap_or_else(|| window.label().trim_start_matches("w-").to_string());
    json!({
        "widget": w,
        "settings": store.state.settings,
        "designs": designs(&w),
        "themes": themes(),
        "widgets": WIDGETS.iter().map(|(id, name)| json!({"id": id, "name": name, "designs": designs(id)})).collect::<Vec<_>>(),
    })
}

#[tauri::command]
pub fn get_state(st: State<AppState>) -> Value {
    let store = st.0.lock().unwrap();
    json!({ "items": store.state.items, "settings": store.state.settings })
}

#[tauri::command]
pub fn save_settings(app: AppHandle, st: State<AppState>, settings: Value) -> Result<(), String> {
    let mut parsed: config::Settings = serde_json::from_value(settings).map_err(|e| e.to_string())?;
    parsed.sanitize();
    let changed_startup;
    {
        let mut store = st.0.lock().unwrap();
        changed_startup = parsed.start_with_windows != store.state.settings.start_with_windows;
        store.state.settings = parsed.clone();
        store.state.save();
    }
    if changed_startup {
        crate::win::set_start_with_windows(parsed.start_with_windows)?;
    }
    windows_mgr::apply_panels_on_top(&app);
    let _ = app.emit("settings-changed", &parsed);
    Ok(())
}

// --------------------------------------------------------------- launcher --

fn launch(item: &Item) -> Result<(), String> {
    if item.kind == "aumid" {
        crate::win::shell_open(&format!("shell:AppsFolder\\{}", item.target), None, None)
    } else if item.kind == "shell" {
        crate::win::shell_open("explorer.exe", Some(&item.target), None)
    } else {
        crate::win::shell_open(&item.target, item.args.as_deref(), item.cwd.as_deref())
    }
}

#[tauri::command]
pub fn launch_item(app: AppHandle, st: State<AppState>, id: String) -> Result<(), String> {
    let item = {
        let store = st.0.lock().unwrap();
        store.state.items.iter().find(|i| i.id == id).cloned()
    };
    let Some(item) = item else { return Err("Item not found".into()) };
    launch(&item)?;
    let mut store = st.0.lock().unwrap();
    if let Some(i) = store.state.items.iter_mut().find(|i| i.id == id) {
        i.launch_count += 1;
        i.last_used = Some(config::now_ms());
    }
    store.state.save();
    drop(store);
    emit_state(&app);
    Ok(())
}

fn item_for_path(path: &std::path::Path) -> Item {
    let target = path.to_string_lossy().to_string();
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    if path.is_dir() {
        Item::new(crate::programs::pretty_name(&target), target, "folder")
    } else if ext == "url" {
        let url = std::fs::read_to_string(path)
            .ok()
            .and_then(|t| t.lines().find_map(|l| l.trim().strip_prefix("URL=").map(|u| u.trim().to_string())));
        match url {
            Some(u) => {
                let mut i = Item::new(stem, u, "url");
                i.icon_source = Some(target);
                i
            }
            None => Item::new(stem, target, "file"),
        }
    } else if ext == "exe" {
        Item::new(crate::programs::pretty_name(&target), target, "app")
    } else {
        Item::new(stem, target, "file")
    }
}

#[tauri::command]
pub fn add_paths(app: AppHandle, st: State<AppState>, paths: Vec<String>) {
    let mut store = st.0.lock().unwrap();
    for p in paths {
        if store.state.items.iter().any(|i| i.target.eq_ignore_ascii_case(&p)) {
            continue;
        }
        store.state.items.push(item_for_path(std::path::Path::new(&p)));
    }
    store.state.save();
    drop(store);
    emit_state(&app);
}

#[tauri::command]
pub async fn pick_and_add(app: AppHandle) -> Result<(), String> {
    let files = rfd::AsyncFileDialog::new().set_title("Add to Opal").pick_files().await;
    if let Some(files) = files {
        let paths = files.iter().map(|f| f.path().to_string_lossy().to_string()).collect();
        add_paths(app.clone(), app.state::<AppState>(), paths);
    }
    Ok(())
}

#[tauri::command]
pub fn add_place(app: AppHandle, st: State<AppState>, place: String) {
    let (name, target) = match place.as_str() {
        "recycle" => ("Recycle Bin", "::{645FF040-5081-101B-9F08-00AA002F954E}"),
        _ => ("This PC", "::{20D04FE0-3AEA-1069-A2D8-08002B30309D}"),
    };
    let mut store = st.0.lock().unwrap();
    if !store.state.items.iter().any(|i| i.target == target) {
        let mut item = Item::new(name.into(), target.into(), "shell");
        item.icon_source = Some(target.into());
        store.state.items.push(item);
        store.state.save();
    }
    drop(store);
    emit_state(&app);
}

#[tauri::command]
pub fn remove_item(app: AppHandle, st: State<AppState>, id: String) {
    let mut store = st.0.lock().unwrap();
    store.state.items.retain(|i| i.id != id);
    store.state.save();
    drop(store);
    emit_state(&app);
}

#[tauri::command]
pub fn rename_item(app: AppHandle, st: State<AppState>, id: String, name: String) {
    let mut store = st.0.lock().unwrap();
    if let Some(i) = store.state.items.iter_mut().find(|i| i.id == id) {
        if !name.trim().is_empty() {
            i.name = name.trim().to_string();
        }
    }
    store.state.save();
    drop(store);
    emit_state(&app);
}

#[tauri::command]
pub fn move_item(app: AppHandle, st: State<AppState>, from: usize, to: usize) {
    let mut store = st.0.lock().unwrap();
    let n = store.state.items.len();
    if from < n && to < n {
        let item = store.state.items.remove(from);
        store.state.items.insert(to, item);
        store.state.save();
    }
    drop(store);
    emit_state(&app);
}

#[tauri::command]
pub fn reveal_item(st: State<AppState>, id: String) {
    let store = st.0.lock().unwrap();
    if let Some(i) = store.state.items.iter().find(|i| i.id == id) {
        crate::win::reveal(&i.target);
    }
}

/// Icon for an item / program as a PNG data URL. Inbox apps prefer their
/// Store logo, like the egui edition.
#[tauri::command]
pub async fn icon(app: AppHandle, path: String, name: Option<String>) -> Option<String> {
    let st = app.state::<AppState>();
    let path = {
        let store = st.0.lock().unwrap();
        name.as_ref()
            .and_then(|n| store.programs.iter().find(|p| p.kind == "aumid" && p.name.eq_ignore_ascii_case(n)))
            .map(|p| p.icon_path())
            .unwrap_or(path)
    };
    if let Some(hit) = st.0.lock().unwrap().icons.get(&path) {
        return hit.clone();
    }
    let p2 = path.clone();
    let url = tauri::async_runtime::spawn_blocking(move || {
        crate::win::com_init();
        let (w, h, rgba) = crate::win::extract_icon(&p2, 96)?;
        let img = image::RgbaImage::from_raw(w as u32, h as u32, rgba)?;
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).ok()?;
        Some(format!("data:image/png;base64,{}", b64(&out.into_inner())))
    })
    .await
    .ok()
    .flatten();
    st.0.lock().unwrap().icons.insert(path, url.clone());
    url
}

#[derive(Serialize)]
pub struct ProgramDto {
    name: String,
    target: String,
    kind: &'static str,
    icon_path: String,
}

#[tauri::command]
pub fn search_programs(st: State<AppState>, query: String) -> Vec<ProgramDto> {
    let store = st.0.lock().unwrap();
    crate::programs::search(&store.programs, &query, 24)
        .into_iter()
        .map(|p| ProgramDto { name: p.name.clone(), target: p.target.clone(), kind: p.kind, icon_path: p.icon_path() })
        .collect()
}

#[tauri::command]
pub fn open_program(target: String, kind: String) -> Result<(), String> {
    let t = if kind == "aumid" { format!("shell:AppsFolder\\{target}") } else { target };
    crate::win::shell_open(&t, None, None)
}

#[tauri::command]
pub fn pin_program(app: AppHandle, st: State<AppState>, name: String, target: String, kind: String, icon_path: String) {
    let mut store = st.0.lock().unwrap();
    if !store.state.items.iter().any(|i| i.target.eq_ignore_ascii_case(&target)) {
        let mut item = Item::new(name, target, if kind == "aumid" { "aumid" } else { "file" });
        item.icon_source = Some(icon_path);
        store.state.items.push(item);
        store.state.save();
    }
    drop(store);
    emit_state(&app);
}

// ------------------------------------------------------------------ notes --

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteDto {
    id: String,
    text: String,
    color: String,
    pinned: bool,
    created_at: i64,
    updated_at: i64,
}

#[tauri::command]
pub fn get_notes(st: State<AppState>) -> Vec<NoteDto> {
    let store = st.0.lock().unwrap();
    store
        .notes
        .notes
        .iter()
        .map(|n| NoteDto { id: n.id.clone(), text: n.text.clone(), color: n.color.clone(), pinned: n.pinned, created_at: n.created_at, updated_at: n.updated_at })
        .collect()
}

#[tauri::command]
pub fn save_notes(st: State<AppState>, notes: Vec<NoteDto>) -> bool {
    let mut store = st.0.lock().unwrap();
    // Keep the stored HTML (formatting from the original Lumen) unless the
    // plain text was edited here.
    let old: std::collections::HashMap<String, (String, String)> =
        store.notes.notes.iter().map(|n| (n.id.clone(), (n.text.clone(), n.body.clone()))).collect();
    store.notes.notes = notes
        .into_iter()
        .map(|d| Note {
            body: match old.get(&d.id) {
                Some((text, body)) if *text == d.text => body.clone(),
                _ => config::text_to_html(&d.text),
            },
            id: d.id,
            text: d.text,
            color: d.color,
            pinned: d.pinned,
            created_at: d.created_at,
            updated_at: d.updated_at,
        })
        .collect();
    store.notes.save()
}

// ----------------------------------------------------------------- extras --

#[tauri::command]
pub fn get_extras(st: State<AppState>) -> config::Extras {
    st.0.lock().unwrap().extras.clone()
}

#[tauri::command]
pub fn save_extras(app: AppHandle, st: State<AppState>, extras: config::Extras) -> bool {
    let ok = {
        let mut store = st.0.lock().unwrap();
        store.extras = extras;
        store.extras.save()
    };
    let _ = app.emit("extras-changed", ());
    ok
}

#[tauri::command]
pub async fn pick_folder() -> Option<String> {
    rfd::AsyncFileDialog::new()
        .set_title("Choose a folder")
        .pick_folder()
        .await
        .map(|f| f.path().to_string_lossy().to_string())
}

// ----------------------------------------------------------------- photos --

fn is_image(p: &std::path::Path) -> bool {
    matches!(
        p.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref(),
        Some("jpg" | "jpeg" | "png" | "webp" | "bmp")
    )
}

fn scan(dir: &std::path::Path, out: &mut Vec<String>, depth: usize) {
    if depth > 3 || out.len() > 5000 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            scan(&p, out, depth + 1);
        } else if is_image(&p) {
            out.push(p.to_string_lossy().to_string());
        }
    }
}

#[tauri::command]
pub async fn list_photos(folder: String) -> Vec<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut out = Vec::new();
        scan(std::path::Path::new(&folder), &mut out, 0);
        let mut seed: u64 = 0x2545_f491_4f6c_dd1d ^ out.len() as u64;
        for i in (1..out.len()).rev() {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            out.swap(i, (seed % (i as u64 + 1)) as usize);
        }
        out
    })
    .await
    .unwrap_or_default()
}

/// A photo scaled for display, as a JPEG data URL.
#[tauri::command]
pub async fn photo(path: String) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let img = image::ImageReader::open(&path).ok()?.with_guessed_format().ok()?.decode().ok()?;
        let img = if img.width() > 1400 || img.height() > 1400 { img.thumbnail(1400, 1400) } else { img };
        let mut out = std::io::Cursor::new(Vec::new());
        img.to_rgb8().write_to(&mut out, image::ImageFormat::Jpeg).ok()?;
        Some(format!("data:image/jpeg;base64,{}", b64(&out.into_inner())))
    })
    .await
    .ok()
    .flatten()
}

// -------------------------------------------------------------- countdown --

/// Next solar date (>= today) of a yearly solar or lunar day/month.
fn next_occurrence(today: chrono::NaiveDate, day: u32, month: u32, lunar: bool) -> Option<chrono::NaiveDate> {
    use chrono::{Datelike, NaiveDate};
    if lunar {
        let ly = crate::lunar::solar_to_lunar(today.day() as i64, today.month() as i64, today.year() as i64).year;
        for y in [ly - 1, ly, ly + 1, ly + 2] {
            // A 30th in a 29-day month falls back to the 29th.
            for d in [day as i64, day as i64 - 1] {
                let Some((sd, sm, sy)) = crate::lunar::lunar_to_solar(d, month as i64, y, false) else { continue };
                let l = crate::lunar::solar_to_lunar(sd, sm, sy);
                if l.month != month as i64 || l.leap {
                    continue;
                }
                let date = NaiveDate::from_ymd_opt(sy as i32, sm as u32, sd as u32)?;
                if date >= today {
                    return Some(date);
                }
                break;
            }
        }
        None
    } else {
        for y in [today.year(), today.year() + 1] {
            let date = NaiveDate::from_ymd_opt(y, month, day).or_else(|| NaiveDate::from_ymd_opt(y, month, day.saturating_sub(1)))?;
            if date >= today {
                return Some(date);
            }
        }
        None
    }
}

/// Upcoming dates for the countdown widget, nearest first.
#[tauri::command]
pub fn countdowns(st: State<AppState>) -> Value {
    use chrono::Datelike;
    let ex = st.0.lock().unwrap().extras.clone();
    let today = chrono::Local::now().date_naive();
    let mut out: Vec<Value> = Vec::new();
    let mut push = |id: &str, name: String, kind: &str, date: chrono::NaiveDate, lunar_label: Option<String>| {
        let l = crate::lunar::solar_to_lunar(date.day() as i64, date.month() as i64, date.year() as i64);
        out.push(json!({
            "id": id, "name": name, "kind": kind,
            "date": date.format("%Y-%m-%d").to_string(),
            "days": (date - today).num_days(),
            "weekday": date.weekday().num_days_from_monday(),
            "lunar": lunar_label.unwrap_or_else(|| format!("{}/{} âm", l.day, l.month)),
        }));
    };
    if ex.countdown_show_tet {
        if let Some(d) = next_occurrence(today, 1, 1, true) {
            let l = crate::lunar::solar_to_lunar(d.day() as i64, d.month() as i64, d.year() as i64);
            push("tet", format!("Tết {}", crate::lunar::year_can_chi(l.year)), "tet", d, Some("Mùng 1 tháng Giêng".into()));
        }
    }
    if ex.countdown_show_moon {
        // The next Mùng 1 and Rằm within the coming month.
        let (mut mung1, mut ram) = (None, None);
        for i in 0..32 {
            let d = today + chrono::Duration::days(i);
            let l = crate::lunar::solar_to_lunar(d.day() as i64, d.month() as i64, d.year() as i64);
            if l.day == 1 && mung1.is_none() {
                mung1 = Some((d, l));
            }
            if l.day == 15 && ram.is_none() {
                ram = Some((d, l));
            }
        }
        if let Some((d, l)) = mung1 {
            push("mung1", format!("Mùng 1 tháng {}", crate::lunar::month_name(l.month)), "moon", d, Some("Sóc · trăng non".into()));
        }
        if let Some((d, l)) = ram {
            push("ram", format!("Rằm tháng {}", crate::lunar::month_name(l.month)), "moon", d, Some("Vọng · trăng tròn".into()));
        }
    }
    for c in &ex.countdowns {
        if let Some(d) = next_occurrence(today, c.day, c.month, c.lunar) {
            let label = if c.lunar { Some(format!("{}/{} âm lịch", c.day, c.month)) } else { None };
            push(&c.id, c.name.clone(), "custom", d, label);
        }
    }
    out.sort_by_key(|v| v["days"].as_i64().unwrap_or(i64::MAX));
    json!(out)
}

#[cfg(test)]
mod countdown_tests {
    use super::next_occurrence;
    use chrono::NaiveDate;

    #[test]
    fn next_tet_and_lunar_dates() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 10).unwrap();
        assert_eq!(next_occurrence(today, 1, 1, true), NaiveDate::from_ymd_opt(2027, 2, 6));
        // Rằm tháng Giêng 2027 = 20/2/2027.
        assert_eq!(next_occurrence(today, 15, 1, true), NaiveDate::from_ymd_opt(2027, 2, 20));
        // Solar: 1/1 next year; today counts as 0 days.
        assert_eq!(next_occurrence(today, 1, 1, false), NaiveDate::from_ymd_opt(2027, 1, 1));
        assert_eq!(next_occurrence(today, 10, 11, false), Some(today));
        // 29/2 in a common year falls back to 28/2.
        assert_eq!(next_occurrence(today, 29, 2, false), NaiveDate::from_ymd_opt(2027, 2, 28));
    }
}

// ------------------------------------------------------------------ lunar --

#[tauri::command]
pub fn lunar_today() -> Value {
    use chrono::Datelike;
    let today = chrono::Local::now().date_naive();
    let l = crate::lunar::solar_to_lunar(today.day() as i64, today.month() as i64, today.year() as i64);
    json!({
        "solar": { "day": today.day(), "month": today.month(), "year": today.year(), "weekday": today.weekday().num_days_from_monday() },
        "lunar": { "day": l.day, "month": l.month, "monthName": crate::lunar::month_name(l.month), "year": l.year, "leap": l.leap },
        "yearCanChi": crate::lunar::year_can_chi(l.year),
        "monthCanChi": crate::lunar::month_can_chi(&l),
        "dayCanChi": crate::lunar::day_can_chi(l.jd),
        "tietKhi": crate::lunar::tiet_khi(l.jd),
        "gioHoangDao": crate::lunar::gio_hoang_dao(l.jd),
    })
}

// ---------------------------------------------------------------- weather --

#[tauri::command]
pub async fn weather(city: String, fahrenheit: bool) -> Result<crate::weather_api::Weather, String> {
    tauri::async_runtime::spawn_blocking(move || crate::weather_api::fetch(city.trim(), fahrenheit))
        .await
        .map_err(|e| e.to_string())?
}

// ----------------------------------------------------------------- system --

pub struct SystemMonitor(Mutex<crate::sysmon::SystemState>);

impl Default for SystemMonitor {
    fn default() -> Self {
        SystemMonitor(Mutex::new(crate::sysmon::SystemState::new()))
    }
}

#[tauri::command]
pub fn system_stats(mon: State<SystemMonitor>) -> Value {
    let mut s = mon.0.lock().unwrap();
    s.pump(true);
    let up = sysinfo::System::uptime();
    let mut v = serde_json::to_value(&*s).unwrap_or(Value::Null);
    v["uptime"] = json!(format!("up {}h {:02}m", up / 3600, (up % 3600) / 60));
    v["downText"] = json!(crate::sysmon::rate(s.down));
    v["upText"] = json!(crate::sysmon::rate(s.up));
    v
}

// ------------------------------------------------------------------ music --

#[tauri::command]
pub fn media_now(media: State<Media>) -> Snapshot {
    media.snapshot()
}

#[tauri::command]
pub fn media_command(media: State<Media>, action: String, value: Option<f64>) {
    media.send(match action.as_str() {
        "next" => Command::Next,
        "prev" => Command::Previous,
        "seek" => Command::Seek(value.unwrap_or(0.0)),
        _ => Command::PlayPause,
    });
}

/// Music-reactive levels (loopback capture runs while this is polled).
#[tauri::command]
pub fn audio_levels(audio: State<crate::audio::Audio>) -> crate::audio::Levels {
    audio.levels()
}

#[tauri::command]
pub fn open_player(which: String) -> Result<(), String> {
    if which == "wmp" {
        crate::media::shared::open_media_player()
    } else {
        crate::media::shared::open_youtube_music()
    }
}

// -------------------------------------------------------------- wallpaper --

struct WallCache {
    key: (String, Option<std::time::SystemTime>),
    value: Value,
}

static WALL: Mutex<Option<WallCache>> = Mutex::new(None);

/// The wallpaper as a data URL plus what the page needs to line it up with
/// the desktop ("Fill" on the primary monitor).
#[tauri::command]
pub async fn wallpaper() -> Value {
    tauri::async_runtime::spawn_blocking(|| {
        let Some(path) = crate::win::wallpaper_path() else { return Value::Null };
        let stamp = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        let key = (path.clone(), stamp);
        if let Some(c) = WALL.lock().unwrap().as_ref() {
            if c.key == key {
                return c.value.clone();
            }
        }
        let Some(img) = image::ImageReader::open(&path).ok().and_then(|r| r.with_guessed_format().ok()).and_then(|r| r.decode().ok()) else {
            return Value::Null;
        };
        let (iw, ih) = (img.width(), img.height());
        let small = if iw > 1600 { img.resize(1600, 1600 * ih / iw.max(1), image::imageops::FilterType::Triangle) } else { img };
        let mut out = std::io::Cursor::new(Vec::new());
        if small.to_rgb8().write_to(&mut out, image::ImageFormat::Jpeg).is_err() {
            return Value::Null;
        }
        let (sw, sh) = crate::win::screen_size();
        let value = json!({
            "url": format!("data:image/jpeg;base64,{}", b64(&out.into_inner())),
            "width": iw, "height": ih, "screenW": sw, "screenH": sh,
            "stamp": format!("{stamp:?}"),
        });
        *WALL.lock().unwrap() = Some(WallCache { key, value: value.clone() });
        value
    })
    .await
    .unwrap_or(Value::Null)
}

// ---------------------------------------------------------------- windows --
// Commands that create windows must be async: building a window from a
// synchronous command blocks the main thread on Windows (the app froze when
// a widget was switched back on).

#[tauri::command]
pub fn widgets_list(app: AppHandle) -> Value {
    json!(WIDGETS.iter().map(|(id, name)| json!({"id": id, "name": name, "open": windows_mgr::is_open(&app, id)})).collect::<Vec<_>>())
}

#[tauri::command]
pub async fn set_widget_open(app: AppHandle, id: String, open: bool) {
    windows_mgr::log(&format!("{id}: {} from the interface", if open { "shown" } else { "hidden" }));
    windows_mgr::set_open(&app, &id, open);
}

#[tauri::command]
pub async fn open_widget_settings(app: AppHandle, id: String) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("p-settings") {
        let _ = w.destroy();
    }
    windows_mgr::open_panel(&app, "settings", &format!("panel=settings&w={id}"), (480.0, 640.0), Some(&id)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_widgets_panel(app: AppHandle) -> Result<(), String> {
    windows_mgr::open_panel(&app, "widgets", "panel=widgets", (460.0, 820.0), Some("launcher")).map_err(|e| e.to_string())
}

/// The page reports use (pointer, keys) so the widget stays in front while
/// it is being used.
#[tauri::command]
pub fn widget_touch(window: WebviewWindow, raise: Option<bool>) {
    windows_mgr::touch(window.label());
    if raise.unwrap_or(false) {
        if let Ok(h) = window.hwnd() {
            let ok = crate::win::raise(h.0 as isize);
            if std::env::var_os("OPAL_RAISE_DEBUG").is_some() {
                windows_mgr::log(&format!("{}: raise on click -> {ok}", window.label()));
            }
        }
    }
}

#[tauri::command]
pub fn close_self(window: WebviewWindow) {
    let _ = window.destroy();
}

#[tauri::command]
pub fn config_folder() {
    crate::win::open_folder(&config::config_dir());
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    windows_mgr::quit_app(&app);
}

#[tauri::command]
pub fn beep() {
    crate::win::beep();
}

// --------------------------------------------------------------- calendar --

struct IcsCache {
    url: String,
    fetched: Instant,
    events: Vec<crate::ics::Event>,
    status: String,
    busy: bool,
}

static ICS: Mutex<Option<IcsCache>> = Mutex::new(None);

#[tauri::command]
pub fn calendar_get(st: State<AppState>) -> Value {
    let cfg = st.0.lock().unwrap().calendar.clone();
    let url = cfg.ics_url.trim().to_string();
    let mut guard = ICS.lock().unwrap();
    let stale = guard.as_ref().is_none_or(|c| c.url != url || c.fetched.elapsed() > Duration::from_secs(15 * 60));
    if !url.is_empty() && stale && !guard.as_ref().is_some_and(|c| c.busy && c.url == url) {
        *guard = Some(IcsCache { url: url.clone(), fetched: Instant::now(), events: guard.take().map(|c| c.events).unwrap_or_default(), status: "Syncing…".into(), busy: true });
        let u = url.clone();
        std::thread::spawn(move || {
            let result = crate::ics::fetch(&u);
            let mut g = ICS.lock().unwrap();
            if let Some(c) = g.as_mut().filter(|c| c.url == u) {
                c.busy = false;
                c.fetched = Instant::now();
                match result {
                    Ok(ev) => {
                        c.status = format!("{} events", ev.len());
                        c.events = ev;
                    }
                    Err(e) => c.status = e,
                }
            }
        });
    }
    let (events, status) = if url.is_empty() {
        (Vec::new(), String::new())
    } else {
        guard.as_ref().map(|c| (c.events.clone(), c.status.clone())).unwrap_or_default()
    };
    json!({ "config": cfg, "events": events, "status": status })
}

#[tauri::command]
pub fn calendar_save(app: AppHandle, st: State<AppState>, config: config::CalendarConfig) -> bool {
    let ok = {
        let mut store = st.0.lock().unwrap();
        store.calendar = config;
        store.calendar.save()
    };
    let _ = app.emit("calendar-changed", ());
    ok
}
