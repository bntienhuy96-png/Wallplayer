use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Settings {
    /// true = behind desktop icons (WorkerW), false = interactive desktop layer
    #[serde(default)]
    pub wallpaper_mode: bool,
    /// settings layout version (2 = Wallpaper Engine style placement is the default)
    #[serde(default)]
    pub mode_ver: u8,
    /// hide all desktop icons (incl. Recycle Bin) while Wallplayer runs; default on
    #[serde(default = "yes")]
    pub hide_icons: bool,
    #[serde(default)]
    pub spotify_client_id: String,
    #[serde(default)]
    pub spotify_refresh: String,
    /// monitor device name ("" = primary)
    #[serde(default)]
    pub monitor: String,
}

fn yes() -> bool {
    true
}

fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("settings.json"))
}

pub fn load(app: &AppHandle) -> Settings {
    path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
        .map(|mut s| {
            if s.mode_ver < 2 {
                s.wallpaper_mode = true;
                s.mode_ver = 2;
            }
            s
        })
        .unwrap_or_else(|| Settings { wallpaper_mode: true, mode_ver: 2, hide_icons: true, ..Default::default() })
}

pub fn save(app: &AppHandle, s: &Settings) {
    if let Some(p) = path(app) {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, serde_json::to_string_pretty(s).unwrap_or_default());
    }
}

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_NAME: &str = "Wallplayer";

pub fn autostart_enabled() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(RUN_KEY)
        .and_then(|k| k.get_value::<String, _>(RUN_NAME))
        .is_ok()
}

pub fn set_autostart(on: bool) -> std::io::Result<()> {
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN_KEY)?;
    if on {
        let exe = std::env::current_exe()?;
        key.set_value(RUN_NAME, &format!("\"{}\"", exe.display()))
    } else {
        match key.delete_value(RUN_NAME) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
}
