# Wallplayer — desktop wallpaper music player (Rust + Tauri v2)

A Windows desktop skin that turns the wallpaper into a music player: big hollow clock,
"Continue Playing" queue, glass now-playing card, multi-platform search, synced karaoke
lyrics, music-video wallpaper and an ambient edge light driven by album colours and
system loudness.

## Architecture

| Layer | Where | What |
|---|---|---|
| Window placement | `src-tauri/src/desktop.rs` | Borderless full-screen window pinned to the bottom of the z-order (subclassed `WM_WINDOWPOSCHANGING`), or re-parented into the shell `WorkerW` behind the icons (classic + Win11 24H2 layouts). Peek mode = top-most for the hotkey search. |
| Now Playing | `src-tauri/src/media.rs` | `GlobalSystemMediaTransportControlsSessionManager` (WinRT): title/artist/album/thumbnail/timeline of Spotify, Apple Music, browsers…; play/pause/next/prev/seek/shuffle. Emits `media`, `media-art`. |
| Ambient spectrum | `src-tauri/src/audio.rs` | WASAPI loopback via `cpal` + 4096-pt FFT (`rustfft`) → `audio-spectrum` [rms, 32 log bands] ≈60 Hz. UI does kick/snare onset detection and drives a WebGL neon-edge shader coloured by the album accent. |
| Accounts | `src-tauri/src/accounts.rs` | YouTube / SoundCloud sign-in windows sharing the WebView2 profile (embedded players run signed-in → Premium/Go+); Spotify OAuth PKCE (user's Client ID) for search + Spotify Connect playback. |
| Network | `src-tauri/src/net.rs` | Per-platform search: YouTube Music (InnerTube WEB_REMIX), YouTube (InnerTube WEB), SoundCloud (api-v2), Apple (iTunes Search); MV ids, LRCLIB synced lyrics, image → data URL. |
| Shell | `src-tauri/src/main.rs` | Tray menu, global hotkey Ctrl+Alt+Space, settings, autostart (HKCU Run). |
| UI | `ui/index.html` | Single-file UI. Runs standalone in a browser in demo mode (generated covers, simulated playback). |

Platform behaviour: YouTube Music/Automatic play in-app through the YouTube IFrame player
(video doubles as wallpaper); Spotify opens `spotify:search:`; Apple Music, Spotify Web
Player, SoundCloud and Amazon Music open their search/track page. Whatever plays is picked
up again through Windows media sessions.

## Build

On Windows (MSVC toolchain, recommended):

```
cd src-tauri
cargo build --release
```

Output: `src-tauri/target/release/wallplayer.exe` (WebView2 loader is linked statically on MSVC).

Cross-compiling from Linux (what produced the shipped build):

```
# needs: mingw-w64, rust-std for x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
# ship wallplayer.exe together with target/x86_64-pc-windows-gnu/release/WebView2Loader.dll
```

Preview the UI without building: open `ui/index.html` in Chrome/Edge.
