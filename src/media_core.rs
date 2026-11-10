//! Now playing via Windows' Global System Media Transport Controls — the
//! same session the volume flyout shows. Works with Spotify, browsers
//! (YouTube Music, etc.), Media Player and most players, replacing Lumen's
//! .NET media bridge and its private Chrome profile.

use egui::{ColorImage, Context, TextureHandle, TextureOptions};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Storage::Streams::DataReader;

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub app: String,
    pub playing: bool,
    /// seconds
    pub position: f64,
    pub duration: f64,
    pub can_seek: bool,
    /// Timeline start (s); positions shown are relative to it.
    pub start: f64,
}

#[derive(Debug)]
pub enum Command {
    PlayPause,
    Next,
    Previous,
    Seek(f64),
}

enum Update {
    Info(Option<NowPlaying>, Instant),
    Cover(Option<ColorImage>),
}

pub struct Media {
    pub now: Option<NowPlaying>,
    sampled: Instant,
    pub cover: Option<TextureHandle>,
    tx: Sender<Command>,
    rx: Receiver<Update>,
    pub angle: f32,
    last_frame: Instant,
}

impl Media {
    pub fn start(ctx: &Context) -> Self {
        let (tx, commands) = channel();
        let (updates, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || worker(commands, updates, ctx));
        Media {
            now: None,
            sampled: Instant::now(),
            cover: None,
            tx,
            rx,
            angle: 0.0,
            last_frame: Instant::now(),
        }
    }

    pub fn pump(&mut self, ctx: &Context) {
        while let Ok(update) = self.rx.try_recv() {
            match update {
                Update::Info(now, at) => {
                    self.now = now;
                    self.sampled = at;
                }
                Update::Cover(img) => {
                    self.cover = img.map(|i| ctx.load_texture("cover", i, TextureOptions::LINEAR));
                }
            }
        }
        let dt = self.last_frame.elapsed().as_secs_f32().min(0.2);
        self.last_frame = Instant::now();
        if self.now.as_ref().is_some_and(|n| n.playing) {
            self.angle = (self.angle + dt * std::f32::consts::TAU / 1.8) % std::f32::consts::TAU;
        }
    }

    /// Live position, extrapolated between samples while playing.
    pub fn position(&self) -> f64 {
        match &self.now {
            Some(n) if n.playing => {
                (n.position + self.sampled.elapsed().as_secs_f64()).min(n.duration.max(n.position))
            }
            Some(n) => n.position,
            None => 0.0,
        }
    }

    pub fn send(&self, c: Command) {
        let _ = self.tx.send(c);
    }

    pub fn wants_animation(&self, settings: &crate::config::Settings) -> bool {
        settings.design("music") == "turntable" && self.now.as_ref().is_some_and(|n| n.playing)
    }
}

