//! Win32 helpers: desktop pinning (ported from Lumen's `main/win32.ts`),
//! launching, shell icons, wallpaper, autostart and single instance.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HWND, RECT, SIZE};
use windows::Win32::Graphics::Dwm::{
    DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DwmExtendFrameIntoClientArea,
    DwmSetWindowAttribute,
};
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits,
    GetObjectW, HGDIOBJ, ReleaseDC,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RegDeleteKeyValueW, RegSetKeyValueW};
use windows::Win32::System::Threading::{CreateMutexW, GetCurrentProcessId};
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
    ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GW_HWNDNEXT, GWL_EXSTYLE, GWLP_HWNDPARENT, GetClassNameW, GetTopWindow, GetWindow,
    GetWindowLongPtrW, GetWindowThreadProcessId, IsWindowVisible, SPI_GETDESKWALLPAPER,
    SPI_GETWORKAREA, SW_SHOWNORMAL, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SetWindowLongPtrW, SetWindowPos,
    SystemParametersInfoW, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};
use windows::core::PCWSTR;

pub fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
}

/// Raw HWND value, so it can be stored in plain structs.
pub type Hwnd = isize;

fn hwnd(h: Hwnd) -> HWND {
    HWND(h as *mut _)
}

/// Find one of our own top-level windows by its (unique) title.
pub fn find_own_window(title: &str) -> Option<Hwnd> {
    let title = wide(title);
    unsafe {
        let h = FindWindowW(PCWSTR::null(), PCWSTR(title.as_ptr())).ok()?;
        if h.0.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(h, Some(&mut pid));
        (pid == GetCurrentProcessId()).then_some(h.0 as Hwnd)
    }
}

/// Tool window (no Alt+Tab, no taskbar) owned by the desktop so that
/// "Show desktop" (Win+D) leaves the widgets visible.
pub fn prepare_widget_window(h: Hwnd) {
    own_by_desktop(h);
    prepare_glass_window(h);
}

/// Tool window (no Alt+Tab, no taskbar) owned by the desktop, without any
/// transparency tricks; used as-is by the Tauri edition, whose WebView2
/// windows already have real per-pixel alpha.
pub fn own_by_desktop(h: Hwnd) {
    let w = hwnd(h);
    unsafe {
        let style = GetWindowLongPtrW(w, GWL_EXSTYLE);
        let next = (style | WS_EX_TOOLWINDOW.0 as isize) & !(WS_EX_APPWINDOW.0 as isize);
        if next != style {
            SetWindowLongPtrW(w, GWL_EXSTYLE, next);
        }
        if let Ok(progman) = FindWindowW(windows::core::w!("Progman"), PCWSTR::null()) {
            if !progman.0.is_null() {
                SetWindowLongPtrW(w, GWLP_HWNDPARENT, progman.0 as isize);
            }
        }
    }
}

/// Per-pixel transparency for a borderless GL window: square DWM corners
/// (the app paints its own rounded glass), no Windows 11 border, and the
/// DWM frame extended over the client area so alpha is composited.
const DWM_DEFAULT: u32 = 17;

/// The clear colour widgets leave outside their glass, as 0x00BBGGRR.
pub const KEY_COLORREF: u32 = 0x0000_0000;
/// Same colour for egui's clear (gamma-space floats).
pub const KEY_CLEAR: [f32; 4] = [0.0, 0.0, 0.0, 0.0];

