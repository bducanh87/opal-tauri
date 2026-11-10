//! Persistent state. File names and JSON field names follow the original
//! Lumen layout (`config/state.json`, `config/notes.json`,
//! `config/calendar.json`) so an existing Lumen `config` folder can be copied
//! next to `opal.exe` and picked up as-is. Window placement for this rewrite
//! lives in `config/opal-egui.json`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn uid() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let t = now_ms() as u64;
    let mix = t.wrapping_mul(6364136223846793005).wrapping_add(n.wrapping_mul(1442695040888963407));
    format!("{:x}{:x}", mix & 0xffff_ffff_ffff, n)
}

/// `config/` beside the executable (portable, like Lumen). Falls back to
/// `%APPDATA%\Opal` when that folder is not writable.
pub fn config_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let dir = pick_config_dir();
        migrate_old_names(&dir);
        dir
    })
    .clone()
}

/// Files written before the app was renamed from its Lumen-rs working name
/// to Opal: picked up once under the new names.
fn migrate_old_names(dir: &Path) {
    for (old, new) in [
        ("lumen-rs.json", "opal-egui.json"),
        ("lumen-rs-extras.json", "opal-extras.json"),
        ("lumen-rs.log", "opal-egui.log"),
        ("lumen-tauri.json", "opal-tauri.json"),
        ("lumen-tauri.log", "opal-tauri.log"),
    ] {
        let (o, n) = (dir.join(old), dir.join(new));
        if o.exists() && !n.exists() {
            let _ = fs::rename(&o, &n);
        }
    }
}

fn pick_config_dir() -> PathBuf {
    {
        if let Ok(dir) = std::env::var("OPAL_CONFIG_DIR") {
            let p = PathBuf::from(dir);
            let _ = fs::create_dir_all(&p);
            return p;
        }
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."));
        let portable = exe_dir.join("config");
        if fs::create_dir_all(&portable).is_ok() {
            let probe = portable.join(".write-test");
            if fs::write(&probe, b"ok").is_ok() {
                let _ = fs::remove_file(probe);
                return portable;
            }
        }
        let appdata = std::env::var("APPDATA").map(PathBuf::from).unwrap_or(exe_dir);
        let fallback = appdata.join("Opal");
        let legacy = appdata.join("LumenRs");
        if legacy.is_dir() && !fallback.exists() {
            let _ = fs::rename(&legacy, &fallback);
        }
        let _ = fs::create_dir_all(&fallback);
        fallback
    }
}

pub fn config_file(name: &str) -> PathBuf {
    config_dir().join(name)
}

/// Read a JSON file. A missing file gives the default. A file that exists
/// but cannot be read or parsed is first copied to `<name>.bad-<time>.json`
/// so that the defaults written later can never destroy the user's data.
fn read_json<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> T {
    if !path.exists() {
        return T::default();
    }
    // A sync client or virus scanner can hold the file for a moment.
    let mut text = None;
    for _ in 0..5 {
        match fs::read_to_string(path) {
            Ok(t) => {
                text = Some(t);
                break;
            }
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(120)),
        }
    }
    let parsed = text.as_deref().map(serde_json::from_str::<T>);
    match parsed {
        Some(Ok(v)) => v,
        other => {
            let why = match other {
                Some(Err(e)) => e.to_string(),
                _ => "unreadable".into(),
            };
            let stamp = now_ms();
            let backup = path.with_extension(format!("bad-{stamp}.json"));
            let _ = fs::copy(path, &backup);
            eprintln!("[opal] {} could not be read ({why}); kept a copy at {}", path.display(), backup.display());
            T::default()
        }
    }
}

/// Atomic write: temp file then rename, as Lumen does. Returns false when
/// the data did not reach the disk, so callers keep it dirty and retry.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> bool {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("json.tmp");
    match serde_json::to_string_pretty(value) {
        Ok(text) => {
            if fs::write(&tmp, text).is_err() {
                return false;
            }
            if fs::rename(&tmp, path).is_err() {
                let _ = fs::remove_file(&tmp);
                return false;
            }
            true
        }
        Err(e) => {
            eprintln!("[opal] serialize failed: {e}");
            false
        }
    }
}

/// Accept 48, 48.0 or "48" for whole-number settings written by other tools.
fn lenient_u32<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let v = Value::deserialize(d)?;
    Ok(match v {
        Value::Number(n) => n.as_f64().unwrap_or(0.0).round().clamp(0.0, u32::MAX as f64) as u32,
        Value::String(s) => s.trim().parse::<f64>().unwrap_or(0.0).round().max(0.0) as u32,
        _ => 0,
    })
}

