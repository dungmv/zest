# Đặc tả Kỹ thuật: Nâng Cấp Giao Diện & Trải Nghiệm Người Dùng (UI / UX Enhancements)

## 1. Mô Hình Phiên Làm Việc Đơn Nhiệm & Điều Hướng Lịch Sử (Single-Active Request & History Navigation)

### 1.1 Cơ Chế Hoạt Động Của Trình Soạn Thảo (Request Composer Navigation):
Thay vì hệ thống multi-tabs truyền thống cồng kềnh dễ gây phân mảnh state và chiếm dụng bộ nhớ (God Object `EditorState`), Zest áp dụng mô hình Single-Active Request hiện đại tương tự Insomnia và Yaak:
1. **Direct Request Activation:** Nhấp chuột vào bất kỳ request nào ở Sidebar sẽ kích hoạt và mở thẳng request đó trong `RequestComposer`.
2. **Single Source of Truth:** `RequestSessionState` chỉ lưu con trỏ `active_request_id: Option<String>` đại diện cho request đang làm việc; toàn bộ dữ liệu request được mượn on-demand từ `WorkspaceDataState`.
3. **Điều Hướng Lịch Sử (History Navigation — Back / Forward):**
   - Hỗ trợ phím tắt điều hướng `NavigateBack` (`Alt+Cmd+Left` / `Cmd+[`) và `NavigateForward` (`Alt+Cmd+Right` / `Cmd+]`).
   - Tự động duy trì ngăn xếp lịch sử các request đã duyệt qua trong `SidebarState`, cho phép quay lại hoặc tiến tới nhanh chóng mà không cần thanh tab rườm rà.
4. **Dirty State & Tự Động Lưu (Debounced Auto-Save):**
   - Khi có thay đổi chưa lưu (sửa URL, params, headers, body, auth, scripts), cờ `is_dirty` được bật.
   - Cơ chế tự động lưu (debounced auto-save) qua `PersistenceState` sẽ đồng bộ dữ liệu request xuống file YAML trên đĩa một cách trơn tru, không làm gián đoạn luồng làm việc của người dùng.

---

## 2. Các Trình Kết Xuất Xem Trước (Preview Renderers)

Trong tab Response, bên cạnh chế độ xem `Pretty` (JSON thụt lề có màu) và `Raw`, bổ sung dropdown chuyển đổi:

### 2.1 Preview Hình Ảnh (Image Preview)
- Tự động nhận diện `Content-Type: image/png`, `image/jpeg`, `image/svg+xml`, `image/webp`, `image/gif`.
- Hiển thị ảnh trực quan, kích thước pixel (`width x height`), dung lượng file byte.
- Hỗ trợ phóng to, thu nhỏ (Zoom & Pan), và nút **"Save Image to File"**.

### 2.2 Sandboxed HTML Preview
- Tự động nhận diện `Content-Type: text/html`.
- Render mã HTML an toàn (tắt JavaScript độc hại hoặc chạy trong sandbox an toàn) để xem nhanh giao diện trang web hoặc thông báo lỗi của máy chủ web (Apache/Nginx/IIS 500 error page).

### 2.3 Hex Viewer
- Dành cho các response dạng nhị phân (Binary payloads, protobuf, file nén zip, file âm thanh).
- Hiển thị theo cột chuẩn: Offset hex bên trái, Byte hex ở giữa, ký tự ASCII bên phải.

---

## 3. Quản Lý Cookie Nâng Cao (Advanced Cookie Manager)

### 3.1 Nâng Cấp Modal Cookies:
- Phân nhóm cookie theo Domain (ví dụ: `.google.com`, `localhost`, `api.github.com`).
- Hiển thị đầy đủ các thuộc tính của cookie RFC 6265:
  - Name, Value
  - Domain, Path
  - Expires / Max-Age
  - Cờ bảo mật: `HttpOnly`, `Secure`, `SameSite (Strict, Lax, None)`.
- Cung cấp tính năng thêm cookie mới bằng tay, sửa giá trị, xóa từng cookie hoặc **"Clear All Cookies for Domain"**.

---

## 4. Tùy Biến Giao Diện & Themes (Customization & Polish)

1. **Bảng màu Tùy Chọn:**
   - **Dark Navy (Mặc định):** Deep navy `#1b1b2a` phong cách Yaak/Zed.
   - **OLED Pure Black:** `#000000` tiết kiệm pin trên màn hình OLED.
   - **Catppuccin Mocha / Macchiato:** Bộ màu được cộng đồng lập trình viên yêu thích nhất.
   - **Light Mode:** Dành cho người dùng thích giao diện sáng khi làm việc ban ngày.