pub fn prepare_glass_window(h: Hwnd) {
    let w = hwnd(h);
    unsafe {
        let pref = DWMWCP_DONOTROUND;
        let _ = DwmSetWindowAttribute(
            w,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const _ as *const _,
            std::mem::size_of_val(&pref) as u32,
        );
        let mode: u32 = std::env::var("OPAL_DWM").ok().and_then(|v| v.parse().ok()).unwrap_or(DWM_DEFAULT);
        if mode & 1 != 0 {
            // No 1px Windows 11 border around the (square) window bounds.
            let none: u32 = 0xFFFF_FFFE; // DWMWA_COLOR_NONE
            let _ = DwmSetWindowAttribute(w, DWMWA_BORDER_COLOR, &none as *const _ as *const _, 4);
        }
        if mode & 2 != 0 {
            // Per-pixel alpha: extend the DWM frame over the whole client
            // area so the GL surface's alpha channel is composited.
            let m = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
            let _ = DwmExtendFrameIntoClientArea(w, &m);
        }
        if mode & 4 != 0 {
            // No Mica/acrylic system backdrop behind transparent pixels.
            let none: u32 = 1; // DWMSBT_NONE
            let _ = DwmSetWindowAttribute(w, windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(38), &none as *const _ as *const _, 4);
        }
        if mode & 16 != 0 {
            // Colour-key transparency: every pixel left at the clear colour
            // (pure black) is cut out by Windows, so the rounded corners show
            // the desktop. OpenGL windows cannot use per-pixel alpha here.
            use windows::Win32::UI::WindowsAndMessaging::{LWA_COLORKEY, SetLayeredWindowAttributes, WS_EX_LAYERED};
            let style = GetWindowLongPtrW(w, GWL_EXSTYLE);
            SetWindowLongPtrW(w, GWL_EXSTYLE, style | WS_EX_LAYERED.0 as isize);
            let _ = SetLayeredWindowAttributes(w, windows::Win32::Foundation::COLORREF(KEY_COLORREF), 0, LWA_COLORKEY);
        }
        if mode & 8 != 0 {
            // Dark immersive frame so any frame colour is dark, not white.
            let on: u32 = 1;
            let _ = DwmSetWindowAttribute(w, windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(20), &on as *const _ as *const _, 4);
        }
        let _ = SetWindowPos(
            w,
            HWND::default(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

fn class_name(w: HWND) -> String {
    let mut buf = [0u16; 256];
    let n = unsafe { GetClassNameW(w, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

/// Z-order the window directly above the desktop but below every app window.
pub fn pin_to_desktop(h: Hwnd) -> bool {
    let target = hwnd(h);
    unsafe {
        let mut after = HWND::default();
        let mut current = match GetTopWindow(HWND::default()) {
            Ok(c) => c,
            Err(_) => return false,
        };
        let mut guard = 0;
        while !current.0.is_null() && guard < 4096 {
            guard += 1;
            if IsWindowVisible(current).as_bool() {
                let class = class_name(current);
                if class == "Progman" || class == "WorkerW" {
                    break;
                }
            }
            if current != target {
                after = current;
            }
            current = match GetWindow(current, GW_HWNDNEXT) {
                Ok(c) => c,
                Err(_) => return false,
            };
        }
        if current.0.is_null() || guard >= 4096 || after.0.is_null() {
            return false;
        }
        // Already down there: only this app's own windows (other widgets)
        // sit between it and the desktop. Without this, widgets keep
        // swapping places with each other.
        let me = windows::Win32::System::Threading::GetCurrentProcessId();
        let mut w = GetWindow(target, GW_HWNDNEXT).unwrap_or_default();
        let mut steps = 0;
        loop {
            if w == current {
                return true;
            }
            if w.0.is_null() || steps > 64 {
                break;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(w, Some(&mut pid));
            if pid != me && IsWindowVisible(w).as_bool() {
                break;
            }
            steps += 1;
            w = GetWindow(w, GW_HWNDNEXT).unwrap_or_default();
        }
        // SWP_NOOWNERZORDER: move this window only. Without it Windows also
        // re-stacks the owner (the desktop) and, with it, every other
        // widget, which dragged a widget in use back down.
        SetWindowPos(target, after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | windows::Win32::UI::WindowsAndMessaging::SWP_NOOWNERZORDER).is_ok()
    }
}

/// Bring a widget in front of the app windows without taking focus (a
/// desktop-owned window is not raised by Windows when clicked).
pub fn raise(h: Hwnd) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::HWND_TOP;
    unsafe { SetWindowPos(hwnd(h), HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | windows::Win32::UI::WindowsAndMessaging::SWP_NOOWNERZORDER).is_ok() }
}

pub fn shell_open(target: &str, args: Option<&str>, cwd: Option<&str>) -> Result<(), String> {
    let file = wide(target);
    let params = args.map(wide);
    let dir = cwd.map(wide);
    // The widgets sit just above the desktop, so Windows may refuse to bring
    // a freshly started app to the front. Let whatever we start take focus.
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow};
        let _ = AllowSetForegroundWindow(ASFW_ANY);
    }
    let result = unsafe {
        ShellExecuteW(
            HWND::default(),
            windows::core::w!("open"),
            PCWSTR(file.as_ptr()),
            params.as_ref().map_or(PCWSTR::null(), |p| PCWSTR(p.as_ptr())),
            dir.as_ref().map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr())),
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err(format!("Could not open {target} (code {})", result.0 as isize))
    }
}

pub fn reveal(path: &str) {
    let _ = std::process::Command::new("explorer.exe")
        .arg(format!("/select,{path}"))
        .spawn();
}

pub fn open_folder(path: &std::path::Path) {
    let _ = shell_open(&path.to_string_lossy(), None, None);
}

const RUN_KEY: PCWSTR = windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const RUN_VALUE: PCWSTR = windows::core::w!("Opal");

pub fn set_start_with_windows(enabled: bool) -> Result<(), String> {
    unsafe {
        // Entry written under the old working name (Lumen-rs).
        let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, windows::core::w!("LumenRs"));
        if enabled {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let cmd = wide(format!("\"{}\"", exe.display()));
            let err = RegSetKeyValueW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                RUN_VALUE,
                REG_SZ.0,
                Some(cmd.as_ptr() as *const _),
                (cmd.len() * 2) as u32,
            );
            if err.is_err() {
                return Err(format!("Registry write failed ({})", err.0));
            }
        } else {
            let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE);
        }
    }
    Ok(())
}

pub fn wallpaper_path() -> Option<String> {
    let mut buf = [0u16; 1024];
    unsafe {
        SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            buf.len() as u32,
            Some(buf.as_mut_ptr() as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .ok()?;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(0);
    let path = String::from_utf16_lossy(&buf[..len]);
    (!path.is_empty()).then_some(path)
}

/// Primary monitor work area in physical pixels (x, y, w, h).
pub fn work_area() -> Option<(i32, i32, i32, i32)> {
    let mut rect = RECT::default();
    unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut _ as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .ok()?;
    }
    Some((rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top))
}

/// Primary monitor size in physical pixels.
pub fn screen_size() -> (i32, i32) {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) }
}

