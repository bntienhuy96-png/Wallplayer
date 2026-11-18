//! System "Now Playing" via Windows Global System Media Transport Controls
//! (the same source as the volume flyout). Works with Spotify, Apple Music,
//! browsers (YouTube Music, SoundCloud, Spotify Web Player), etc.

use std::time::Duration;

use base64::Engine;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Storage::Streams::DataReader;
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

#[derive(Serialize, Clone, Default, PartialEq, Debug)]
pub struct MediaState {
    pub has: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub app: String,
    pub playing: bool,
    pub position_ms: i64,
    pub duration_ms: i64,
    /// Unix ms at which `position_ms` was sampled by the source app.
    pub updated_ms: i64,
}

const FILETIME_UNIX_DIFF: i64 = 116_444_736_000_000_000;

fn init_thread() {
    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }
}

fn manager() -> Option<Manager> {
    Manager::RequestAsync().ok()?.get().ok()
}

fn is_own(s: &Session) -> bool {
    let id = s
        .SourceAppUserModelId()
        .map(|h| h.to_string())
        .unwrap_or_default()
        .to_lowercase();
    id.contains("wallplayer") || id.contains(crate::desktop::APP_ID) || id.contains("msedgewebview2")
}

fn is_playing(s: &Session) -> bool {
    s.GetPlaybackInfo()
        .and_then(|i| i.PlaybackStatus())
        .map(|st| st == Status::Playing)
        .unwrap_or(false)
}

/// Prefer: a playing foreign session → the system's current session → any foreign session.
fn pick(m: &Manager) -> Option<Session> {
    let mut fallback: Option<Session> = None;
    if let Ok(list) = m.GetSessions() {
        let n = list.Size().unwrap_or(0);
        for i in 0..n {
            if let Ok(s) = list.GetAt(i) {
                if is_own(&s) {
                    continue;
                }
                if is_playing(&s) {
                    return Some(s);
                }
                if fallback.is_none() {
                    fallback = Some(s);
                }
            }
        }
    }
    if let Ok(cur) = m.GetCurrentSession() {
        if !is_own(&cur) {
            return Some(cur);
        }
    }
    fallback
}

fn read(s: &Session) -> MediaState {
    let mut st = MediaState { has: true, ..Default::default() };
    st.app = s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default();
    if let Ok(p) = s.TryGetMediaPropertiesAsync().and_then(|op| op.get()) {
        st.title = p.Title().map(|h| h.to_string()).unwrap_or_default();
        st.artist = p.Artist().map(|h| h.to_string()).unwrap_or_default();
        st.album = p.AlbumTitle().map(|h| h.to_string()).unwrap_or_default();
    }
    st.playing = is_playing(s);
    if let Ok(tl) = s.GetTimelineProperties() {
        let pos = tl.Position().map(|t| t.Duration).unwrap_or(0);
        let start = tl.StartTime().map(|t| t.Duration).unwrap_or(0);
        let end = tl.EndTime().map(|t| t.Duration).unwrap_or(0);
        let upd = tl.LastUpdatedTime().map(|d| d.UniversalTime).unwrap_or(0);
        st.position_ms = (pos - start).max(0) / 10_000;
        st.duration_ms = (end - start).max(0) / 10_000;
        st.updated_ms = if upd > 0 { (upd - FILETIME_UNIX_DIFF) / 10_000 } else { 0 };
    }
    if st.title.is_empty() {
        st.has = false;
    }
    st
}

fn thumbnail(s: &Session) -> Option<String> {
    let p = s.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
    let r = p.Thumbnail().ok()?;
    let stream = r.OpenReadAsync().ok()?.get().ok()?;
    let size = stream.Size().ok()? as u32;
    if size == 0 || size > 20_000_000 {
        return None;
    }
    let input = stream.GetInputStreamAt(0).ok()?;
    let reader = DataReader::CreateDataReader(&input).ok()?;
    reader.LoadAsync(size).ok()?.get().ok()?;
    let mut buf = vec![0u8; size as usize];
    reader.ReadBytes(&mut buf).ok()?;
    let mime = if buf.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if buf.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else {
        "image/png"
    };
    Some(format!(
        "data:{};base64,{}",
        mime,
        base64::engine::general_purpose::STANDARD.encode(&buf)
    ))
}

/// Background poller: emits `media` (MediaState) and `media-art` (data URL).
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        init_thread();
        let mgr = loop {
            if let Some(m) = manager() {
                break m;
            }
            std::thread::sleep(Duration::from_secs(3));
        };
        let mut last = MediaState::default();
        let mut last_key = String::new();
        loop {
            let st = match pick(&mgr) {
                Some(s) => {
                    let st = read(&s);
                    let key = format!("{}\u{1}{}\u{1}{}", st.title, st.artist, st.album);
                    if st.has && key != last_key {
                        last_key = key;
                        let art = thumbnail(&s);
                        let _ = app.emit("media-art", art);
                    }
                    st
                }
                None => MediaState::default(),
            };
            if st != last {
                let _ = app.emit("media", &st);
                last = st;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });
}

/// Control the current foreign session. Runs on its own MTA thread.
pub fn control(action: String, value: Option<f64>) -> Result<(), String> {
    std::thread::spawn(move || -> Result<(), String> {
        init_thread();
        let mgr = manager().ok_or("media manager unavailable")?;
        let s = pick(&mgr).ok_or("nothing is playing")?;
        let r = match action.as_str() {
            "toggle" => s.TryTogglePlayPauseAsync().and_then(|o| o.get()),
            "play" => s.TryPlayAsync().and_then(|o| o.get()),
            "pause" => s.TryPauseAsync().and_then(|o| o.get()),
            "next" => s.TrySkipNextAsync().and_then(|o| o.get()),
            "prev" => s.TrySkipPreviousAsync().and_then(|o| o.get()),
            "seek" => {
                let secs = value.unwrap_or(0.0).max(0.0);
                let start = s
                    .GetTimelineProperties()
                    .and_then(|t| t.StartTime())
                    .map(|t| t.Duration)
                    .unwrap_or(0);
                s.TryChangePlaybackPositionAsync(start + (secs * 10_000_000.0) as i64)
                    .and_then(|o| o.get())
            }
            "shuffle" => {
                let cur = s
                    .GetPlaybackInfo()
                    .and_then(|i| i.IsShuffleActive())
                    .and_then(|v| v.Value())
                    .unwrap_or(false);
                s.TryChangeShuffleActiveAsync(!cur).and_then(|o| o.get())
            }
            _ => return Err(format!("unknown action {action}")),
        };
        r.map(|_| ()).map_err(|e| e.to_string())
    })
    .join()
    .map_err(|_| "media thread panicked".to_string())?
}