fn lenient_u32_map<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BTreeMap<String, u32>, D::Error> {
    let v = Value::deserialize(d)?;
    let mut out = BTreeMap::new();
    if let Value::Object(map) = v {
        for (k, v) in map {
            if let Some(f) = v.as_f64() {
                out.insert(k, f.round().clamp(0.0, 100.0) as u32);
            }
        }
    }
    Ok(out)
}

fn lenient_opt_u32<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    let v = Value::deserialize(d)?;
    Ok(v.as_f64().map(|f| f.round().max(0.0) as u32))
}

// ---------------------------------------------------------------- launcher --

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    #[serde(default = "uid")]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// app | url | folder | file | aumid | shell
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    #[serde(default = "now_ms")]
    pub added_at: i64,
    #[serde(default)]
    pub launch_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used: Option<i64>,
    /// Unknown fields written by the original Lumen are kept on save.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn default_kind() -> String {
    "app".into()
}

impl Item {
    pub fn new(name: String, target: String, kind: &str) -> Self {
        Item {
            id: uid(),
            name,
            target,
            args: None,
            cwd: None,
            kind: kind.into(),
            icon: None,
            icon_source: None,
            group: None,
            accent: None,
            added_at: now_ms(),
            launch_count: 0,
            last_used: None,
            extra: BTreeMap::new(),
        }
    }

    /// Path handed to the shell icon extractor.
    pub fn icon_path(&self) -> String {
        if let Some(src) = &self.icon_source {
            return src.clone();
        }
        if self.kind == "aumid" {
            return format!("shell:AppsFolder\\{}", self.target);
        }
        self.target.clone()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ClockSettings {
    pub theme: String,
    pub frosted_glass: bool,
    pub time_zone: String,
    pub hour12: bool,
    pub show_seconds: bool,
    pub open: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for ClockSettings {
    fn default() -> Self {
        ClockSettings {
            theme: "lumen".into(),
            frosted_glass: true,
            time_zone: "local".into(),
            hour12: false,
            show_seconds: true,
            open: true,
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub clock: ClockSettings,
    pub theme: String,
    #[serde(deserialize_with = "lenient_u32")]
    pub columns: u32,
    #[serde(deserialize_with = "lenient_u32")]
    pub icon_size: u32,
    pub show_search: bool,
    pub show_labels: bool,
    pub wallpaper_frost: bool,
    #[serde(deserialize_with = "lenient_u32")]
    pub frost_strength: u32,
    #[serde(deserialize_with = "lenient_u32")]
    pub blur_size: u32,
    #[serde(deserialize_with = "lenient_u32")]
    pub background_opacity: u32,
    #[serde(deserialize_with = "lenient_u32_map")]
    pub widget_opacity: BTreeMap<String, u32>,
    pub widget_theme: BTreeMap<String, String>,
    pub widget_design: BTreeMap<String, String>,
    pub widget_frosted: BTreeMap<String, bool>,
    pub widget_appearance: BTreeMap<String, WidgetAppearance>,
    #[serde(deserialize_with = "lenient_u32")]
    pub veil: u32,
    pub start_with_windows: bool,
    pub memory_saver: bool,
    pub performance_mode: bool,
    pub seeded: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            clock: ClockSettings::default(),
            theme: "opal".into(),
            columns: 4,
            icon_size: 46,
            show_search: true,
            show_labels: true,
            wallpaper_frost: true,
            frost_strength: 62,
            blur_size: 44,
            background_opacity: 100,
            widget_opacity: BTreeMap::new(),
            widget_theme: BTreeMap::new(),
            widget_design: BTreeMap::new(),
            widget_frosted: BTreeMap::new(),
            widget_appearance: BTreeMap::new(),
            veil: 48,
            start_with_windows: false,
            memory_saver: true,
            performance_mode: false,
            seeded: false,
            extra: BTreeMap::new(),
        }
    }
}

impl Settings {
    pub fn sanitize(&mut self) {
        self.columns = self.columns.clamp(3, 12);
        self.icon_size = self.icon_size.clamp(36, 60);
        self.frost_strength = self.frost_strength.min(100);
        self.blur_size = self.blur_size.min(80);
        self.background_opacity = self.background_opacity.min(100);
        self.veil = self.veil.min(100);
        if crate::theme::find(&self.theme).is_none() {
            self.theme = "obsidian".into();
        }
    }

    pub fn theme_for(&self, widget: &str) -> &str {
        self.widget_theme.get(widget).map(String::as_str).unwrap_or(&self.theme)
    }

    pub fn frosted(&self, widget: &str) -> bool {
        if let Some(v) = self.widget_frosted.get(widget) {
            return *v;
        }
        if widget == "clock" {
            return self.clock.frosted_glass;
        }
        true
    }

    fn appearance(&self, widget: &str) -> Option<&WidgetAppearance> {
        self.widget_appearance.get(widget)
    }

    /// Glass tint (Lumen's `veil`) for one widget, 0..100.
    pub fn veil_for(&self, widget: &str) -> u32 {
        self.appearance(widget).and_then(|a| a.veil).unwrap_or(self.veil).min(100)
    }

    pub fn frost_for(&self, widget: &str) -> u32 {
        self.appearance(widget).and_then(|a| a.frost_strength).unwrap_or(self.frost_strength).min(100)
    }

    pub fn blur_for(&self, widget: &str) -> u32 {
        self.appearance(widget).and_then(|a| a.blur_size).unwrap_or(self.blur_size).min(80)
    }

    pub fn wallpaper_frost_for(&self, widget: &str) -> bool {
        self.appearance(widget).and_then(|a| a.wallpaper_frost).unwrap_or(self.wallpaper_frost)
    }

    pub fn appearance_mut(&mut self, widget: &str) -> &mut WidgetAppearance {
        self.widget_appearance.entry(widget.to_string()).or_default()
    }

    pub fn opacity(&self, widget: &str) -> u32 {
        (*self.widget_opacity.get(widget).unwrap_or(&self.background_opacity)).max(10)
    }

    pub fn design(&self, widget: &str) -> String {
        if widget == "clock" {
            return self.clock.theme.clone();
        }
        self.widget_design
            .get(widget)
            .cloned()
            .unwrap_or_else(|| crate::theme::designs(widget)[0].id.to_string())
    }

    pub fn set_design(&mut self, widget: &str, id: &str) {
        if widget == "clock" {
            self.clock.theme = id.into();
        } else {
            self.widget_design.insert(widget.into(), id.into());
        }
    }
}

/// Per-widget overrides of the shared glass settings (Lumen's
/// `widgetAppearance`).
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct WidgetAppearance {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallpaper_frost: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "lenient_opt_u32")]
    pub frost_strength: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "lenient_opt_u32")]
    pub blur_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "lenient_opt_u32")]
    pub veil: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct State {
    pub version: u32,
    pub items: Vec<Item>,
    pub settings: Settings,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl State {
    pub fn load() -> Self {
        let mut state: State = read_json(&config_file("state.json"));
        state.version = 1;
        state.settings.sanitize();
        state.items.retain(|i| !i.target.is_empty());
        for item in &mut state.items {
            if item.name.is_empty() {
                item.name = crate::programs::pretty_name(&item.target);
            }
        }
        state
    }

    pub fn save(&self) -> bool {
        write_json(&config_file("state.json"), self)
    }
}

// ------------------------------------------------------------------- notes --

pub const NOTE_COLORS: [&str; 7] = ["amber", "rose", "sky", "mint", "violet", "peach", "slate"];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    #[serde(default = "uid")]
    pub id: String,
    /// Lumen stores a small HTML subset; we keep that format on disk.
    #[serde(default)]
    pub body: String,
    #[serde(default = "default_note_color")]
    pub color: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default = "now_ms")]
    pub created_at: i64,
    #[serde(default = "now_ms")]
    pub updated_at: i64,
    #[serde(skip)]
    pub text: String,
}

