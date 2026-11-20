# Changelog

Each section is the release notes of one version. The release workflow publishes
the section that matches the version in `src-tauri/Cargo.toml`.

## 1.0.3

### Mới
- **Tự cập nhật, không cần file cài.** Mỗi lần mở, Wallplayer kiểm tra ngầm một lần. Có bản mới thì menu chuột phải biểu tượng khay hiện *Update available*; bấm vào để xem release notes và chọn *Update* hoặc *Skip*. Tắt kiểm tra lúc mở trong Appearance → Updates.
- **Nhẹ máy hơn.** Dừng nhạc là viền sáng và nền chậm dần rồi thôi vẽ. Có game, phim toàn màn hình hoặc cửa sổ phóng to che kín màn hình thì Wallplayer nghỉ và tạm dừng video nền. Chạy pin hoặc bật Energy Saver thì tự nhẹ đi (30 khung hình/giây, video tối đa 720p). Chọn Auto, Full hoặc Saver trong Appearance → Performance.
- **Tắt tìm lời bài hát trên mạng** trong Appearance → Lyrics → *Find lyrics online*.
- **Giao diện tiếng Việt.** Chọn English hoặc Tiếng Việt ở đầu Appearance; menu icon khay đổi theo. Máy để tiếng Việt thì app tự dùng tiếng Việt.
- **Nhạc trên máy.** Nút thư mục ở playlist bên trái → *Add a folder…*, hoặc kéo thả thư mục / bài hát vào Wallplayer. Đọc tên bài, ca sĩ, ảnh bìa trong file; lời bài hát và viền sáng chạy như thường. Thư mục được nhớ cho lần sau.
- **Playlist:** rê chuột lên một bài để *Phát tiếp theo* hoặc xóa; kéo tay cầm ⋮⋮ để sắp xếp.
- **Lời bài hát:** chỉnh giờ cho bài này hoặc mọi bài ngay trong bảng Lyrics, Wallplayer ghi nhớ; tự gõ để tìm lời; đoạn nhạc dạo dài hiện nốt ♪ và ba chấm đếm ngược tới câu tiếp theo.
- **Viền sáng:** thêm 6 hiệu ứng (Lửa, Dung nham, Rắn, Lấp lánh, Neon, Sóng); chỉ sáng cả viền, cạnh dưới, cạnh trên hoặc các góc; bật tắt riêng từng màn hình; rê chuột lên tên hiệu ứng để xem trước.
- **Icon khay** thành ảnh bìa bài đang phát, có vòng tròn đập theo nhạc (tắt được trong Appearance → Layout).
- **Chọn MV:** các chấm phía trên trình phát để chọn MV của bài hoặc MV khác của cùng ca sĩ.
- **Nhạc phát ở hai nơi** (ví dụ Wallplayer và Spotify): Wallplayer hỏi bạn giữ bên nào và dừng bên kia.

### Thay đổi
- Wallplayer giờ là mã nguồn mở, giấy phép MIT.
- Font chữ đi kèm app, mở app không cần tải font từ Google nữa.
- Kéo thả ảnh / video vào app để làm hình nền chạy được trên Windows.
- Phân tích âm thanh bỏ qua những đoạn im lặng.

## 1.0.2 and earlier

See [Releases](https://github.com/lehuynhtan17/Wallplayer/releases).
