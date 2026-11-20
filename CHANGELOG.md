# Changelog

Each section is the release notes of one version. The release workflow publishes
the section that matches the version in `src-tauri/Cargo.toml`.

## 1.0.3

### Mới
- **Tự cập nhật, không cần file cài.** Mỗi lần mở, Wallplayer kiểm tra ngầm một lần; có bản mới thì hiện nhãn nhỏ với *Update* và *Skip*. Chuột phải biểu tượng khay → *Check for updates* để xem release notes và tải bản mới. Tắt kiểm tra lúc mở trong Appearance → Updates.
- **Nhẹ máy hơn.** Dừng nhạc là viền sáng và nền chậm dần rồi thôi vẽ. Có game, phim toàn màn hình hoặc cửa sổ phóng to che kín màn hình thì Wallplayer nghỉ và tạm dừng video nền. Chạy pin hoặc bật Energy Saver thì tự nhẹ đi (30 khung hình/giây, video tối đa 720p). Chọn Auto, Full hoặc Saver trong Appearance → Performance.
- **Tắt tìm lời bài hát trên mạng** trong Appearance → Lyrics → *Find lyrics online*.

### Thay đổi
- Wallplayer giờ là mã nguồn mở, giấy phép MIT.
- Font chữ đi kèm app, mở app không cần tải font từ Google nữa.
- Phân tích âm thanh bỏ qua những đoạn im lặng.

## 1.0.2 and earlier

See [Releases](https://github.com/lehuynhtan17/Wallplayer/releases).