fn default_note_color() -> String {
    "amber".into()
}

impl Note {
    pub fn new(color: &str) -> Self {
        let now = now_ms();
        Note {
            id: uid(),
            body: String::new(),
            color: color.into(),
            pinned: false,
            created_at: now,
            updated_at: now,
            text: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct NotesFile {
    pub version: u32,
    pub notes: Vec<Note>,
}

impl NotesFile {
    pub fn load() -> Self {
        let mut file: NotesFile = read_json(&config_file("notes.json"));
        file.version = 1;
        for note in &mut file.notes {
            if !NOTE_COLORS.contains(&note.color.as_str()) {
                note.color = "amber".into();
            }
            note.text = html_to_text(&note.body);
        }
        file
    }

    pub fn save(&mut self) -> bool {
        for note in &mut self.notes {
            if html_to_text(&note.body) != note.text {
                note.body = text_to_html(&note.text);
            }
        }
        write_json(&config_file("notes.json"), self)
    }
}

/// Convert Lumen's note HTML to plain text for editing.
pub fn html_to_text(html: &str) -> String {
    let mut s = html.to_string();
    for (from, to) in [
        ("<br>", "\n"),
        ("<br/>", "\n"),
        ("<br />", "\n"),
        ("</div>", "\n"),
        ("</p>", "\n"),
        ("</li>", "\n"),
        ("<li>", "• "),
    ] {
        s = s.replace(from, to);
    }
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    let out = out
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&");
    out.trim_end_matches('\n').to_string()
}

pub fn text_to_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\n', "<br>")
}

// ---------------------------------------------------------------- calendar --

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct CalendarConfig {
    pub ics_url: String,
    pub week_starts_on: u32,
    pub theme: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for CalendarConfig {
    fn default() -> Self {
        CalendarConfig {
            ics_url: String::new(),
            week_starts_on: 1,
            theme: "glass".into(),
            extra: BTreeMap::new(),
        }
    }
}

impl CalendarConfig {
    pub fn load() -> Self {
        let mut c: CalendarConfig = read_json(&config_file("calendar.json"));
        if c.week_starts_on > 6 {
            c.week_starts_on = 1;
        }
        c
    }
    pub fn save(&self) -> bool {
        write_json(&config_file("calendar.json"), self)
    }
}

// ------------------------------------------------------------------ layout --

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Placement {
    pub open: bool,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Layout {
    pub widgets: BTreeMap<String, Placement>,
}

impl Layout {
    pub fn load() -> Self {
        read_json(&config_file("opal-egui.json"))
    }
    pub fn save(&self) -> bool {
        write_json(&config_file("opal-egui.json"), self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_round_trip() {
        let text = "Buy milk\n<b> & eggs";
        assert_eq!(html_to_text(&text_to_html(text)), text);
        assert_eq!(html_to_text("<div>one</div><div>two</div>"), "one\ntwo");
    }

    #[test]
    fn reads_lumen_state() {
        let raw = r#"{"version":1,"items":[{"id":"a","name":"Code","target":"C:\\code.exe","kind":"app","icon":null,"addedAt":1,"launchCount":3,"iconStamp":7}],
            "settings":{"theme":"aurora","columns":20,"clock":{"theme":"neon","hour12":true,"position":{"x":1,"y":2}},"displayLayout":"x"}}"#;
        let mut s: State = serde_json::from_str(raw).unwrap();
        s.settings.sanitize();
        assert_eq!(s.settings.theme, "aurora");
        assert_eq!(s.settings.columns, 12);
        assert_eq!(s.settings.clock.theme, "neon");
        assert!(s.settings.clock.hour12);
        assert_eq!(s.items[0].launch_count, 3);
        let back = serde_json::to_string(&s).unwrap();
        assert!(back.contains("iconStamp"), "unknown item fields survive");
        assert!(back.contains("displayLayout"), "unknown settings survive");
        assert!(back.contains("\"position\""), "unknown clock fields survive");
    }
}

// ------------------------------------------------------------ new widgets --

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Todo {
    #[serde(default = "uid")]
    pub id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub done: bool,
    #[serde(default = "now_ms")]
    pub created_at: i64,
}

/// A yearly date to count down to, solar or lunar (giỗ, sinh nhật…).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Countdown {
    #[serde(default = "uid")]
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub day: u32,
    pub month: u32,
    /// Day and month are in the lunar calendar.
    #[serde(default)]
    pub lunar: bool,
}

/// Settings and data for the widgets this rewrite adds (weather, system,
/// focus timer, photos, lunar calendar, to-do, countdown). Stored in
/// `config/opal-extras.json`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Extras {
    /// City name for the forecast; empty = locate by IP address.
    pub weather_city: String,
    pub weather_fahrenheit: bool,
    pub photos_folder: String,
    /// Seconds between photos.
    pub photos_interval: u32,
    pub focus_minutes: u32,
    pub short_break_minutes: u32,
    pub long_break_minutes: u32,
    pub todos: Vec<Todo>,
    pub system_show_network: bool,
    pub system_show_graph: bool,
    pub lunar_show_hours: bool,
    pub lunar_show_term: bool,
    pub countdowns: Vec<Countdown>,
    /// Built-in: Tết Nguyên Đán.
    pub countdown_show_tet: bool,
    /// Built-in: the next Rằm (15) and Mùng 1.
    pub countdown_show_moon: bool,
}

impl Default for Extras {
    fn default() -> Self {
        let pictures = std::env::var("USERPROFILE")
            .map(|h| format!(r"{h}\Pictures"))
            .unwrap_or_default();
        Extras {
            weather_city: String::new(),
            weather_fahrenheit: false,
            photos_folder: pictures,
            photos_interval: 20,
            focus_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
            todos: Vec::new(),
            system_show_network: true,
            system_show_graph: true,
            lunar_show_hours: true,
            lunar_show_term: true,
            countdowns: Vec::new(),
            countdown_show_tet: true,
            countdown_show_moon: true,
        }
    }
}

impl Extras {
    pub fn load() -> Self {
        let mut e: Extras = read_json(&config_file("opal-extras.json"));
        e.photos_interval = e.photos_interval.clamp(5, 600);
        e.focus_minutes = e.focus_minutes.clamp(1, 180);
        e.short_break_minutes = e.short_break_minutes.clamp(1, 60);
        e.long_break_minutes = e.long_break_minutes.clamp(1, 90);
        e
    }
    pub fn save(&self) -> bool {
        write_json(&config_file("opal-extras.json"), self)
    }
}
