//! Window placement on the Windows desktop.
//!
//! Three modes:
//! * `Desktop`   – borderless full-screen window pinned to the bottom of the
//!                 z-order (covers the desktop icons, fully interactive).
//! * `Wallpaper` – window re-parented into the shell's WorkerW, i.e. drawn
//!                 behind the desktop icons like a real wallpaper.
//! * `Peek`      – temporarily top-most + focused (global hotkey search).

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Mutex;

use serde::Serialize;

use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, LRESULT, RECT, TRUE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromWindow, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::Shell::{
    DefSubclassProc, SHAppBarMessage, SetCurrentProcessExplicitAppUserModelID, SetWindowSubclass, ShellExecuteW,
    ABM_GETSTATE, APPBARDATA,
};
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Desktop = 0,
    Wallpaper = 1,
    Peek = 2,
}

impl Mode {
    fn from_u8(v: u8) -> Mode {
        match v {
            1 => Mode::Wallpaper,
            2 => Mode::Peek,
            _ => Mode::Desktop,
        }
    }
}

/// True while the skin is deliberately raised above the (empty) desktop,
/// e.g. after Win+D / "show desktop".
static FRONT: AtomicBool = AtomicBool::new(false);
static MODE: AtomicU8 = AtomicU8::new(Mode::Desktop as u8);
/// Device name of the monitor to cover ("" = primary).
static TARGET: Mutex<String> = Mutex::new(String::new());

