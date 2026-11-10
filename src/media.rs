//! Now playing for the web UI, built on the shared media code (media_core.rs)
//! (Windows media sessions: Spotify, browsers, Media Player…).

#[path = "media_core.rs"]
#[allow(dead_code)]
pub mod shared;

use base64::Engine;
use serde::Serialize;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager as Manager;

pub use shared::Command;

#[derive(Clone, Default, Serialize)]
pub struct Snapshot {
    pub now: Option<shared::NowPlaying>,
    /// PNG data URL of the album art.
    pub cover: Option<String>,
    /// Milliseconds since the position was sampled.
    pub age_ms: u64,
}

struct Shared {
    now: Option<shared::NowPlaying>,
    cover: Option<String>,
    sampled: Instant,
}

pub struct Media {
    data: Arc<Mutex<Shared>>,
    tx: Mutex<Sender<Command>>,
}

impl Media {
    pub fn start() -> Self {
        let data = Arc::new(Mutex::new(Shared { now: None, cover: None, sampled: Instant::now() }));
        let (tx, rx) = channel();
        let d = data.clone();
        std::thread::spawn(move || worker(rx, d));
        Media { data, tx: Mutex::new(tx) }
    }

    pub fn snapshot(&self) -> Snapshot {
        let d = self.data.lock().unwrap();
        Snapshot { now: d.now.clone(), cover: d.cover.clone(), age_ms: d.sampled.elapsed().as_millis() as u64 }
    }

    pub fn send(&self, c: Command) {
        let _ = self.tx.lock().unwrap().send(c);
    }
}

fn png_data_url(img: &egui::ColorImage) -> Option<String> {
    let mut rgba = Vec::with_capacity(img.pixels.len() * 4);
    for p in &img.pixels {
        rgba.extend_from_slice(&p.to_array());
    }
    let buf = image::RgbaImage::from_raw(img.size[0] as u32, img.size[1] as u32, rgba)?;
    let mut out = std::io::Cursor::new(Vec::new());
    buf.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(out.into_inner())))
}

fn worker(commands: Receiver<Command>, data: Arc<Mutex<Shared>>) {
    crate::win::com_init();
    let manager = loop {
        match Manager::RequestAsync().and_then(|op| op.get()) {
            Ok(m) => break m,
            Err(_) => std::thread::sleep(Duration::from_secs(10)),
        }
    };
    let mut cover_key = String::new();
    let mut cover_tries = 0;
    loop {
        let session = shared::pick_session(&manager);
        let info = session.as_ref().and_then(shared::read_info);
        let key = info.as_ref().map(|i| format!("{}\u{1}{}", i.title, i.artist)).unwrap_or_default();
        if key != cover_key {
            cover_key = key;
            cover_tries = 0;
            data.lock().unwrap().cover = None;
        }
        // Artwork often arrives a moment after the title; retry a few times.
        if info.is_some() && cover_tries < 15 && data.lock().unwrap().cover.is_none() {
            cover_tries += 1;
            if let Some(img) = session.as_ref().and_then(shared::read_cover) {
                data.lock().unwrap().cover = png_data_url(&img);
            }
        }
        {
            let mut d = data.lock().unwrap();
            d.now = info;
            d.sampled = Instant::now();
        }
        match commands.recv_timeout(Duration::from_millis(1000)) {
            Ok(cmd) => {
                if let Some(s) = &session {
                    shared::run(s, cmd);
                }
                std::thread::sleep(Duration::from_millis(150));
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}
