//! Your own music files: folders or files you add (button or drag and drop).
//!
//! The UI keeps the list of paths; here we let the webview read them (asset
//! protocol scope), walk folders for audio files, read their tags (title,
//! artist, album, length) and, on request, the cover picture.

use std::path::{Path, PathBuf};

use base64::Engine;
use lofty::config::ParseOptions;
use lofty::prelude::*;
use lofty::probe::Probe;
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

const AUDIO: &[&str] = &["mp3", "m4a", "aac", "flac", "wav", "ogg", "oga", "opus", "wma", "aiff", "aif", "alac", "webm"];
const MEDIA: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "bmp", "mp4", "webm", "mov", "mkv"];
const MAX_FILES: usize = 5000;

#[derive(Serialize, Clone, Debug)]
pub struct LocalTrack {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
}

#[derive(Serialize, Default)]
pub struct Imported {
    /// audio files found (folders walked)
    pub tracks: Vec<LocalTrack>,
    /// folders and audio files to remember
    pub sources: Vec<String>,
    /// pictures / videos (for the wallpaper)
    pub media: Vec<String>,
}

fn ext(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

fn is_audio(p: &Path) -> bool {
    AUDIO.contains(&ext(p).as_str())
}

/// Let the webview read this file or folder through the asset protocol.
fn allow(app: &AppHandle, p: &Path) {
    let scope = app.asset_protocol_scope();
    if p.is_dir() {
        let _ = scope.allow_directory(p, true);
    } else {
        let _ = scope.allow_file(p);
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if out.len() >= MAX_FILES {
            return;
        }
        if p.is_dir() {
            walk(&p, out);
        } else if is_audio(&p) {
            out.push(p);
        }
    }
}

/// Title / artist from the tags, else from the file name ("Artist - Title").
fn read_tags(p: &Path) -> LocalTrack {
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").trim().to_string();
    let (mut artist, mut title) = match stem.split_once(" - ") {
        Some((a, t)) => (a.trim().to_string(), t.trim().to_string()),
        None => (String::new(), stem.clone()),
    };
    let mut album = p.parent().and_then(|d| d.file_name()).and_then(|s| s.to_str()).unwrap_or("").to_string();
    let mut duration_ms = 0u64;
    let opts = ParseOptions::new().read_cover_art(false);
    if let Ok(f) = Probe::open(p).and_then(|pr| pr.options(opts).read()) {
        duration_ms = f.properties().duration().as_millis() as u64;
        if let Some(tag) = f.primary_tag().or_else(|| f.first_tag()) {
            if let Some(t) = tag.title().filter(|t| !t.trim().is_empty()) {
                title = t.trim().to_string();
            }
            if let Some(a) = tag.artist().filter(|a| !a.trim().is_empty()) {
                artist = a.trim().to_string();
            }
            if let Some(a) = tag.album().filter(|a| !a.trim().is_empty()) {
                album = a.trim().to_string();
            }
        }
    }
    LocalTrack { path: p.to_string_lossy().to_string(), title, artist, album, duration_ms }
}

/// Folders and files you add or drop: audio is read (folders walked), pictures and
/// videos are passed back for the wallpaper. Everything returned can be read by the UI.
pub fn import(app: &AppHandle, paths: &[String]) -> Imported {
    let mut out = Imported::default();
    let mut files = Vec::new();
    for s in paths {
        let p = PathBuf::from(s);
        if !p.exists() {
            continue;
        }
        if p.is_dir() {
            allow(app, &p);
            let before = files.len();
            walk(&p, &mut files);
            if files.len() > before {
                out.sources.push(s.clone());
            }
        } else if is_audio(&p) {
            allow(app, &p);
            files.push(p);
            out.sources.push(s.clone());
        } else if MEDIA.contains(&ext(&p).as_str()) {
            allow(app, &p);
            out.media.push(s.clone());
        }
    }
    files.dedup();
    out.tracks = files.iter().map(|p| read_tags(p)).collect();
    out
}

/// Only re-open access to remembered folders / files (fast, no scanning).
pub fn allow_all(app: &AppHandle, paths: &[String]) {
    for s in paths {
        let p = PathBuf::from(s);
        if p.exists() {
            allow(app, &p);
        }
    }
}

/// The cover picture inside an audio file, as a data URL.
pub fn art(path: &str) -> Option<String> {
    let f = Probe::open(path).ok()?.read().ok()?;
    let tag = f.primary_tag().or_else(|| f.first_tag())?;
    let pic = tag.pictures().iter().find(|p| p.pic_type() == lofty::picture::PictureType::CoverFront).or_else(|| tag.pictures().first())?;
    if pic.data().len() > 8 * 1024 * 1024 {
        return None;
    }
    let mime = pic.mime_type().map(|m| m.as_str().to_string()).unwrap_or_else(|| "image/jpeg".into());
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(pic.data())))
}

/// "Add a folder…": the Windows folder picker.
pub fn pick_folder(app: &AppHandle) -> Option<String> {
    let p = app.dialog().file().set_title("Add a music folder").blocking_pick_folder()?;
    p.into_path().ok().map(|p| p.to_string_lossy().to_string())
}