#[derive(Serialize, Clone, Debug)]
pub struct Monitor {
    pub id: String,
    pub primary: bool,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

unsafe extern "system" fn mon_cb(hm: HMONITOR, _hdc: HDC, _r: *mut RECT, lp: LPARAM) -> BOOL {
    let v = &mut *(lp.0 as *mut Vec<Monitor>);
    let mut mi = MONITORINFOEXW::default();
    mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if GetMonitorInfoW(hm, &mut mi as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool() {
        let r = mi.monitorInfo.rcMonitor;
        let id = String::from_utf16_lossy(&mi.szDevice).trim_end_matches('\0').to_string();
        v.push(Monitor { id, primary: mi.monitorInfo.dwFlags & 1 != 0, x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top });
    }
    TRUE
}

/// All monitors, left to right.
pub fn monitors() -> Vec<Monitor> {
    let mut v: Vec<Monitor> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(HDC::default(), None, Some(mon_cb), LPARAM(&mut v as *mut Vec<Monitor> as isize));
    }
    v.sort_by_key(|m| (m.x, m.y));
    v
}

pub fn set_target(id: &str) {
    *TARGET.lock().unwrap() = id.to_string();
}

/// Device name of the monitor the main skin is on (chosen one, else primary).
pub fn target_id() -> String {
    let want = TARGET.lock().unwrap().clone();
    let list = monitors();
    list.iter()
        .find(|m| !want.is_empty() && m.id == want)
        .or_else(|| list.iter().find(|m| m.primary))
        .or_else(|| list.first())
        .map(|m| m.id.clone())
        .unwrap_or_default()
}

/// Rect of the chosen monitor (falls back to the primary one).
fn target_rect() -> (i32, i32, i32, i32) {
    let want = TARGET.lock().unwrap().clone();
    let list = monitors();
    let m = list
        .iter()
        .find(|m| !want.is_empty() && m.id == want)
        .or_else(|| list.iter().find(|m| m.primary))
        .or_else(|| list.first());
    match m {
        Some(m) => (m.x, m.y, m.w, m.h),
        None => unsafe { (0, 0, GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) },
    }
}

/// Companion windows on the other monitors: (hwnd, monitor device name).
static MIRRORS: Mutex<Vec<(isize, String)>> = Mutex::new(Vec::new());

pub fn add_mirror(raw: isize, monitor: &str) {
    let mut v = MIRRORS.lock().unwrap();
    v.retain(|(h, _)| *h != raw);
    v.push((raw, monitor.to_string()));
}

pub fn remove_mirror(raw: isize) {
    MIRRORS.lock().unwrap().retain(|(h, _)| *h != raw);
}

fn mirror_handles() -> Vec<isize> {
    MIRRORS.lock().unwrap().iter().map(|(h, _)| *h).collect()
}

/// Rect a given window should cover: its own monitor for a mirror, else the target.
fn rect_for(h: HWND) -> (i32, i32, i32, i32) {
    let mon = MIRRORS.lock().unwrap().iter().find(|(m, _)| *m == h.0 as isize).map(|(_, id)| id.clone());
    if let Some(id) = mon {
        if let Some(m) = monitors().into_iter().find(|m| m.id == id) {
            return (m.x, m.y, m.w, m.h);
        }
    }
    target_rect()
}

fn is_mirror(h: HWND) -> bool {
    MIRRORS.lock().unwrap().iter().any(|(m, _)| *m == h.0 as isize)
}

pub const APP_ID: &str = "com.tan.wallplayer";

fn hwnd(raw: isize) -> HWND {
    HWND(raw as *mut core::ffi::c_void)
}

/// Give the process its own AppUserModelID so our own media session can be
/// told apart from Spotify / browsers in the system media list.
pub fn set_app_id() {
    unsafe {
        let id = HSTRING::from(APP_ID);
        let _ = SetCurrentProcessExplicitAppUserModelID(&id);
    }
}

/// Hide from Alt+Tab and hook z-order changes.
pub fn install(raw: isize) {
    unsafe {
        let h = hwnd(raw);
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
        SetWindowLongPtrW(h, GWL_EXSTYLE, ex | WS_EX_TOOLWINDOW.0 as isize);
        let _ = SetWindowSubclass(h, Some(subclass_proc), 0x5750, 0);
    }
}

unsafe extern "system" fn subclass_proc(
    h: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    match msg {
        // Keep the window glued to the bottom of the z-order in Desktop mode,
        // even when the user clicks it.
        WM_WINDOWPOSCHANGING => {
            if MODE.load(Ordering::Relaxed) == Mode::Desktop as u8 && !FRONT.load(Ordering::Relaxed) {
                let p = lp.0 as *mut WINDOWPOS;
                if !p.is_null() {
                    (*p).hwndInsertAfter = HWND_BOTTOM;
                    (*p).flags = SET_WINDOW_POS_FLAGS((*p).flags.0 & !SWP_NOZORDER.0);
                }
            }
        }
        // Resolution / monitor change: refit.
        WM_DISPLAYCHANGE => {
            let _ = place(h, mode_for(h));
        }
        // Moving to a monitor with another scale: let the window handle it, then refit.
        WM_DPICHANGED => {
            let r = DefSubclassProc(h, msg, wp, lp);
            let _ = place(h, mode_for(h));
            return r;
        }
        _ => {}
    }
    DefSubclassProc(h, msg, wp, lp)
}

/// Mode a window should be in: the main window's mode, Peek → Desktop for mirrors.
fn mode_for(h: HWND) -> Mode {
    let m = Mode::from_u8(MODE.load(Ordering::Relaxed));
    if m == Mode::Peek && is_mirror(h) {
        Mode::Desktop
    } else {
        m
    }
}

pub fn set_mode(raw: isize, mode: Mode) -> Result<(), String> {
    unsafe { place(hwnd(raw), mode) }
}

/// Place every mirror window in the given mode (Peek is never used for them).
pub fn place_mirrors(mode: Mode) {
    let mode = if mode == Mode::Peek { Mode::Desktop } else { mode };
    for raw in mirror_handles() {
        unsafe {
            if place(hwnd(raw), mode).is_err() {
                let _ = place(hwnd(raw), Mode::Desktop);
            }
        }
    }
}

/// Current placement mode of the main skin.
pub fn current_mode() -> Mode {
    Mode::from_u8(MODE.load(Ordering::Relaxed))
}

unsafe fn place(h: HWND, mode: Mode) -> Result<(), String> {
    let (mx, my, sw, sh) = rect_for(h);
    // Mirrors follow the main window's mode; only the main window sets it.
    let mirror = is_mirror(h);
    let store_mode = |m: Mode| {
        if !mirror {
            MODE.store(m as u8, Ordering::SeqCst);
        }
    };

    if mode == Mode::Wallpaper {
        let worker = find_worker().ok_or_else(|| "desktop layer (WorkerW) not found".to_string())?;
        store_mode(mode);
        let style = GetWindowLongPtrW(h, GWL_STYLE);
        SetWindowLongPtrW(
            h,
            GWL_STYLE,
            (style & !(WS_POPUP.0 as isize)) | WS_CHILD.0 as isize,
        );
        let _ = SetParent(h, worker);
        if GetParent(h).ok() != Some(worker) {
            // roll back
            let style = GetWindowLongPtrW(h, GWL_STYLE);
            SetWindowLongPtrW(h, GWL_STYLE, (style & !(WS_CHILD.0 as isize)) | WS_POPUP.0 as isize);
            store_mode(Mode::Desktop);
            return Err("could not attach to desktop layer".into());
        }
        // WorkerW spans the whole virtual screen; the primary monitor sits at
        // (-vx, -vy) inside it.
        let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
        SetWindowPos(
            h,
            HWND::default(),
            mx - vx,
            my - vy,
            sw,
            sh,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_SHOWWINDOW,
        )
        .map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Detach from WorkerW if needed.
    let is_child = GetParent(h).map(|p| !p.is_invalid()).unwrap_or(false)
        && (GetWindowLongPtrW(h, GWL_STYLE) & WS_CHILD.0 as isize) != 0;
    store_mode(mode);
    if is_child {
        let _ = SetParent(h, HWND::default());
        let style = GetWindowLongPtrW(h, GWL_STYLE);
        SetWindowLongPtrW(h, GWL_STYLE, (style & !(WS_CHILD.0 as isize)) | WS_POPUP.0 as isize);
    }

    let after = if mode == Mode::Peek { HWND_TOPMOST } else { HWND_BOTTOM };
    let mut flags = SWP_SHOWWINDOW | SWP_FRAMECHANGED;
    if mode != Mode::Peek {
        flags |= SWP_NOACTIVATE;
    }
    // One pixel short of the monitor height: Windows then never treats the
    // skin as a full-screen app, so the taskbar stays on top / still pops up
    // on hover (auto-hide) even while the skin has focus.
    SetWindowPos(h, after, mx, my, sw, sh - 1, flags).map_err(|e| e.to_string())?;
    if mode == Mode::Peek {
        let _ = SetForegroundWindow(h);
    }
    Ok(())
}

/// Locate the WorkerW that sits behind the desktop icons.
/// Handles both the classic layout and Windows 11 24H2+ (WorkerW as a child
/// of Progman).
unsafe fn find_worker() -> Option<HWND> {
    let progman = FindWindowW(w!("Progman"), PCWSTR::null()).ok()?;
    if progman.is_invalid() {
        return None;
    }
    // Ask Progman to spawn the WorkerW behind the icons.
    let mut res: usize = 0;
    let _ = SendMessageTimeoutW(
        progman,
        0x052C,
        WPARAM(0xD),
        LPARAM(0x1),
        SMTO_NORMAL,
        1000,
        Some(&mut res),
    );
    let _ = SendMessageTimeoutW(progman, 0x052C, WPARAM(0), LPARAM(0), SMTO_NORMAL, 1000, Some(&mut res));

    // 24H2+: WorkerW is a child of Progman.
    if let Ok(w) = FindWindowExW(progman, HWND::default(), w!("WorkerW"), PCWSTR::null()) {
        if !w.is_invalid() {
            return Some(w);
        }
    }

    // Classic: the WorkerW right after the top-level window hosting SHELLDLL_DefView.
    let mut worker = HWND::default();
    let _ = EnumWindows(Some(enum_proc), LPARAM(&mut worker as *mut HWND as isize));
    if worker.is_invalid() {
        None
    } else {
        Some(worker)
    }
}

unsafe extern "system" fn enum_proc(top: HWND, lp: LPARAM) -> BOOL {
    if let Ok(dv) = FindWindowExW(top, HWND::default(), w!("SHELLDLL_DefView"), PCWSTR::null()) {
        if !dv.is_invalid() {
            if let Ok(w) = FindWindowExW(HWND::default(), top, w!("WorkerW"), PCWSTR::null()) {
                if !w.is_invalid() {
                    *(lp.0 as *mut HWND) = w;
                }
            }
        }
    }
    TRUE
}

/// Open a URL or URI (https:, spotify:, ...) with the default handler.
pub fn open_external(url: &str) {
    unsafe {
        let u = HSTRING::from(url);
        ShellExecuteW(
            HWND::default(),
            w!("open"),
            &u,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

/// Bring up the taskbar of the monitor the skin is on (also slides out an auto-hidden one).
pub fn reveal_taskbar(raw: isize) {
    unsafe {
        let ours = MonitorFromWindow(hwnd(raw), MONITOR_DEFAULTTONEAREST);
        let mut bars: Vec<HWND> = Vec::new();
        if let Ok(t) = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) {
            if !t.is_invalid() {
                bars.push(t);
            }
        }
        let mut prev = HWND::default();
        while let Ok(t) = FindWindowExW(HWND::default(), prev, w!("Shell_SecondaryTrayWnd"), PCWSTR::null()) {
            if t.is_invalid() {
                break;
            }
            bars.push(t);
            prev = t;
        }
        let Some(tray) = bars.iter().copied().find(|b| MonitorFromWindow(*b, MONITOR_DEFAULTTONEAREST) == ours).or(bars.first().copied()) else {
            return;
        };
        let mut abd = APPBARDATA { cbSize: std::mem::size_of::<APPBARDATA>() as u32, ..Default::default() };
        let state = SHAppBarMessage(ABM_GETSTATE, &mut abd) as u32;
        let _ = SetWindowPos(
            tray,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        // ABS_AUTOHIDE = 1: an auto-hidden taskbar slides out when it gets focus
        if state & 1 != 0 {
            let _ = SetForegroundWindow(tray);
        }
    }
}

unsafe fn class_of(h: HWND) -> String {
    let mut b = [0u16; 64];
    let n = GetClassNameW(h, &mut b) as usize;
    String::from_utf16_lossy(&b[..n.min(64)])
}

/// "Show desktop" (Win+D, the taskbar corner, Win+M ...) makes Windows focus
/// the desktop itself. In Desktop mode the skin then rises above the empty
/// desktop instead of leaving the plain desktop on show, and drops back to the
/// bottom as soon as any real app window takes the focus again.
pub fn spawn_watcher(raw: isize) {
    std::thread::spawn(move || unsafe {
        let h = hwnd(raw);
        loop {
            std::thread::sleep(std::time::Duration::from_millis(120));
            if MODE.load(Ordering::Relaxed) != Mode::Desktop as u8 {
                FRONT.store(false, Ordering::Relaxed);
                continue;
            }
            let fg = GetForegroundWindow();
            if fg.is_invalid() {
                continue;
            }
            let cls = class_of(fg);
            let on_desktop = cls == "Progman" || cls == "WorkerW";
            let neutral = fg == h
                || is_mirror(fg)
                || matches!(
                    cls.as_str(),
                    "Shell_TrayWnd"
                        | "Shell_SecondaryTrayWnd"
                        | "TopLevelWindowForOverflowXamlIsland"
                        | "XamlExplorerHostIslandWindow"
                        | "Windows.UI.Core.CoreWindow"
                        | "NotifyIconOverflowWindow"
                );
            if on_desktop {
                let (mx, my, sw, sh) = target_rect();
                if !FRONT.swap(true, Ordering::Relaxed) || IsIconic(h).as_bool() || !IsWindowVisible(h).as_bool() {
                    if IsIconic(h).as_bool() {
                        let _ = ShowWindow(h, SW_SHOWNOACTIVATE);
                    }
                    let _ = SetWindowPos(h, HWND_TOP, mx, my, sw, sh - 1, SWP_NOACTIVATE | SWP_SHOWWINDOW);
                    for m in mirror_handles() {
                        let mh = hwnd(m);
                        let (x, y, w, hh) = rect_for(mh);
                        let _ = SetWindowPos(mh, HWND_TOP, x, y, w, hh - 1, SWP_NOACTIVATE | SWP_SHOWWINDOW);
                    }
                }
            } else if !neutral && FRONT.swap(false, Ordering::Relaxed) {
                let _ = SetWindowPos(h, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE);
                for m in mirror_handles() {
                    let _ = SetWindowPos(hwnd(m), HWND_BOTTOM, 0, 0, 0, 0, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE);
                }
            }
        }
    });
}

/// Average colours of the picture currently shown on the target monitor
/// (the playing music video / video wallpaper). The screen is shrunk to a
/// tiny bitmap and two quiet zones, away from the UI and the edge light,
/// are averaged: left and right of the middle. Returns [left, right] as RGB.
pub fn screen_colors() -> Option<[[u8; 3]; 2]> {
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC,
        SelectObject, SetStretchBltMode, StretchBlt, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
        HALFTONE, SRCCOPY,
    };
    unsafe {
        let (mx, my, sw, sh) = target_rect();
        let (tw, th) = (64i32, 36i32);
        let sdc = GetDC(HWND::default());
        if sdc.is_invalid() {
            return None;
        }
        let mdc = CreateCompatibleDC(sdc);
        let bmp = CreateCompatibleBitmap(sdc, tw, th);
        let old = SelectObject(mdc, bmp);
        SetStretchBltMode(mdc, HALFTONE);
        let ok = StretchBlt(mdc, 0, 0, tw, th, sdc, mx, my, sw, sh, SRCCOPY).as_bool();
        let mut out = None;
        if ok {
            let mut bi = BITMAPINFO::default();
            bi.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: tw,
                biHeight: -th,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            };
            let mut px = vec![0u8; (tw * th * 4) as usize];
            let n = GetDIBits(mdc, bmp, 0, th as u32, Some(px.as_mut_ptr() as *mut _), &mut bi, DIB_RGB_COLORS);
            if n > 0 {
                let zone = |x0: i32, x1: i32, y0: i32, y1: i32| -> [u8; 3] {
                    let (mut r, mut g, mut b, mut c) = (0u32, 0u32, 0u32, 0u32);
                    for y in y0..y1 {
                        for x in x0..x1 {
                            let i = ((y * tw + x) * 4) as usize;
                            b += px[i] as u32;
                            g += px[i + 1] as u32;
                            r += px[i + 2] as u32;
                            c += 1;
                        }
                    }
                    let c = c.max(1);
                    [(r / c) as u8, (g / c) as u8, (b / c) as u8]
                };
                out = Some([zone(12, 22, 8, 22), zone(42, 52, 8, 22)]);
            }
        }
        SelectObject(mdc, old);
        let _ = DeleteObject(bmp);
        let _ = DeleteDC(mdc);
        ReleaseDC(HWND::default(), sdc);
        out
    }
}
