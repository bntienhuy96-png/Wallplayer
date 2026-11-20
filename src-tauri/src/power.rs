//! Power awareness: battery / Energy Saver, and whether each of our windows is
//! hidden behind another app.
//!
//! Our windows sit at the very bottom of the z-order (or inside WorkerW), so a
//! window that fills a monitor hides everything we draw there. Every 400 ms the
//! thread works out, per monitor, whether a full-screen window (game, video,
//! presentation) or a maximised window covers it, and tells each of our
//! windows (`power-state`) about its own monitor, plus the power source. The UI
//! then rests: it stops drawing what nobody can see, and gets lighter on battery.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITORINFOEXW, MONITOR_DEFAULTTONEAREST};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::desktop;

/// UI preference: rest while a window covers the screen (on by default).
static COVER_PAUSE: AtomicBool = AtomicBool::new(true);
/// UI preference: lighter mode is in effect (battery / saver / chosen).
pub static LOW: AtomicBool = AtomicBool::new(false);

pub fn set_prefs(cover_pause: bool, low: bool) {
    COVER_PAUSE.store(cover_pause, Ordering::Relaxed);
    LOW.store(low, Ordering::Relaxed);
}

#[derive(Serialize, Clone, PartialEq, Default, Debug)]
pub struct PowerState {
    /// running on battery
    pub battery: bool,
    /// Windows Battery saver / Energy saver is on
    pub saver: bool,
    /// a full-screen app (game, video, slideshow) covers this window's monitor
    pub full: bool,
    /// a maximised window covers this window's monitor
    pub max: bool,
}

#[derive(Default, Clone, Copy)]
struct Cover {
    full: bool,
    max: bool,
}

struct Scan {
    pid: u32,
    /// monitor device name → what covers it
    cover: HashMap<String, Cover>,
}

