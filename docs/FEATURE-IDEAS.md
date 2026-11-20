# Wallplayer — Tính năng cân nhắc (research thị trường)

Research ngày 20/11/2026: các app cùng loại trên Windows/Mac (Wallpaper Engine, Lively Wallpaper, Rainmeter, Sleeve 2, Lyrimuse, Lyricify, FluentFlyout, Notchify, WinIsland, Hue Sync, Govee Desktop, các app lofi + Pomodoro).

Ký hiệu: ✅ đã làm · ⏸️ làm xong, tạm ẩn · 🔨 đang làm · ⭐ nên làm sớm · 💡 cân nhắc · ⛔ chưa nên làm

## Nên làm sớm

| | Tính năng | Ai đang có | Vì sao hợp với Wallplayer | Công sức |
|---|---|---|---|---|
| ✅ | **Lời bài hát chạy theo từng chữ** (kiểu Apple Music) | Apple Music, Lyrimuse, Lyricify, LyricGlow | Nguồn NetEase có sẵn định dạng YRC (thời điểm từng chữ). Hợp với các hiệu ứng lyrics đang có. **Đã làm (1.0.3)**: mọi hiệu ứng + hiệu ứng Apple Music | Vừa |
| ⏸️ | **Dynamic Island**: đảo nhỏ trên cùng màn hình khi cửa sổ che hình nền hoặc khi bấm thu gọn | Notchify, WinIsland, Isle Notch, FluentFlyout | Wallplayer nằm sau các cửa sổ, đảo nhỏ bù đúng chỗ thiếu. Có thể hiện luôn dòng lyrics. **Đã làm xong nhưng đang tạm ẩn** (bật lại: `ISLAND_ENABLED = true` trong `ui/index.html`) | Vừa |
| ⭐ | **Hẹn giờ tập trung / hẹn giờ tắt nhạc** trên chiếc đồng hồ lớn | LoFi Pomodoro, Lofikofi, Cozy Pomodoro | Đã có đồng hồ to và nhạc; chỉ cần đếm ngược + vòng tiến trình. Rất hợp chất aesthetic | Nhỏ |
| ⭐ | **Điều khiển bằng điện thoại qua mã QR** (không cần cài app) | Simple Remote Control, LocalTV Remote, WeRV | Mở trang điều khiển trên trình duyệt điện thoại cùng Wi-Fi: play/pause, chuyển bài, âm lượng, xem lyrics | Vừa |
| ⭐ | **Hiện bài đang nghe trên Discord** | Music Discord Rich Presence, AMWin-RP, Navicord | Wallplayer biết bài đang phát từ mọi nguồn (Spotify, YouTube, nhạc máy) | Nhỏ |
| ⭐ | **Ghi lịch sử nghe lên Last.fm** | Sleeve 2, ZuseMe, foo_scrobbler | Người nghe nhạc kỹ tính rất chuộng; đi chung với Discord | Nhỏ |

## Cân nhắc (cần thiết bị để thử)

| | Tính năng | Ai đang có | Ghi chú |
|---|---|---|---|
| 💡 | **Đèn trong phòng đổi màu theo nhạc** | Philips Hue Sync, Govee Desktop (Music DreamView), LumiSync | Govee có API điều khiển trong mạng nội bộ (UDP), dễ làm. Dùng màu bìa album + nhịp nhạc sẵn có |
| 💡 | **Lưu / chia sẻ theme** (xuất – nhập file, vài theme mẫu) | Sleeve 2 (theme đồng bộ iCloud), Rainmeter | Theme = màu, font đồng hồ, kiểu lyrics, hiệu ứng viền |
| 💡 | **Phím tắt toàn cục cho play/pause, chuyển bài, like** | Sleeve 2 | Đã có phím mở search; mở rộng thêm |
| 💡 | **Hình nền Shadertoy** (dán link shader làm nền) | Lively Wallpaper | Đã có WebGL cho viền sáng |
| 💡 | **Phòng 3D nhỏ có đèn nháy theo bass** | Lively "Living Room" | Một kiểu nền mới, nặng hơn |
| 💡 | **Visualizer kiểu kính iOS** | Wallpaper Engine "Music Visualizer iOS Style" | Hợp với phong cách kính của app |
| 💡 | **Taskbar đổi màu theo hình nền** | Lively Wallpaper | Đụng tới cài đặt Windows, cần cẩn thận |
| 💡 | **Màu tắt / ẩn visualizer khi im lặng** | Wallpaper Engine ("Silent Color", "Hide when Silent") | Một phần đã có (ngừng vẽ khi im lặng) |

## Chưa nên làm

| | Tính năng | Lý do |
|---|---|---|
| ⛔ | Karaoke tắt giọng ca sĩ (như Apple Music Sing) | Cần tách giọng bằng học máy theo thời gian thực; Wallplayer không can thiệp được âm thanh của YouTube / Spotify |
| ⛔ | Video Canvas của Spotify | Không có API công khai; MV làm hình nền đã thay thế được |

## Nguồn

- Wallpaper Engine, Lively Wallpaper, Rainmeter skins (Monstercat Visualizer, Lano, PlayMusic)
- Sleeve 2 (replay.software/sleeve)
- Lyrimuse, Lyricify, LyricGlow, Lyric Overlay — lyrics từng chữ
- FluentFlyout, Notchify, WinIsland, Isle Notch — media flyout / Dynamic Island cho Windows
- Philips Hue Sync, Govee Desktop, LumiSync — đèn theo nhạc
- Music Discord Rich Presence, AMWin-RP, ZuseMe, foo_scrobbler — Discord / Last.fm
- Simple Remote Control, LocalTV Remote, WeRV — điều khiển bằng điện thoại
- LoFi Pomodoro, Lofikofi, Cozy Pomodoro — hẹn giờ tập trung kèm nhạc
- Lyricify docs (định dạng YRC của NetEase)
