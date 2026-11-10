//! Opal's own YouTube Music window. A small script in that window reports
//! the playing track and the "Up next" queue (YouTube Music has no API for
//! the queue, and Windows media sessions only know the current track), and
//! the music widget drives playback back through it.
//!
//! The page is remote, so its IPC access is limited to `ytm_report` (see
//! capabilities/ytm.json and the app manifest in build.rs).

use serde_json::Value;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "ytm";
const URL: &str = "https://music.youtube.com/";

#[derive(Default)]
pub struct Ytm(Mutex<Option<(Instant, Value)>>);

/// Runs in the YouTube Music page (top frame only).
const SCRIPT: &str = r#"
(function () {
  if (window.top !== window || location.host !== 'music.youtube.com' || window.__opalYtm) return;
  window.__opalYtm = true;
  const txt = e => e ? (e.textContent || '').trim().replace(/\s+/g, ' ') : '';
  const src = e => { const s = e ? (e.currentSrc || e.src || '') : ''; return s.startsWith('http') ? s : ''; };
  const big = s => s.replace(/=w\d+-h\d+[^&?#]*/, '=w544-h544-l90-rj');
  function collect() {
    const bar = document.querySelector('ytmusic-player-bar');
    if (!bar) return null;
    const title = txt(bar.querySelector('.title'));
    const v = document.querySelector('video');
    const queue = [...document.querySelectorAll('ytmusic-player-queue-item')].map(i => ({
      title: txt(i.querySelector('.song-title')),
      artist: txt(i.querySelector('.byline')),
      thumb: big(src(i.querySelector('img'))),
      duration: txt(i.querySelector('.duration')),
      current: i.hasAttribute('selected'),
    })).filter(q => q.title);
    return {
      title, artist: txt(bar.querySelector('.byline')).split(' • ')[0],
      album: (txt(bar.querySelector('.byline')).split(' • ')[1] || ''),
      cover: big(src(bar.querySelector('img.image') || bar.querySelector('img'))),
      playing: !!(v && !v.paused), position: v ? v.currentTime : 0,
      duration: v && isFinite(v.duration) ? v.duration : 0, queue,
    };
  }
  setInterval(() => {
    try {
      const d = collect();
      const ipc = window.__TAURI_INTERNALS__;
      if (d && ipc) ipc.invoke('ytm_report', { data: d }).catch(() => {});
    } catch (e) {}
  }, 1000);
  window.__opalControl = function (action, arg) {
    const bar = document.querySelector('ytmusic-player-bar');
    const v = document.querySelector('video');
    if (!bar) return;
    if (action === 'play') bar.querySelector('#play-pause-button')?.click();
    else if (action === 'next') bar.querySelector('.next-button')?.click();
    else if (action === 'prev') bar.querySelector('.previous-button')?.click();
    else if (action === 'seek' && v) v.currentTime = arg;
    else if (action === 'queue') {
      const it = document.querySelectorAll('ytmusic-player-queue-item')[arg];
      if (it) (it.querySelector('ytmusic-play-button-renderer') || it.querySelector('.song-info') || it).click();
    }
  };
})();
"#;

/// Report from the page (the only command the page may call).
#[tauri::command]
pub fn ytm_report(state: State<Ytm>, data: Value) {
    let mut s = state.0.lock().unwrap();
    if s.is_none() {
        crate::windows_mgr::log(&format!(
            "ytm: page reporting (track: {}, queue: {})",
            data["title"].as_str().unwrap_or(""),
            data["queue"].as_array().map(|q| q.len()).unwrap_or(0)
        ));
    }
    *s = Some((Instant::now(), data));
}

/// Latest state while the window is open and reporting.
#[tauri::command]
pub fn ytm_now(app: AppHandle, state: State<Ytm>) -> Value {
    if app.get_webview_window(LABEL).is_none() {
        return Value::Null;
    }
    match &*state.0.lock().unwrap() {
        Some((at, v)) if at.elapsed() < Duration::from_secs(4) => {
            let mut v = v.clone();
            v["open"] = Value::Bool(true);
            v
        }
        _ => serde_json::json!({ "open": true }),
    }
}

/// Open (or bring back) the YouTube Music window.
#[tauri::command]
pub async fn ytm_open(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }
    WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::External(URL.parse().map_err(|e| format!("{e}"))?))
        .title("Opal · YouTube Music")
        .inner_size(1180.0, 780.0)
        .min_inner_size(480.0, 400.0)
        .initialization_script(SCRIPT)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Playback control: play, next, prev, seek <seconds>, queue <index>.
#[tauri::command]
pub fn ytm_control(app: AppHandle, action: String, arg: Option<f64>) -> Result<(), String> {
    if !matches!(action.as_str(), "play" | "next" | "prev" | "seek" | "queue") {
        return Err("unknown action".into());
    }
    let w = app.get_webview_window(LABEL).ok_or("YouTube Music is not open")?;
    let arg = arg.unwrap_or(0.0);
    w.eval(&format!("window.__opalControl && window.__opalControl('{action}', {arg})")).map_err(|e| e.to_string())
}

/// Clear the report when the window closes.
pub fn closed(app: &AppHandle) {
    *app.state::<Ytm>().0.lock().unwrap() = None;
}