unsafe fn monitor_of(w: HWND) -> Option<(String, RECT, RECT)> {
    let hm = MonitorFromWindow(w, MONITOR_DEFAULTTONEAREST);
    let mut mi = MONITORINFOEXW::default();
    mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if !GetMonitorInfoW(hm, &mut mi as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool() {
        return None;
    }
    let id = String::from_utf16_lossy(&mi.szDevice).trim_end_matches('\0').to_string();
    Some((id, mi.monitorInfo.rcMonitor, mi.monitorInfo.rcWork))
}

fn covers(r: &RECT, area: &RECT) -> bool {
    // a few pixels of slack: frame bounds can sit 1–2 px inside the edge
    r.left <= area.left + 2 && r.top <= area.top + 2 && r.right >= area.right - 2 && r.bottom >= area.bottom - 2
}

unsafe fn class_of(h: HWND) -> String {
    let mut b = [0u16; 64];
    let n = GetClassNameW(h, &mut b) as usize;
    String::from_utf16_lossy(&b[..n.min(64)])
}

unsafe extern "system" fn scan_cb(w: HWND, lp: LPARAM) -> BOOL {
    let st = &mut *(lp.0 as *mut Scan);
    if !IsWindowVisible(w).as_bool() || IsIconic(w).as_bool() {
        return TRUE;
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(w, Some(&mut pid as *mut u32));
    if pid == st.pid {
        return TRUE; // our own windows
    }
    let ex = GetWindowLongPtrW(w, GWL_EXSTYLE) as u32;
    // click-through overlays (game bars, screen dimmers…) hide nothing
    if ex & WS_EX_TRANSPARENT.0 != 0 && ex & WS_EX_LAYERED.0 != 0 {
        return TRUE;
    }
    let mut cloaked = 0u32;
    let _ = DwmGetWindowAttribute(w, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut _, 4);
    if cloaked != 0 {
        return TRUE; // other virtual desktops, suspended store apps
    }
    if matches!(class_of(w).as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
        return TRUE;
    }
    let mut r = RECT::default();
    if DwmGetWindowAttribute(w, DWMWA_EXTENDED_FRAME_BOUNDS, &mut r as *mut RECT as *mut _, std::mem::size_of::<RECT>() as u32).is_err() {
        if GetWindowRect(w, &mut r).is_err() {
            return TRUE;
        }
    }
    let Some((id, mon, work)) = monitor_of(w) else { return TRUE };
    let c = st.cover.entry(id).or_default();
    if covers(&r, &mon) {
        c.full = true;
    } else if IsZoomed(w).as_bool() || covers(&r, &work) {
        c.max = true;
    }
    TRUE
}

/// What covers each monitor right now.
fn scan() -> HashMap<String, Cover> {
    unsafe {
        let mut st = Scan { pid: GetCurrentProcessId(), cover: HashMap::new() };
        let _ = EnumWindows(Some(scan_cb), LPARAM(&mut st as *mut Scan as isize));
        // exclusive full-screen games do not always report a covering rect
        if let Ok(q) = SHQueryUserNotificationState() {
            if q == QUNS_RUNNING_D3D_FULL_SCREEN || q == QUNS_BUSY || q == QUNS_PRESENTATION_MODE {
                let fg = GetForegroundWindow();
                let mut pid = 0u32;
                GetWindowThreadProcessId(fg, Some(&mut pid as *mut u32));
                if !fg.is_invalid() && pid != st.pid {
                    if let Some((id, _, _)) = monitor_of(fg) {
                        st.cover.entry(id).or_default().full = true;
                    }
                }
            }
        }
        st.cover
    }
}

fn battery() -> (bool, bool) {
    unsafe {
        let mut s = SYSTEM_POWER_STATUS::default();
        if GetSystemPowerStatus(&mut s).is_err() {
            return (false, false);
        }
        // ACLineStatus 0 = unplugged; BatteryFlag 128 = no battery at all
        let on_battery = s.ACLineStatus == 0 && s.BatteryFlag != 128;
        (on_battery, s.SystemStatusFlag == 1)
    }
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last: HashMap<String, PowerState> = HashMap::new();
        let mut tick = 0u32;
        let (mut bat, mut saver) = battery();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(400));
            tick += 1;
            if tick % 12 == 0 {
                (bat, saver) = battery(); // every ~5 s is plenty
            }
            let cover = scan();
            let peek = desktop::current_mode() == desktop::Mode::Peek;
            let main_mon = desktop::target_id();
            let mut all_hidden = true;
            let mut any = false;
            for (label, w) in app.webview_windows() {
                let mon = if label == "main" {
                    Some(main_mon.clone())
                } else if label.starts_with("mirror-") {
                    w.hwnd().ok().and_then(|h| desktop::mirror_monitor(h.0 as isize))
                } else {
                    None
                };
                let Some(mon) = mon else { continue };
                any = true;
                let c = cover.get(&mon).copied().unwrap_or_default();
                // the search overlay floats above everything: never covered then
                let lifted = peek && label == "main";
                let ps = PowerState { battery: bat, saver, full: c.full && !lifted, max: c.max && !lifted };
                if !(ps.full || ps.max) {
                    all_hidden = false;
                }
                if last.get(&label) != Some(&ps) {
                    let _ = app.emit_to(label.as_str(), "power-state", &ps);
                    last.insert(label, ps);
                }
            }
            // nobody can see the light: stop analysing the sound too
            crate::audio::PAUSED.store(any && all_hidden && COVER_PAUSE.load(Ordering::Relaxed), Ordering::Relaxed);
        }
    });
}

/// The state right now for one of our windows (asked for when a page loads).
pub fn current(app: &AppHandle, label: &str) -> PowerState {
    let (bat, saver) = battery();
    let cover = scan();
    let mon = if label == "main" {
        Some(desktop::target_id())
    } else {
        app.get_webview_window(label).and_then(|w| w.hwnd().ok()).and_then(|h| desktop::mirror_monitor(h.0 as isize))
    };
    let c = mon.and_then(|m| cover.get(&m).copied()).unwrap_or_default();
    let lifted = label == "main" && desktop::current_mode() == desktop::Mode::Peek;
    PowerState { battery: bat, saver, full: c.full && !lifted, max: c.max && !lifted }
}