2. **Cỡ Chữ & Font Family:**
   - Cho phép người dùng tùy chọn Font chữ Monospace (JetBrains Mono, Fira Code, Cascadia Code, SF Mono) và kích thước font size từ 11px đến 18px trong Settings.

---

## 5. Thanh Nhập Liệu Request Thông Minh (Unified Smart Request Bar)

1. **Tự Động Nhận Diện & Parse cURL Khi Dán (Auto-Detect cURL on Paste):**
   - Khi người dùng dán (Ctrl+V) câu lệnh cURL vào ô nhập URL, thanh input tự động phát hiện lệnh cURL, bóc tách method, URL, headers, auth và body.
   - Tránh việc phải mở modal import riêng biệt, mang lại trải nghiệm tiện dụng cao nhất.
2. **Tự Động Tách Query Parameters:**
   - Khi gõ hoặc dán URL có chứa `?param1=val1&param2=val2`, hệ thống tự động trích xuất các tham số này vào bảng `Params` và giữ URL gốc sạch sẽ.
3. **Inline Environment Variable Tokens & Autocomplete:**
   - Khi gõ `{{`, hiển thị popup gợi ý biến môi trường (kèm scope Global / Active Environment, giá trị hiện tại).
   - Render biến dưới dạng token màu trực quan (xanh lá cho biến hợp lệ, cam/đỏ cho biến chưa được định nghĩa).
   - Di chuột lên token để xem popover giá trị giải mã (native `Tooltip`); click vào token để mở nhanh modal cấu hình Environment.

---

## 6. Tính Năng UI Đã Ship Trong Cycle 2 (2026-10-27 → 2026-11-15)

Tổng hợp các nâng cấp giao diện đã hoàn tất và được chuẩn hóa qua migration `gpui-kit` (toàn bộ 24 nhóm component):

1. **Sidebar:**
   - Section **Pinned** trở thành folder thu gọn được (chevron disclosure, ẩn hiện khi không có pin, indentation cấp 1).
   - **Multi-select** Ctrl/Cmd-click cho request + folder; kéo-thả nhiều item cùng lúc (`DragMultiItem`); context menu khi multi-select chỉ còn "Delete".
   - Tạo mới request/folder đều kích hoạt **inline rename** ngay (text được select sẵn) và tự mở rộng folder cha đang thu gọn.
   - Context menu 100% chuẩn hóa `PopupMenuItem` kèm icon; menu New HTTP/WebSocket/gRPC.
2. **Command Palette (Ctrl/Cmd+K):** chỉ còn 2 nhóm Switch Request & Switch Environment; mỗi request hiển thị full path `Folder / Sub / Name`; hoạt động ổn định trên Windows kể cả khi input đang focus (intercept `root.on_key_down`).
3. **Titlebar:** breadcrumb phân cấp với Smart Head Truncation (`… › Folder › Request`), ngắt theo char-budget; tooltip đa nền tảng (⌘/Ctrl); hitbox Windows cho các nút dưới drag region.
4. **Modals:** toàn bộ 8 modal chuẩn `ModalDialog` semantics; fixed-height chống nhảy card khi chuyển tab (import 640×520, workspace settings 580×320); env modal tách module (`env_modal/`) với `EnvVarTable` đồng bộ thuần qua event (chữa triệt để bug typing race do watcher self-echo).
5. **Environment:** danh sách env sort alphabetically (case-insensitive) tại state layer; modal env không còn hiển thị trạng thái active.
6. **Response Inspector:** spinner/progress/skeleton chuẩn gpui-kit cho trạng thái in-flight; line-wrap toggle ghost + icon tint; waterfall timing popover theo token theme.
7. **Git:** history modal 2 hàng (message / author + hash badge + relative time); Collapsible cho diff commit; diff view Split/Unified qua `Diff` component + `diff_parser`.
8. **Body editor:** multipart type selector dropdown (Text/File), file input hợp nhất full-width, "Add Part"/"Add Row" đưa lên toolbar, tự động đồng bộ header `Content-Type` theo body type.
9. **Script editor:** snippets panel đáp ứng layout split (dọc → hàng ngang scroll phía trên; ngang → cột phải 200px).
10. **About & Update:** About modal + tab Settings → About; version resolve từ GitHub Actions build (`ZEST_APP_VERSION`/`ZEST_COMMIT_HASH` qua `build.rs`: env → `GITHUB_REF_NAME` → `git describe` → `CARGO_PKG_VERSION`); update checker GitHub Releases (`zest_core::updater`) chạy trên `global_tokio_runtime` + oneshot bridge.
