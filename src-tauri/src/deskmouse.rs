//! Mouse forwarding for "Wallpaper" mode (the way Wallpaper Engine does it).
//!
//! In Wallpaper mode the skin lives inside the shell's WorkerW, *behind* the desktop
//! icons, so Windows never delivers mouse input to it: the icon layer is on top.
//! A low-level mouse hook watches the cursor instead. When it is over empty desktop
//! (not over an icon, not over another app) the position and buttons are sent to the
//! page as `desk-mouse` events, which the page turns into hover / click / drag / wheel.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
use windows::Win32::System::Memory::{VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::desktop;

struct Ev {
    k: &'static str,
    x: i32,
    y: i32,
    d: i32,
}

static TX: OnceLock<Mutex<Sender<Ev>>> = OnceLock::new();
static CAPTURED: AtomicBool = AtomicBool::new(false);
static OVER: AtomicBool = AtomicBool::new(false);
static LAST_MOVE: Mutex<Option<Instant>> = Mutex::new(None);
static OWN_PID: AtomicIsize = AtomicIsize::new(0);

fn send(k: &'static str, x: i32, y: i32, d: i32) {
    if let Some(tx) = TX.get() {
        if let Ok(tx) = tx.lock() {
            let _ = tx.send(Ev { k, x, y, d });
        }
    }
}

unsafe fn class_of(h: HWND) -> String {
    let mut b = [0u16; 64];
    let n = GetClassNameW(h, &mut b) as usize;
    String::from_utf16_lossy(&b[..n.min(64)])
}

/// Is there an icon under the cursor? (asks the desktop's list view; needs memory inside explorer)
unsafe fn over_icon(lv: HWND, pt: POINT) -> bool {
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct HitTest {
        x: i32,
        y: i32,
        flags: u32,
        item: i32,
        sub: i32,
        group: i32,
    }
    let mut c = pt;
    if !ScreenToClient(lv, &mut c).as_bool() {
        return false;
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(lv, Some(&mut pid));
    let Ok(proc) = OpenProcess(PROCESS_VM_OPERATION | PROCESS_VM_READ | PROCESS_VM_WRITE, false, pid) else {
        return false;
    };
    let size = std::mem::size_of::<HitTest>();
    let remote = VirtualAllocEx(proc, None, size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
    let mut hit = false;
    if !remote.is_null() {
        let h = HitTest { x: c.x, y: c.y, ..Default::default() };
        if WriteProcessMemory(proc, remote, &h as *const _ as *const _, size, None).is_ok() {
            // LVM_HITTEST = 0x1012; result is the item index (-1 = none)
            let mut res: usize = 0;
            let _ = SendMessageTimeoutW(lv, 0x1012, WPARAM(0), LPARAM(remote as isize), SMTO_ABORTIFHUNG, 80, Some(&mut res));
            let mut out = HitTest::default();
            if ReadProcessMemory(proc, remote, &mut out as *mut _ as *mut _, size, None).is_ok() {
                // LVHT_ONITEM = 0x0E (icon, label or state image)
                hit = out.item >= 0 && (out.flags & 0x0E) != 0;
            }
        }
        let _ = VirtualFreeEx(proc, remote, 0, MEM_RELEASE);
    }
    let _ = CloseHandle(proc);
    hit
}

/// Cursor is over the bare desktop (or over us), i.e. nothing else would take the click.
unsafe fn over_desktop(pt: POINT, check_icon: bool) -> bool {
    let w = WindowFromPoint(pt);
    if w.is_invalid() {
        return false;
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(w, Some(&mut pid));
    if pid as isize == OWN_PID.load(Ordering::Relaxed) {
        return true;
    }
    match class_of(w).as_str() {
        "SysListView32" => !(check_icon && over_icon(w, pt)),
        "SHELLDLL_DefView" | "WorkerW" | "Progman" => true,
        _ => false,
    }
}

unsafe extern "system" fn hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code < 0 || !desktop::is_wallpaper() {
        return CallNextHookEx(None, code, wp, lp);
    }
    let ms = &*(lp.0 as *const MSLLHOOKSTRUCT);
    if ms.flags & 1 != 0 {
        return CallNextHookEx(None, code, wp, lp); // injected by us / another app
    }
    let pt = ms.pt;
    let (mx, my, sw, sh) = desktop::target_rect();
    let inside = pt.x >= mx && pt.x < mx + sw && pt.y >= my && pt.y < my + sh;
    let (x, y) = (pt.x - mx, pt.y - my);
    let msg = wp.0 as u32;
    let captured = CAPTURED.load(Ordering::Relaxed);

    match msg {
        WM_MOUSEMOVE => {
            if captured {
                send("move", x, y, 0);
            } else if inside && over_desktop(pt, false) {
                OVER.store(true, Ordering::Relaxed);
                let mut last = LAST_MOVE.lock().unwrap_or_else(|e| e.into_inner());
                if last.map(|t| t.elapsed() >= Duration::from_millis(14)).unwrap_or(true) {
                    *last = Some(Instant::now());
                    send("move", x, y, 0);
                }
            } else if OVER.swap(false, Ordering::Relaxed) {
                send("leave", 0, 0, 0);
            }
        }
        WM_LBUTTONDOWN => {
            if inside && over_desktop(pt, true) {
                CAPTURED.store(true, Ordering::Relaxed);
                send("down", x, y, 0);
                return LRESULT(1); // the desktop must not start a selection rectangle
            }
        }
        WM_LBUTTONUP => {
            if captured {
                CAPTURED.store(false, Ordering::Relaxed);
                send("up", x, y, 0);
                return LRESULT(1);
            }
        }
        WM_MOUSEWHEEL => {
            if inside && over_desktop(pt, false) {
                send("wheel", x, y, ((ms.mouseData >> 16) as i16) as i32);
            }
        }
        _ => {}
    }
    CallNextHookEx(None, code, wp, lp)
}

/// Start the hook (own thread with a message loop) and the thread that relays events to the page.
pub fn start(app: AppHandle) {
    let (tx, rx) = channel::<Ev>();
    let _ = TX.set(Mutex::new(tx));
    unsafe { OWN_PID.store(GetCurrentProcessId() as isize, Ordering::Relaxed) };
    std::thread::spawn(move || {
        for e in rx {
            let _ = app.emit("desk-mouse", json!({"k": e.k, "x": e.x, "y": e.y, "d": e.d}));
        }
    });
    std::thread::spawn(|| unsafe {
        if SetWindowsHookExW(WH_MOUSE_LL, Some(hook), None, 0).is_err() {
            return;
        }
        let mut m = MSG::default();
        while GetMessageW(&mut m, HWND::default(), 0, 0).as_bool() {
            let _ = TranslateMessage(&m);
            DispatchMessageW(&m);
        }
    });
}
