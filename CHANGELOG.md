# Changelog

Each section is the release notes of one version. The release workflow publishes
the section that matches the version in `src-tauri/Cargo.toml`.

## 1.0.3

### New
- **Updates without an installer.** Wallplayer quietly checks for a new version once when it starts. When there is one, the tray menu shows *Update available*; click it to read the release notes and choose *Update* or *Skip*. Turn the check off in Appearance → Updates.
- **Lighter on your computer.** When the music stops, the edge light and background slow down and stop drawing. Behind a full-screen game or film, or a maximised window, Wallplayer rests and pauses the background video. On battery or with Energy Saver it gets lighter by itself (30 fps, video up to 720p). Choose Auto, Full or Saver in Appearance → Performance.
- **Your own music.** The folder button on the playlist → *Add a folder…*, or drop folders and songs onto Wallplayer. Titles, artists and covers are read from the files; lyrics and the edge light work as usual. Folders are remembered.
- **Vietnamese interface.** Choose English or Tiếng Việt at the top of Appearance; the tray menu follows. Wallplayer uses Vietnamese when Windows does.
- **Playlist:** point at a song to play it next or remove it; drag the ⋮⋮ handle to reorder.
- **Lyrics:** timing for this song or for every song in the Lyrics panel, remembered; type to search for other lyrics; long instrumental parts show a ♪ with dots counting down to the next line.
- **Edge light:** 6 new effects (Fire, Lava, Snake, Sparkle, Neon, Wave); light the whole frame, the bottom, the top or only the corners; turn it off on any display; point at an effect to preview it.
- **Tray icon** shows the cover of the song playing, with a ring that beats (Appearance → Layout).
- **Pick the video:** dots above the search field choose the song's own music video or another one by the same artist.
- **Music in two places** (Wallplayer and Spotify, for example): Wallplayer asks which one to keep and pauses the other.
- **Turn off lyrics lookups** in Appearance → Lyrics → *Find lyrics online*.

### Changed
- Wallplayer is now open source under the MIT license.
- Fonts come with the app: it no longer loads them from Google at start.
- Dropping a picture or video onto Wallplayer to make it the wallpaper works on Windows.
- The sound analysis skips silence.
- English user guide (`GUIDE.txt`) next to the Vietnamese one.

## 1.0.2 and earlier

See [Releases](https://github.com/lehuynhtan17/Wallplayer/releases).
