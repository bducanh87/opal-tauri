//! Music-reactive levels: what the speakers are playing, captured with
//! WASAPI loopback (no microphone), split into frequency bands. Capture only
//! runs while a widget keeps asking for levels, and stops a few seconds
//! after the last request.

use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED};

pub const BANDS: usize = 16;
const N: usize = 1024;

#[derive(Clone, Copy, Default, Serialize)]
pub struct Levels {
    pub bands: [f32; BANDS],
    /// Overall loudness 0..1.
    pub level: f32,
    /// Low end (kick, bass) 0..1.
    pub bass: f32,
}

struct Shared {
    levels: Levels,
    asked: Instant,
    running: bool,
}

pub struct Audio(Arc<Mutex<Shared>>);

impl Default for Audio {
    fn default() -> Self {
        Audio(Arc::new(Mutex::new(Shared { levels: Levels::default(), asked: Instant::now(), running: false })))
    }
}

impl Audio {
    /// Latest levels; starts capture if it is not running.
    pub fn levels(&self) -> Levels {
        let mut s = self.0.lock().unwrap();
        s.asked = Instant::now();
        if !s.running {
            s.running = true;
            let shared = self.0.clone();
            std::thread::spawn(move || {
                if let Err(e) = capture(&shared) {
                    crate::windows_mgr::log(&format!("audio: loopback capture stopped ({e})"));
                }
                let mut s = shared.lock().unwrap();
                s.running = false;
                s.levels = Levels::default();
            });
        }
        s.levels
    }
}

fn capture(shared: &Arc<Mutex<Shared>>) -> windows::core::Result<()> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let dev = en.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let client: IAudioClient = dev.Activate(CLSCTX_ALL, None)?;
        let fmt = client.GetMixFormat()?;
        let channels = (*fmt).nChannels.max(1) as usize;
        let rate = (*fmt).nSamplesPerSec.max(8000) as f32;
        let bits = (*fmt).wBitsPerSample;
        let init = client.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, 1_000_000, 0, fmt, None);
        CoTaskMemFree(Some(fmt as *const _));
        init?;
        let cap: IAudioCaptureClient = client.GetService()?;
        client.Start()?;

        let mut ring = vec![0f32; N];
        let mut pos = 0usize;
        let mut smooth = Levels::default();
        let edges = band_edges(rate);
        loop {
            std::thread::sleep(Duration::from_millis(16));
            if shared.lock().unwrap().asked.elapsed() > Duration::from_secs(3) {
                break;
            }
            let mut got = false;
            loop {
                let n = cap.GetNextPacketSize()?;
                if n == 0 {
                    break;
                }
                let mut data = std::ptr::null_mut();
                let mut frames = 0u32;
                let mut flags = 0u32;
                cap.GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
                let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0;
                for f in 0..frames as usize {
                    let mut v = 0f32;
                    if !silent && !data.is_null() {
                        for c in 0..channels {
                            let i = f * channels + c;
                            v += match bits {
                                32 => *(data as *const f32).add(i),
                                16 => *(data as *const i16).add(i) as f32 / 32768.0,
                                _ => 0.0,
                            };
                        }
                        v /= channels as f32;
                    }
                    ring[pos] = v;
                    pos = (pos + 1) % N;
                }
                cap.ReleaseBuffer(frames)?;
                got = true;
            }
            // No packets: the endpoint is silent.
            let raw = if got { analyse(&ring, pos, &edges) } else { Levels::default() };
            for b in 0..BANDS {
                smooth.bands[b] = ease(smooth.bands[b], raw.bands[b]);
            }
            smooth.level = ease(smooth.level, raw.level);
            smooth.bass = ease(smooth.bass, raw.bass);
            shared.lock().unwrap().levels = smooth;
        }
        let _ = client.Stop();
    }
    Ok(())
}

/// Fast attack, slower release, like a VU meter.
fn ease(cur: f32, target: f32) -> f32 {
    let k = if target > cur { 0.6 } else { 0.18 };
    cur + (target - cur) * k
}

/// FFT bin ranges for BANDS log-spaced bands from 40 Hz to 16 kHz.
fn band_edges(rate: f32) -> Vec<(usize, usize)> {
    let (lo, hi) = (40f32, 16000f32.min(rate / 2.0 - 1.0));
    let bin = |f: f32| ((f / rate * N as f32).round() as usize).clamp(1, N / 2 - 1);
    (0..BANDS)
        .map(|i| {
            let a = lo * (hi / lo).powf(i as f32 / BANDS as f32);
            let b = lo * (hi / lo).powf((i + 1) as f32 / BANDS as f32);
            let (s, e) = (bin(a), bin(b));
            (s, e.max(s + 1))
        })
        .collect()
}

fn analyse(ring: &[f32], pos: usize, edges: &[(usize, usize)]) -> Levels {
    let mut re = vec![0f32; N];
    let mut im = vec![0f32; N];
    let mut sum = 0f32;
    for i in 0..N {
        let s = ring[(pos + i) % N];
        sum += s * s;
        let w = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (N - 1) as f32).cos();
        re[i] = s * w;
    }
    fft(&mut re, &mut im);
    let mut out = Levels::default();
    for (b, &(s, e)) in edges.iter().enumerate() {
        let mut m = 0f32;
        for k in s..e {
            m = m.max((re[k] * re[k] + im[k] * im[k]).sqrt());
        }
        // magnitude -> dB -> 0..1 over a 60 dB window
        let db = 20.0 * (m / (N as f32 / 4.0) + 1e-9).log10();
        out.bands[b] = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
    }
    let rms = (sum / N as f32).sqrt();
    out.level = ((20.0 * (rms + 1e-9).log10() + 50.0) / 50.0).clamp(0.0, 1.0);
    out.bass = (out.bands[0] + out.bands[1] + out.bands[2] + out.bands[3]) / 4.0;
    out
}

/// In-place iterative radix-2 FFT.
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f32::consts::PI / len as f32;
        let (wr, wi) = (ang.cos(), ang.sin());
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1f32, 0f32);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let nr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = nr;
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_440hz_tone_lights_its_band() {
        let rate = 48000.0;
        let ring: Vec<f32> = (0..N).map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / rate).sin() * 0.5).collect();
        let edges = band_edges(rate);
        let l = analyse(&ring, 0, &edges);
        let peak = (0..BANDS).max_by(|a, b| l.bands[*a].total_cmp(&l.bands[*b])).unwrap();
        let (s, e) = edges[peak];
        let (fs, fe) = (s as f32 * rate / N as f32, e as f32 * rate / N as f32);
        assert!(fs <= 470.0 && fe >= 410.0, "peak band {peak} covers {fs}-{fe} Hz");
        assert!(l.level > 0.5);
        let silent = analyse(&vec![0.0; N], 0, &edges);
        assert!(silent.level == 0.0 && silent.bands.iter().all(|b| *b == 0.0));
    }
}