fn worker(commands: Receiver<Command>, updates: Sender<Update>, ctx: Context) {
    crate::win::com_init();
    let manager = loop {
        match Manager::RequestAsync().and_then(|op| op.get()) {
            Ok(m) => break m,
            Err(_) => {
                if updates.send(Update::Info(None, Instant::now())).is_err() {
                    return;
                }
                std::thread::sleep(Duration::from_secs(10));
            }
        }
    };
    if std::env::var_os("OPAL_MEDIA_DEBUG").is_some() {
        use std::io::Write;
        let mut out = String::new();
        if let Ok(list) = manager.GetSessions() {
            for sess in list {
                let id = sess.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default();
                let title = sess.TryGetMediaPropertiesAsync().and_then(|o| o.get()).and_then(|p| p.Title()).map(|t| t.to_string()).unwrap_or_default();
                let st = sess.GetPlaybackInfo().and_then(|i| i.PlaybackStatus()).map(|s| s.0).unwrap_or(-1);
                out.push_str(&format!("{id} | {title} | status {st}
"));
            }
        }
        if out.is_empty() {
            out.push_str("no sessions
");
        }
        if let Ok(mut f) = std::fs::File::create(crate::config::config_file("media-sessions.log")) {
            let _ = f.write_all(out.as_bytes());
        }
    }
    let mut last_info: Option<NowPlaying> = None;
    let mut cover_key = String::new();
    let mut cover_missing_since: Option<Instant> = None;
    loop {
        let session = pick_session(&manager);
        let info = session.as_ref().and_then(read_info);
        let key = info.as_ref().map(|i| format!("{}\u{1}{}\u{1}{}", i.title, i.artist, i.album)).unwrap_or_default();
        if info != last_info {
            if updates.send(Update::Info(info.clone(), Instant::now())).is_err() {
                return;
            }
            ctx.request_repaint();
        }
        // Players often publish the artwork a moment after the title, so
        // keep asking for a while when it is missing.
        let retry = cover_missing_since.is_some_and(|t| t.elapsed() < Duration::from_secs(30));
        if key != cover_key || retry {
            let changed = key != cover_key;
            cover_key = key;
            let cover = session.as_ref().and_then(read_cover);
            if changed {
                cover_missing_since = if cover.is_none() && info.is_some() { Some(Instant::now()) } else { None };
            } else if cover.is_some() {
                cover_missing_since = None;
            }
            if changed || cover.is_some() {
                if updates.send(Update::Cover(cover)).is_err() {
                    return;
                }
                ctx.request_repaint();
            }
        }
        last_info = info;

        match commands.recv_timeout(Duration::from_millis(1000)) {
            Ok(cmd) => {
                if let Some(s) = &session {
                    run(s, cmd);
                }
                std::thread::sleep(Duration::from_millis(150));
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// Prefer a session that is playing, then Windows' current one, then any
/// session that has a title (e.g. a paused YouTube Music window).
pub fn pick_session(manager: &Manager) -> Option<Session> {
    let all: Vec<Session> = manager
        .GetSessions()
        .ok()
        .map(|list| list.into_iter().collect())
        .unwrap_or_default();
    let playing = |s: &Session| {
        s.GetPlaybackInfo()
            .and_then(|i| i.PlaybackStatus())
            .map(|st| st == Status::Playing)
            .unwrap_or(false)
    };
    if let Some(s) = all.iter().find(|s| playing(s)) {
        return Some(s.clone());
    }
    if let Ok(cur) = manager.GetCurrentSession() {
        return Some(cur);
    }
    all.into_iter().find(|s| {
        s.TryGetMediaPropertiesAsync()
            .and_then(|o| o.get())
            .and_then(|p| p.Title())
            .map(|t| !t.is_empty())
            .unwrap_or(false)
    })
}

/// Open music.youtube.com as an app window in Chrome (or Edge) with a
/// dedicated profile beside the app, like Lumen's own music player.
pub fn open_youtube_music() -> Result<(), String> {
    let candidates = [
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    ];
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let user_chrome = format!(r"{local}\Google\Chrome\Application\chrome.exe");
    let browser = std::iter::once(user_chrome.as_str())
        .chain(candidates.iter().copied())
        .find(|p| std::path::Path::new(p).exists())
        .ok_or_else(|| "Chrome or Edge was not found".to_string())?;
    let name = if browser.to_lowercase().contains("msedge") { "edge" } else { "chrome" };
    let profile = crate::config::config_dir().join("music-player").join(name);
    let _ = std::fs::create_dir_all(&profile);
    std::process::Command::new(browser)
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg("--app=https://music.youtube.com")
        .arg("--no-first-run")
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Windows' Media Player (the Windows 11 app, formerly Groove). It publishes
/// its playback to Windows, so the widget follows and controls it.
pub fn open_media_player() -> Result<(), String> {
    crate::win::shell_open(r"shell:AppsFolder\Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic", None, None)
        .or_else(|_| crate::win::shell_open("wmplayer.exe", None, None))
        .map_err(|_| "Media Player is not installed".to_string())
}

pub fn run(s: &Session, cmd: Command) {
    let _ = match cmd {
        Command::PlayPause => s.TryTogglePlayPauseAsync().and_then(|o| o.get()),
        Command::Next => s.TrySkipNextAsync().and_then(|o| o.get()),
        Command::Previous => s.TrySkipPreviousAsync().and_then(|o| o.get()),
        Command::Seek(secs) => {
            let start = s
                .GetTimelineProperties()
                .and_then(|t| t.StartTime())
                .map(|t| t.Duration)
                .unwrap_or(0);
            s.TryChangePlaybackPositionAsync(start + (secs * 10_000_000.0) as i64).and_then(|o| o.get())
        }
    };
}

/// Windows FILETIME ticks (100 ns since 1601) for "now".
fn now_ticks() -> i64 {
    let unix_ms = crate::config::now_ms();
    (unix_ms + 11_644_473_600_000) * 10_000
}

pub fn read_info(s: &Session) -> Option<NowPlaying> {
    let props = s.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
    let playback = s.GetPlaybackInfo().ok()?;
    let playing = playback.PlaybackStatus().ok()? == Status::Playing;
    let can_seek = playback
        .Controls()
        .and_then(|c| c.IsPlaybackPositionEnabled())
        .unwrap_or(false);
    let (mut position, mut duration, mut tl_start) = (0.0, 0.0, 0.0);
    if let Ok(tl) = s.GetTimelineProperties() {
        let pos = tl.Position().map(|t| t.Duration).unwrap_or(0) as f64 / 1e7;
        let end = tl.EndTime().map(|t| t.Duration).unwrap_or(0) as f64 / 1e7;
        let start = tl.StartTime().map(|t| t.Duration).unwrap_or(0) as f64 / 1e7;
        duration = (end - start).max(0.0);
        position = pos - start;
        tl_start = start;
        if playing {
            if let Ok(updated) = tl.LastUpdatedTime() {
                let age = (now_ticks() - updated.UniversalTime) as f64 / 1e7;
                if (0.0..3600.0).contains(&age) {
                    position += age;
                }
            }
        }
        if duration > 0.0 {
            position = position.clamp(0.0, duration);
        }
    }
    let title = props.Title().map(|h| h.to_string()).unwrap_or_default();
    if title.is_empty() {
        return None;
    }
    let app = s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default();
    Some(NowPlaying {
        title,
        artist: props.Artist().map(|h| h.to_string()).unwrap_or_default(),
        album: props.AlbumTitle().map(|h| h.to_string()).unwrap_or_default(),
        app: friendly_app(&app),
        playing,
        position,
        duration,
        start: tl_start,
        can_seek,
    })
}

fn friendly_app(aumid: &str) -> String {
    let lower = aumid.to_lowercase();
    for (needle, name) in [
        ("spotify", "Spotify"),
        ("chrome", "Chrome"),
        ("msedge", "Edge"),
        ("firefox", "Firefox"),
        ("zunemusic", "Media Player"),
        ("vlc", "VLC"),
    ] {
        if lower.contains(needle) {
            return name.into();
        }
    }
    let first = aumid.split(['!', '.']).next().unwrap_or("");
    // Browser web-app ids are opaque hashes; don't show them.
    if first.len() >= 12 && first.chars().all(|c| c.is_ascii_hexdigit()) {
        return String::new();
    }
    first.to_string()
}

pub fn read_cover(s: &Session) -> Option<ColorImage> {
    let props = s.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
    let thumb = props.Thumbnail().ok()?;
    let stream = thumb.OpenReadAsync().ok()?.get().ok()?;
    let size = stream.Size().ok()? as u32;
    if size == 0 || size > 16 * 1024 * 1024 {
        return None;
    }
    let reader = DataReader::CreateDataReader(&stream).ok()?;
    reader.LoadAsync(size).ok()?.get().ok()?;
    let mut bytes = vec![0u8; size as usize];
    reader.ReadBytes(&mut bytes).ok()?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let mut img = if img.width() > 512 {
        image::imageops::resize(&img, 512, 512 * img.height() / img.width(), image::imageops::FilterType::Triangle)
    } else {
        img
    };
    crate::theme::avoid_key(&mut img);
    Some(ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw()))
}
