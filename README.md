# Opal (bản Tauri)

**Opal** là bộ widget kính mờ nằm ngay trên hình nền Windows: thanh ứng dụng, đồng hồ, ghi chú, lịch, nhạc, thời tiết, âm lịch, đếm ngược… Mỗi widget là một ô kính trong suốt ăn theo màu hình nền, viền ánh nhiều màu như đá opal.

Opal viết bằng Rust. Bản này dùng [Tauri 2](https://tauri.app): phần lõi là Rust, giao diện là HTML/CSS chạy trong WebView2 có sẵn của Windows. Cả app là **một file `opal-tauri.exe` khoảng 17 MB**, không cần cài thêm gì trên Windows 11.

Opal bắt đầu từ việc viết lại **Lumen 0.1.0** (giấy phép MIT) bằng Rust, rồi phát triển thêm theme, widget và tính năng riêng. Còn một bản dùng egui ở repo [`opal`](https://github.com/bducanh87/opal).

## Mục lục

- [Chạy Opal](#chạy-opal)
- [Làm quen trong 1 phút](#làm-quen-trong-1-phút)
- [Các widget](#các-widget)
- [Nghe YouTube Music](#nghe-youtube-music)
- [Cài đặt](#cài-đặt)
- [Dữ liệu lưu ở đâu](#dữ-liệu-lưu-ở-đâu)
- [Tham số dòng lệnh](#tham-số-dòng-lệnh)
- [Cấu trúc mã nguồn](#cấu-trúc-mã-nguồn)
- [Câu hỏi thường gặp](#câu-hỏi-thường-gặp)

## Chạy Opal

Cần có [Rust](https://rustup.rs) (bản stable) và Windows 10/11 có WebView2 (Windows 11 có sẵn).

```bash
cargo build --release
```

Build xong, chạy file `target\release\opal-tauri.exe`. Icon Opal (viên đá opal) xuất hiện ở khay hệ thống, các widget hiện trên hình nền.

Mỗi lúc chỉ chạy được một bản Opal. Nếu bản egui đang chạy, bản Tauri sẽ báo và tự thoát (và ngược lại).

## Làm quen trong 1 phút

| Muốn làm gì | Cách làm |
|---|---|
| Xem các widget | Ra màn hình desktop (`Win + D`). Widget luôn nằm sát hình nền, dưới các cửa sổ khác. |
| Dùng một widget khi đang mở cửa sổ khác | Bấm vào phần widget đang lộ ra: nó nổi lên trên. Để yên 10 giây thì nó tự chìm xuống lại. |
| Bật / tắt widget | Bấm icon Opal ở khay hệ thống để mở bảng **Widgets**, gạt công tắc từng widget. Có nút **Show all** / **Hide all**. |
| Di chuyển widget | Nhấn giữ vào vùng trống của widget rồi kéo. |
| Đổi kích thước | Kéo góc dưới bên phải của widget. |
| Cài đặt riêng một widget | Rê chuột lên widget, bấm bánh răng ở góc trên phải. Với App Dock, bánh răng nằm ở cuối ô tìm kiếm. |
| Ẩn một widget | Rê chuột lên widget, bấm dấu × cạnh bánh răng. |
| Thoát Opal | Chuột phải icon ở khay hệ thống → **Quit Opal**. |

Vị trí, kích thước và trạng thái bật tắt của widget được lưu lại, lần sau mở Opal vẫn như cũ.

## Các widget

| Widget | Dùng để làm gì |
|---|---|
| **App Dock** | Lưới ứng dụng ghim sẵn và ô tìm kiếm. Gõ để tìm mọi app trong Start Menu và Microsoft Store, `Enter` để mở, bấm ghim để thêm vào lưới. Kéo icon để đổi thứ tự. Thả file hoặc thư mục vào để thêm. Chuột phải icon: mở, hiện trong thư mục, đổi tên, xoá. Ô **Add** thêm file, thư mục, This PC, Recycle Bin. |
| **Icon gọi nhanh** | Nằm ở cuối ô tìm kiếm của App Dock. Bấm icon widget để bật hoặc tắt widget đó (đang mở thì có gạch màu bên dưới). Có thể thêm Chụp màn hình, Task Manager, File Explorer, Windows Settings. Chọn icon trong cài đặt App Dock, mục **Gọi nhanh**. |
| **Clock** | 9 mặt đồng hồ: Opal, Afterglow, Paper, Nightfall, Colour block, Split flap, Daybook, Timeline, Terminal. Chọn 12/24 giờ, hiện giây, múi giờ. |
| **Sticky notes** | Ghi chú nhiều màu, ghim lên đầu, tự lưu khi gõ. 3 kiểu: Pinboard, Notebook, Index. |
| **Calendar** | Lịch tháng, 6 kiểu. Dán link **ICS** của Google Calendar hoặc Outlook để hiện sự kiện, kể cả sự kiện lặp lại. |
| **Music** | Điều khiển nhạc đang phát: phát/dừng, chuyển bài, tua, ảnh bìa. Kiểu **Coverflow** hiện bìa các bài trong hàng đợi và danh sách phát khi dùng với YouTube Music của Opal. Có hiệu ứng nhảy theo nhạc. |
| **Weather** | Nhiệt độ, trạng thái trời, cao/thấp, độ ẩm, gió, dự báo theo giờ hoặc 6 ngày. Không cần khoá API. Để trống thành phố thì tự định vị theo mạng. |
| **System** | CPU (kèm biểu đồ), RAM, ổ đĩa, tốc độ mạng, thời gian máy đã chạy. 3 kiểu: Opal, Rings, Terminal. |
| **Focus timer** | Đồng hồ Pomodoro: Focus, nghỉ ngắn, nghỉ dài sau mỗi 4 phiên, có tiếng báo khi hết giờ. |
| **Photos** | Trình chiếu ảnh trong một thư mục. Bấm đúp để chuyển ảnh. 3 kiểu: Opal, Polaroid, Film. |
| **Âm lịch** | Ngày dương và ngày âm, can chi ngày/tháng/năm, tiết khí, giờ hoàng đạo. |
| **To-do** | Danh sách việc cần làm: thêm, đánh dấu xong, xoá, dọn việc đã xong. |
| **Đếm ngược** | Tự đếm ngày tới Tết, Rằm và Mùng 1 sắp tới. Bấm **+** để thêm ngày riêng theo **âm lịch hoặc dương lịch** (giỗ, sinh nhật…), lặp lại hằng năm. 3 kiểu: Opal, Hero, Tickets. |

## Nghe YouTube Music

Widget Music có hai nguồn nhạc:

1. **Mọi trình phát trên máy** (Spotify, trình duyệt, Media Player…): widget hiện bài đang phát và điều khiển được, nhưng không biết danh sách phát.
2. **Cửa sổ YouTube Music của chính Opal**: widget biết cả bài đang phát lẫn danh sách "Tiếp theo".

Cách dùng nguồn thứ hai:

1. Trên widget Music, bấm nút **↗** (hoặc ô "Open your player"). Cũng có thể chuột phải icon khay → **YouTube Music**.
2. Đăng nhập YouTube Music trong cửa sổ đó (chỉ cần một lần) và phát nhạc.
3. Trên widget: bìa bài đang phát ở giữa, bài trước và bài sau ở hai bên. Bấm nút danh sách để mở **PLAYLIST**; bấm vào bài nào là phát bài đó.
4. Bấm **X** trên cửa sổ YouTube Music chỉ ẩn cửa sổ, **nhạc vẫn phát**. Muốn tắt hẳn: chuột phải icon khay → **Stop YouTube Music**.

**Hiệu ứng theo nhạc:** sóng nhạc trong ô bài đang phát nhảy theo các dải tần, bìa giữa nhún theo tiếng bass, quầng sáng sau bìa sáng theo độ lớn. Opal nghe âm thanh đang phát ra loa (không dùng micro), nên hiệu ứng chạy với mọi nguồn nhạc. Việc này chỉ chạy khi widget Music đang hiện trên màn hình. Tắt trong cài đặt Music → **Hiệu ứng theo nhạc**.

## Cài đặt

Mỗi widget có bảng cài đặt riêng (bấm bánh răng), gồm:

- **Design**: kiểu hiển thị của widget đó.
- **Theme**: màu kính. Chọn riêng cho widget, hoặc **Global** để dùng theme chung.
- **Tuỳ chọn riêng** của widget (thành phố, thư mục ảnh, thời lượng Pomodoro…).
- **Glass**: bật/tắt kính mờ, nhìn xuyên hình nền, độ mờ (Frost), độ nhoè (Blur), độ phủ màu (Tint), độ trong (Opacity).

Cài đặt chung nằm trong bảng của **App Dock**:

| Mục | Ý nghĩa |
|---|---|
| Theme (all widgets) | Theme chung. **Opal** là mặc định: kính ánh nhiều màu, viền đổi màu chạy chậm. |
| Columns, Icon size, Search box, Labels | Bố cục App Dock. |
| Start with Windows | Tự chạy Opal khi mở máy. |
| Performance mode | Giảm độ nhoè và dừng các hiệu ứng chuyển động, cho máy yếu. |
| Bảng nổi lên trên | Bảng Widgets và bảng cài đặt luôn nằm trên các cửa sổ khác. |
| Chìm xuống nền sau (giây) | Sau khi dùng, widget ở trên bao lâu rồi mới chìm xuống hình nền (0–30 giây, mặc định 10). |
| Gọi nhanh | Chọn các icon hiện ở cuối ô tìm kiếm. |
| Glass (all widgets) | Thông số kính dùng chung cho mọi widget. |

## Dữ liệu lưu ở đâu

Opal lưu mọi thứ trong thư mục `config\` nằm cạnh file exe (kiểu portable). Nếu thư mục đó không ghi được, Opal dùng `%APPDATA%\Opal`.

| File | Nội dung |
|---|---|
| `state.json` | Ứng dụng ghim trong App Dock và toàn bộ cài đặt. |
| `notes.json` | Ghi chú. |
| `calendar.json` | Link ICS và cài đặt lịch. |
| `opal-extras.json` | Việc cần làm, ngày đếm ngược, cài đặt thời tiết, ảnh, Pomodoro… |
| `opal-tauri.json` | Vị trí, kích thước, trạng thái bật tắt của từng widget. |
| `opal-tauri.log` | Nhật ký: widget bật/tắt lúc nào và vì sao. |

Muốn mang Opal sang máy khác, chép file exe cùng thư mục `config`. Mở nhanh thư mục này: bảng Widgets → **Open config folder**.

Đặt biến môi trường `OPAL_CONFIG_DIR` nếu muốn dùng thư mục cấu hình khác.

## Tham số dòng lệnh

| Tham số | Tác dụng |
|---|---|
| `--widgets` | Mở bảng Widgets khi khởi động. |
| `--settings` | Mở cài đặt chung khi khởi động. |
| `--widget-settings <id>` | Mở cài đặt của một widget: `launcher`, `clock`, `notes`, `calendar`, `music`, `weather`, `system`, `pomodoro`, `photos`, `lunar`, `todo`, `countdown`. |
| `--youtube-music` | Mở cửa sổ YouTube Music khi khởi động. |

## Cấu trúc mã nguồn

```
src/
  main.rs          khởi động, danh sách widget, đăng ký lệnh
  windows_mgr.rs   cửa sổ widget và bảng, khay hệ thống, nổi lên / chìm xuống
  commands.rs      các lệnh giao diện gọi vào Rust
  ytm.rs           cửa sổ YouTube Music: đọc bài + hàng đợi, điều khiển phát
  audio.rs         thu âm loopback (WASAPI) và tách dải tần cho hiệu ứng nhạc
  media.rs         bài đang phát từ Windows (media_core.rs)
  config.rs        đọc/ghi các file cấu hình
  lunar.rs         âm lịch Việt Nam (thuật toán Hồ Ngọc Đức, múi giờ +7)
  ics.rs           đọc lịch ICS, sự kiện lặp
  weather_api.rs   dự báo từ Open-Meteo
  sysmon.rs        CPU, RAM, ổ đĩa, mạng
  programs.rs      danh mục ứng dụng và tìm kiếm
  theme.rs         theme và các kiểu thiết kế
  win.rs           Win32: ghim xuống desktop, icon, hình nền, tự chạy
ui/
  index.html, app.js, style.css   toàn bộ giao diện (không dùng framework)
capabilities/      quyền của từng loại cửa sổ
icons/             icon và script vẽ icon (make-icon.ps1)
```

Chạy test:

```bash
cargo test --release
```

**Về an toàn:** trang YouTube Music là trang bên ngoài, nên nó chỉ được gọi đúng một lệnh của app (báo bài đang phát). Mọi lệnh khác (mở file, chạy chương trình, đổi cài đặt…) chỉ giao diện của chính Opal gọi được. Quy định này nằm ở `build.rs` và `capabilities/`.

## Câu hỏi thường gặp

**Mở Opal mà không thấy widget đâu?**
Widget nằm dưới các cửa sổ khác. Nhấn `Win + D` để ra desktop. Nếu vẫn không thấy, bấm icon Opal ở khay hệ thống và gạt công tắc trong bảng Widgets.

**Widget Music trống hoặc không có danh sách?**
Danh sách chỉ có khi phát bằng cửa sổ YouTube Music của Opal (nút ↗). Nhạc từ nguồn khác chỉ hiện bài đang phát.

**Sóng nhạc không nhảy?**
Hiệu ứng chỉ chạy khi widget Music đang hiện trên màn hình và có âm thanh phát ra loa. Xem `config\opal-tauri.log` nếu có lỗi thu âm.

**YouTube Music đổi giao diện thì sao?**
Phần đọc bài và danh sách dựa vào cấu trúc trang YouTube Music. Nếu YouTube thay đổi, cần chỉnh lại đoạn script trong `src/ytm.rs`.

**Thời tiết, lịch ICS cần mạng không?**
Có. Thời tiết lấy từ Open-Meteo (định vị theo mạng nếu để trống thành phố), lịch tải từ link ICS bạn dán vào. Các widget khác chạy không cần mạng.

## Nguồn gốc và giấy phép

Opal được viết lại và phát triển từ **Lumen 0.1.0**, một ứng dụng phát hành theo giấy phép MIT. Thuật toán âm lịch theo công bố của Hồ Ngọc Đức.

Mã nguồn Opal phát hành theo giấy phép MIT, xem file [LICENSE](LICENSE). File đó cũng ghi rõ phần kế thừa từ Lumen.