/// Returns false when the named mutex already exists (another instance holds
/// it). See `claim_instance`.
pub fn single_instance_named(name: &str) -> bool {
    let wname = wide(name);
    unsafe {
        let handle = CreateMutexW(None, true, PCWSTR(wname.as_ptr()));
        if handle.is_err() {
            return true;
        }
        // Keep the handle for the life of the process.
        std::mem::forget(handle);
        GetLastError() != ERROR_ALREADY_EXISTS
    }
}

/// Opal runs one edition at a time (egui or Tauri): both share this lock.
const SHARED_LOCK: &str = "Local\\OpalDesktopWidgets";

pub enum Instance {
    /// Nothing else running: carry on.
    First,
    /// This edition is already running.
    SameRunning,
    /// The other edition holds the desktop.
    OtherRunning,
}

pub fn claim_instance(own_lock: &str) -> Instance {
    if !single_instance_named(own_lock) {
        return Instance::SameRunning;
    }
    if !single_instance_named(SHARED_LOCK) {
        return Instance::OtherRunning;
    }
    Instance::First
}

pub fn message_box(title: &str, text: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONINFORMATION, MB_OK, MessageBoxW};
    let (t, m) = (wide(title), wide(text));
    unsafe {
        MessageBoxW(None, PCWSTR(m.as_ptr()), PCWSTR(t.as_ptr()), MB_OK | MB_ICONINFORMATION);
    }
}

pub fn com_init() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
}

/// Shell icon for a file, folder, shortcut or `shell:AppsFolder\<aumid>`.
/// Returns straight-alpha RGBA. Call `com_init` on the thread first.
pub fn extract_icon(path: &str, size: i32) -> Option<(usize, usize, Vec<u8>)> {
    let wpath = wide(path);
    unsafe {
        let factory: IShellItemImageFactory =
            SHCreateItemFromParsingName(PCWSTR(wpath.as_ptr()), None).ok()?;
        let hbm = factory
            .GetImage(SIZE { cx: size, cy: size }, SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK)
            .ok()?;
        let mut bmp = BITMAP::default();
        let got = GetObjectW(
            HGDIOBJ(hbm.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bmp as *mut _ as *mut _),
        );
        if got == 0 || bmp.bmWidth <= 0 || bmp.bmHeight <= 0 {
            let _ = DeleteObject(HGDIOBJ(hbm.0));
            return None;
        }
        let (w, h) = (bmp.bmWidth as usize, bmp.bmHeight as usize);
        let mut info = BITMAPINFO::default();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w as i32,
            biHeight: -(h as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };
        let mut bgra = vec![0u8; w * h * 4];
        let dc = GetDC(HWND::default());
        let lines = GetDIBits(
            dc,
            hbm,
            0,
            h as u32,
            Some(bgra.as_mut_ptr() as *mut _),
            &mut info,
            DIB_RGB_COLORS,
        );
        ReleaseDC(HWND::default(), dc);
        let _ = DeleteObject(HGDIOBJ(hbm.0));
        if lines == 0 {
            return None;
        }
        let has_alpha = bgra.chunks_exact(4).any(|p| p[3] != 0);
        let mut rgba = Vec::with_capacity(bgra.len());
        for p in bgra.chunks_exact(4) {
            let a = if has_alpha { p[3] } else { 255 };
            // The shell hands back premultiplied alpha.
            let un = |c: u8| -> u8 {
                if a == 0 || !has_alpha {
                    c
                } else {
                    ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8
                }
            };
            rgba.extend_from_slice(&[un(p[2]), un(p[1]), un(p[0]), a]);
        }
        Some((w, h, rgba))
    }
}

/// The Windows "notification" sound.
pub fn beep() {
    use windows::Win32::System::Diagnostics::Debug::MessageBeep;
    use windows::Win32::UI::WindowsAndMessaging::MB_ICONASTERISK;
    unsafe {
        let _ = MessageBeep(MB_ICONASTERISK);
    }
}
