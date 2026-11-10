//! Program catalog (Start Menu shortcuts + packaged apps) and Lumen's search
//! scoring from `shared/programSearch.ts`.

use std::path::{Path, PathBuf};
use std::sync::mpsc;

#[derive(Clone, Debug)]
pub struct Program {
    pub name: String,
    /// .lnk/.url/.exe path, or an AUMID for packaged apps.
    pub target: String,
    pub kind: &'static str,
}

impl Program {
    pub fn icon_path(&self) -> String {
        if self.kind == "aumid" {
            format!("shell:AppsFolder\\{}", self.target)
        } else {
            self.target.clone()
        }
    }
}

pub fn pretty_name(file: &str) -> String {
    let path = Path::new(file);
    let base = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| file.to_string());
    let known = match base.to_lowercase().as_str() {
        "msedge" => "Microsoft Edge",
        "chrome" => "Google Chrome",
        "firefox" => "Firefox",
        "code" => "Visual Studio Code",
        "devenv" => "Visual Studio",
        "explorer" => "File Explorer",
        "notepad" => "Notepad",
        "calc" => "Calculator",
        "cmd" => "Command Prompt",
        "powershell" | "pwsh" => "PowerShell",
        "wt" | "windowsterminal" => "Windows Terminal",
        "taskmgr" => "Task Manager",
        "mspaint" => "Paint",
        "spotify" => "Spotify",
        "discord" => "Discord",
        "steam" => "Steam",
        "telegram" => "Telegram",
        "slack" => "Slack",
        "notion" => "Notion",
        "obsidian" => "Obsidian",
        "vlc" => "VLC",
        "7zfm" => "7-Zip",
        _ => "",
    };
    if !known.is_empty() {
        return known.to_string();
    }
    if Path::new(file).is_dir() {
        return path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or(base);
    }
    let cleaned = base.replace(['-', '_'], " ").trim().to_string();
    if cleaned.is_empty() { base } else { cleaned }
}

fn search_text(value: &str) -> String {
    // NFKD + strip marks, approximated for Latin/Vietnamese letters.
    value
        .chars()
        .map(fold_diacritic)
        .collect::<String>()
        .to_lowercase()
        .trim()
        .to_string()
}

fn fold_diacritic(c: char) -> char {
    const GROUPS: &[(&str, char)] = &[
        ("àáạảãâầấậẩẫăằắặẳẵÀÁẠẢÃÂẦẤẬẨẪĂẰẮẶẲẴäåÄÅ", 'a'),
        ("èéẹẻẽêềếệểễÈÉẸẺẼÊỀẾỆỂỄëË", 'e'),
        ("ìíịỉĩÌÍỊỈĨïÏ", 'i'),
        ("òóọỏõôồốộổỗơờớợởỡÒÓỌỎÕÔỒỐỘỔỖƠỜỚỢỞỠöÖ", 'o'),
        ("ùúụủũưừứựửữÙÚỤỦŨƯỪỨỰỬỮüÜ", 'u'),
        ("ỳýỵỷỹỲÝỴỶỸ", 'y'),
        ("đĐ", 'd'),
    ];
    for (set, base) in GROUPS {
        if set.contains(c) {
            return *base;
        }
    }
    c
}

pub fn score(name: &str, target: &str, query: &str) -> u32 {
    let q = search_text(query);
    if q.is_empty() {
        return 0;
    }
    let name = search_text(name);
    if name == q {
        return 100;
    }
    if name.starts_with(&q) {
        return 80;
    }
    if name.split_whitespace().any(|w| w.starts_with(&q)) {
        return 70;
    }
    if name.contains(&q) {
        return 50;
    }
    let text = format!("{name} {}", search_text(target));
    if q.split_whitespace().all(|w| text.contains(w)) { 20 } else { 0 }
}

pub fn search<'a>(catalog: &'a [Program], query: &str, limit: usize) -> Vec<&'a Program> {
    let mut hits: Vec<(u32, &Program)> = catalog
        .iter()
        .map(|p| (score(&p.name, &p.target, query), p))
        .filter(|(s, _)| *s > 0)
        .collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase())));
    hits.into_iter().take(limit).map(|(_, p)| p).collect()
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 6 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out, depth + 1);
        } else if let Some(ext) = path.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            if ext == "lnk" || ext == "url" {
                out.push(path);
            }
        }
    }
}

fn start_menu_programs() -> Vec<Program> {
    let mut roots = Vec::new();
    if let Ok(pd) = std::env::var("ProgramData") {
        roots.push(PathBuf::from(pd).join("Microsoft\\Windows\\Start Menu\\Programs"));
    }
    if let Ok(ad) = std::env::var("APPDATA") {
        roots.push(PathBuf::from(ad).join("Microsoft\\Windows\\Start Menu\\Programs"));
    }
    let mut files = Vec::new();
    for root in roots {
        walk(&root, &mut files, 0);
    }
    files
        .into_iter()
        .filter_map(|path| {
            let name = path.file_stem()?.to_string_lossy().to_string();
            let lower = name.to_lowercase();
            if lower.contains("uninstall") || lower.starts_with("desktop") && lower.ends_with("ini") {
                return None;
            }
            Some(Program { name, target: path.to_string_lossy().to_string(), kind: "file" })
        })
        .collect()
}

/// Packaged (Store) apps via `Get-StartApps`; AUMIDs contain a `!`.
fn packaged_apps() -> Vec<Program> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Console]::OutputEncoding=[Text.Encoding]::UTF8; Get-StartApps | ForEach-Object { $_.Name + \"`t\" + $_.AppID }",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    let Ok(output) = output else { return Vec::new() };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let (name, id) = line.split_once('\t')?;
            let (name, id) = (name.trim(), id.trim());
            (id.contains('!') && !name.is_empty()).then(|| Program {
                name: name.to_string(),
                target: id.to_string(),
                kind: "aumid",
            })
        })
        .collect()
}

/// Build the catalog on a worker thread; the result arrives on the channel.
pub fn load_async() -> mpsc::Receiver<Vec<Program>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut all = start_menu_programs();
        let mut seen: std::collections::HashSet<String> =
            all.iter().map(|p| p.name.to_lowercase()).collect();
        let _ = tx.send(all.clone());
        for app in packaged_apps() {
            if seen.insert(app.name.to_lowercase()) {
                all.push(app);
            }
        }
        let _ = tx.send(all);
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoring_matches_lumen() {
        assert_eq!(score("Notepad", "", "notepad"), 100);
        assert_eq!(score("Notepad++", "", "note"), 80);
        assert_eq!(score("Visual Studio Code", "", "stu"), 70);
        assert_eq!(score("Calculator", "", "cula"), 50);
        assert_eq!(score("Tiếng Việt", "", "viet"), 70);
        assert_eq!(score("X", "C:\\tools\\foo.exe", "tools foo"), 20);
        assert_eq!(score("X", "", "zzz"), 0);
    }

    #[test]
    fn pretty_names() {
        assert_eq!(pretty_name("C:\\x\\msedge.exe"), "Microsoft Edge");
        assert_eq!(pretty_name("C:\\x\\my-tool_v2.exe"), "my tool v2");
    }
}
