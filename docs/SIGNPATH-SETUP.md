# Ký số Wallplayer bằng SignPath (miễn phí cho mã nguồn mở)

Phần kỹ thuật đã có sẵn trong repo: giấy phép MIT (`LICENSE`), chính sách ký số
(`CODE_SIGNING.md`), quyền riêng tư (`PRIVACY.md`), cấu hình ký
(`.signpath/artifact-configuration.xml`) và workflow GitHub Actions
(`.github/workflows/build.yml`). Bạn chỉ cần làm các bước dưới đây.

## Bước 1. Bật xác thực 2 bước cho GitHub (bắt buộc)

GitHub → ảnh đại diện → **Settings → Password and authentication → Two-factor
authentication → Enable**. Dùng app như Google Authenticator.

## Bước 2. Nộp đơn

Vào <https://signpath.org/apply> và điền. Nội dung gợi ý (dán nguyên):

| Mục | Điền |
|---|---|
| Project name | Wallplayer |
| Repository | https://github.com/lehuynhtan17/Wallplayer |
| Homepage / download page | https://github.com/lehuynhtan17/Wallplayer |
| License | MIT |
| Code signing policy | https://github.com/lehuynhtan17/Wallplayer/blob/main/CODE_SIGNING.md |
| Privacy policy | https://github.com/lehuynhtan17/Wallplayer/blob/main/PRIVACY.md |
| Build system | GitHub Actions on GitHub-hosted runners (`.github/workflows/build.yml`) |
| What is signed | `Wallplayer.exe` (portable Windows app, inside `Wallplayer-Windows-x64.zip`) |
| Releases | https://github.com/lehuynhtan17/Wallplayer/releases |

Mô tả dự án (tiếng Anh, dán vào ô description):

> Wallplayer is an open source (MIT) Windows app that turns the desktop wallpaper into
> a music player: a tall clock, a glass cover flow, synced lyrics with translated
> subtitles and an edge light that follows the music. It plays YouTube Music and
> SoundCloud through their official embedded players and follows Spotify and other
> apps through Windows media sessions. It is built with Tauri 2 and Rust, from this
> repository only, by GitHub Actions. It is a portable app (no installer) that can
> update itself from this repository's GitHub releases after the user agrees. It has
> no telemetry. The team is one maintainer, who reviews every outside contribution
> and approves every signing request.

Lưu ý khi họ hỏi:
- **Thư viện bên thứ ba:** chỉ có các crate Rust mã nguồn mở (MIT / Apache-2.0), font
  chữ giấy phép SIL OFL, và Microsoft WebView2 là thành phần của Windows. Không ký file
  nào ngoài `Wallplayer.exe`.
- **Tự cập nhật:** chỉ tải từ GitHub Releases của chính repo này, và chỉ khi người dùng
  bấm Update.

## Bước 3. Sau khi được duyệt (SignPath gửi email)

Trong trang quản lý SignPath (app.signpath.io), thường họ đã tạo sẵn tổ chức cho bạn:

1. **Project**: tạo project tên `Wallplayer`, slug `wallplayer`. Gắn repository GitHub ở trên.
2. **Trusted build system**: chọn GitHub.com, liên kết với project.
3. **Artifact configuration**: tạo mới, slug `initial`, dán nội dung file
   `.signpath/artifact-configuration.xml` trong repo.
4. **Signing policy**: slug `release-signing`, chứng chỉ do SignPath Foundation cấp,
   **bật yêu cầu duyệt (approval)**, người duyệt là bạn.
5. **API token**: tạo người dùng CI (CI user) có quyền submit vào policy trên, tạo
   token, chép lại.

## Bước 4. Dán thông tin vào GitHub

Repo trên GitHub → **Settings → Secrets and variables → Actions**:

- Tab **Secrets** → *New repository secret*:
  - `SIGNPATH_API_TOKEN` = token ở bước 3.5
- Tab **Variables** → *New repository variable*:
  - `SIGNPATH_ORGANIZATION_ID` = mã tổ chức (chuỗi trong địa chỉ `app.signpath.io/Web/<mã này>/...`)
  - `SIGNPATH_PROJECT_SLUG` = `wallplayer`
  - `SIGNPATH_POLICY_SLUG` = `release-signing`
  - `SIGNPATH_ARTIFACT_CONFIGURATION_SLUG` = `initial`

Xong bước này thì mọi bản phát hành sau đều được ký.

## Mỗi lần phát hành

1. (Claude làm) Cập nhật `CHANGELOG.md` và số phiên bản, đẩy lên GitHub. GitHub Actions
   tự build; tải bản build trong trang **Actions** để thử trên Windows.
2. Thử xong: **Actions → Build and release → Run workflow** (để tick *Publish a release*) → **Run workflow**.
3. SignPath gửi email yêu cầu ký: bấm **Approve** trong vòng 4 giờ.
4. Workflow tự tạo release `Vx.y.z` có file zip đã ký và release notes. Người dùng
   bản cũ sẽ thấy nhãn cập nhật vào lần mở app tiếp theo.

Chưa xong bước 3–4 thì workflow vẫn phát hành được, chỉ là file chưa ký (có cảnh báo
vàng trong log).
