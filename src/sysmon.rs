//! System monitor samples (CPU with history, memory, system drive,
//! network throughput). Shared by the egui and Tauri editions.

use serde::Serialize;
use std::collections::VecDeque;
use std::time::Instant;

#[derive(Serialize)]
pub struct SystemState {
    #[serde(skip)]
    sys: sysinfo::System,
    #[serde(skip)]
    disks: sysinfo::Disks,
    #[serde(skip)]
    nets: sysinfo::Networks,
    #[serde(skip)]
    last: Option<Instant>,
    pub cpu: f32,
    pub cpu_history: VecDeque<f32>,
    pub mem_used: u64,
    pub mem_total: u64,
    pub disk_used: u64,
    pub disk_total: u64,
    pub disk_name: String,
    pub down: f64,
    pub up: f64,
}

impl SystemState {
    pub fn new() -> Self {
        SystemState {
            sys: sysinfo::System::new(),
            disks: sysinfo::Disks::new(),
            nets: sysinfo::Networks::new(),
            last: None,
            cpu: 0.0,
            cpu_history: VecDeque::with_capacity(61),
            mem_used: 0,
            mem_total: 0,
            disk_used: 0,
            disk_total: 0,
            disk_name: String::new(),
            down: 0.0,
            up: 0.0,
        }
    }

    pub fn pump(&mut self, active: bool) {
        if !active {
            return;
        }
        let elapsed = self.last.map(|t| t.elapsed().as_secs_f64());
        if elapsed.is_some_and(|e| e < 0.95) {
            return;
        }
        self.last = Some(Instant::now());
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.cpu = self.sys.global_cpu_usage();
        if self.cpu_history.len() >= 60 {
            self.cpu_history.pop_front();
        }
        self.cpu_history.push_back(self.cpu);
        self.mem_used = self.sys.used_memory();
        self.mem_total = self.sys.total_memory();
        if self.disks.list().is_empty() {
            self.disks.refresh(true);
        } else {
            self.disks.refresh(false);
        }
        let sysdrive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        if let Some(d) = self
            .disks
            .list()
            .iter()
            .find(|d| d.mount_point().to_string_lossy().to_uppercase().starts_with(&sysdrive.to_uppercase()))
            .or_else(|| self.disks.list().first())
        {
            self.disk_total = d.total_space();
            self.disk_used = d.total_space().saturating_sub(d.available_space());
            self.disk_name = d.mount_point().to_string_lossy().trim_end_matches('\\').to_string();
        }
        let first = self.nets.list().is_empty();
        self.nets.refresh(true);
        if !first {
            let secs = elapsed.unwrap_or(1.0).max(0.2);
            let (mut rx, mut tx) = (0u64, 0u64);
            for (_, n) in self.nets.list() {
                rx += n.received();
                tx += n.transmitted();
            }
            self.down = rx as f64 / secs;
            self.up = tx as f64 / secs;
        }
    }
}

pub fn gb(b: u64) -> f64 {
    b as f64 / 1024f64.powi(3)
}

pub fn rate(bps: f64) -> String {
    if bps >= 1024.0 * 1024.0 {
        format!("{:.1} MB/s", bps / 1024.0 / 1024.0)
    } else if bps >= 1024.0 {
        format!("{:.0} KB/s", bps / 1024.0)
    } else {
        format!("{bps:.0} B/s")
    }
}

