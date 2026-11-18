# Wallplayer — ghi chú cho Claude Code

Chủ dự án: Tấn (Senior UX/UI Product Designer, không biết code). **Luôn trả lời bằng tiếng Việt**, giải thích đơn giản, tự làm hết các bước (build, commit, push, release), chỉ hỏi lại khi thật sự cần.

## Dự án
App Windows (Tauri v2 + Rust) biến hình nền desktop thành trình phát nhạc kiểu "aesthetic desktop skin" (đồng hồ cao, cover flow kính, search nhiều nền tảng, playlist 3D, lyrics đồng bộ + phụ đề 2 ngôn ngữ, ambient light viền neon nhảy theo nhạc, chỉnh đồng hồ/lyrics kiểu lock screen iPhone với chữ kính).

## Cấu trúc
- `ui/index.html`: toàn bộ giao diện (1 file HTML/CSS/JS, chạy trong WebView2).
- `src-tauri/src/`: Rust. `desktop.rs` (WorkerW, đa màn hình, hiện taskbar), `media.rs` (Windows Media), `audio.rs` (loopback + FFT), `net.rs` (search, lyrics, dịch), `accounts.rs` (đăng nhập YouTube/SoundCloud/Spotify), `volume.rs` (âm lượng Windows), `settings.rs`, `main.rs`.
- `HUONG-DAN.txt`: hướng dẫn người dùng (tiếng Việt, giữ cập nhật khi thêm tính năng).

## Build (trên Windows)
```
cd src-tauri
cargo build --release
```
File ra: `src-tauri/target/release/wallplayer.exe`. Khi phát hành phải đi kèm `WebView2Loader.dll` cùng thư mục. Cần Rust (MSVC) và Microsoft Edge WebView2 Runtime.

## Phát hành
Đóng gói thư mục `Wallplayer/` gồm `Wallplayer.exe`, `WebView2Loader.dll`, `HUONG-DAN.txt` (CRLF) thành `Wallplayer-Windows-x64.zip`, rồi tạo GitHub Release (`gh release create vX.Y.Z <zip>`).

## Lưu ý
- Bản exe cũ được build chéo từ Linux và chưa từng chạy thử trên Windows thật: hãy build và kiểm tra lại trên máy Windows trước khi phát hành.
- Genius, YouTube InnerTube, dịch Google dùng API không chính thức, có thể đổi bất cứ lúc nào.
- Google lyrics không tự động hóa được (captcha), app chỉ mở trình duyệt.
