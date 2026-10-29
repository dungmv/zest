# Kế hoạch & Nhật ký Migration sang gpui-kit Standard Components

- **Ngày cập nhật**: 2026-10-29
- **Trạng thái**: Hoàn thành Giai đoạn 1, 2 & 3 — Lập kế hoạch chi tiết Giai đoạn 4, 5, 6 & 7
- **Phiên bản thư viện**: `gpui-kit` (tích hợp `gpui-component 0.8.1` & `gpui-pre 0.3.10`)

---

## 1. Bảng đối chiếu toàn diện & Trạng thái Migration

| STT | UI Component | Trạng thái | Vị trí hiện tại trong Zest | Component tương đương trong `gpui-kit` | Ghi chú & Tình trạng |
| :--- | :--- | :--- | :--- | :--- | :--- |
| 1 | **Checkbox** | **ĐÃ HOÀN THÀNH** | `git/common.rs`, `import_modal.rs`, `kv_table.rs`, `multipart.rs` | `gpui_kit::component::checkbox::Checkbox` | Hỗ trợ 3 trạng thái (`checked`, `unchecked`, `indeterminate`), click/key event chuẩn macOS. |
| 2 | **Switch (Toggle)** | **ĐÃ HOÀN THÀNH** | `settings_modal/network_tab.rs` | `gpui_kit::component::switch::Switch` | Spring animation chuẩn macOS, xử lý checked/disabled tự động. |
| 3 | **Kbd (Phím tắt)** | **ĐÃ HOÀN THÀNH** | `settings_modal/shortcuts_tab.rs`, `command_palette.rs` | `gpui_kit::component::kbd::Kbd` | Phân giải tự động phím tắt macOS (⌘, ⌥, ⇧, ↵) và Windows (Ctrl, Alt). |
| 4 | **Tab & TabBar** | **ĐÃ HOÀN THÀNH** | `request_composer.rs`, `response_inspector.rs` | `gpui_kit::component::tab::{Tab, TabBar}` | TabBar underline variant, sliding indicator bounds, padding và gap đồng bộ. |
| 5 | **Alert (Banner)** | **ĐÃ HOÀN THÀNH** | `git/common.rs`, `import_modal.rs` | `gpui_kit::component::alert::Alert` | Banner cảnh báo trạng thái, hỗ trợ error/warning/info và nút close tự động. |
| 6 | **Empty States** | **ĐÃ HOÀN THÀNH** | `git/common.rs`, `cookies_tab.rs`, `console_panel.rs`, `diff_view.rs`, `body_editor/mod.rs` | `gpui_kit::component::empty::Empty` | Chuẩn hóa header, icon, title, description và content actions. |
| 7 | **Button Actions** | **ĐÃ HOÀN THÀNH** | `kv_table.rs`, `body_editor/mod.rs`, `response_inspector.rs`, `import_modal.rs`, `git/*_modal.rs` | `gpui_kit::component::button::Button` | Button `primary`, `ghost`, `small`, `xsmall`, tích hợp `loading`, icon và tooltip. |
| 8 | **Delete Button** | **ĐÃ HOÀN THÀNH** | `kv_table.rs`, `body_editor/multipart.rs` | Container tương tác `size(px(24.))` | Kích thước chuẩn `sm` 24x24px, icon 14px, ghost style `t.text_muted`, hover chuyển sang đỏ mềm dịu `t.json_null` (`#f87171` / `#e11d48`) không gây chói mắt và không gây panic. |
| 9 | **Line Wrap Toggle** | **ĐÃ HOÀN THÀNH** | `response_inspector.rs` | `Button::new("btn-toggle-wrap")` | Disabled là ghost button thông thường, enabled nổi bật với `ButtonVariant::Primary` (cyan/blue). |
| 10 | **Dialog / Modals** | **CÒN THIẾU** | `settings_modal/`, `env_modal.rs`, `import_modal.rs`, `scope_settings_modal.rs`, `git/*_modal.rs` | `gpui_kit::component::dialog::{Dialog, DialogHeader, DialogTitle, DialogContent, DialogFooter}` | Hiện tại đang tự vẽ backdrop thủ công `div().absolute().inset_0().bg(rgba(0x00000088))`. |
| 11 | **Select / Dropdown** | **CÒN THIẾU** | `request_bar.rs` (Method), `titlebar.rs` (Env), `auth_tab.rs` (Auth Type), `body_editor/mod.rs` (Body Type) | `gpui_kit::component::select::Select` & `menu::DropdownMenu` | Hiện tự vẽ menu floating hoặc popup menu ad-hoc. |
| 12 | **Tag / Badge** | **CÒN THIẾU** | `status_badge.rs` (`MethodBadge`, `StatusCodeBadge`), `tabs_with_badge.rs` | `gpui_kit::component::tag::Tag` & `badge::Badge` | Hiện tự vẽ bằng `div()` bo tròn thủ công. |
| 13 | **Toast / Notification** | **CÒN THIẾU** | `components/toast.rs` | `gpui_kit::component::notification::*` | Hiện tự render danh sách toast thủ công ở góc màn hình. |
| 14 | **Separator / Divider** | **CÒN THIẾU** | `scope_settings_modal.rs`, `script_editor.rs`, sidebar dividers | `gpui_kit::component::separator::Separator` | Đang dùng các khối `div().h(px(1.)).bg(t.border)`. |
| 15 | **Tooltip Unification** | **CÒN THIẾU** | Một số nút icon trong sidebar, request item, query params | `gpui_kit::component::tooltip::Tooltip` | Một số vị trí còn dùng text title hoặc hover thủ công chưa gắn Tooltip đồng nhất. |
| 16 | **Git Diff View** | **CÒN THIẾU** | `git/diff_view.rs` | `gpui_kit::component::diff::Diff` | Đang tự phân tích diff hunks thủ công bằng regex và tự render table dòng code. |

---

## 2. Phần 1: Chi tiết những gì ĐÃ LÀM ĐƯỢC

### 2.1 Giai đoạn 1: Form Controls & Tab Navigation (Hoàn thành)
- **`Checkbox`**:
  - `crates/app/src/components/git/common.rs`: Delegate `render_checkbox` sang `gpui_kit::component::checkbox::Checkbox`, hỗ trợ 3 trạng thái (`checked`, `unchecked`, `indeterminate`/`partial`).
  - `crates/app/src/components/import_modal.rs`: Checkbox chọn file và import environment.
  - `crates/app/src/components/kv_table.rs`: Checkbox kích hoạt/vô hiệu hóa từng dòng header / query param / form URL.
  - `crates/app/src/components/body_editor/multipart.rs`: Checkbox bật/tắt từng field multipart form.
- **`Switch`**:
  - `crates/app/src/components/settings_modal/network_tab.rs`: Thay thế toàn bộ toggle div thủ công bằng `Switch::new("switch-follow-redirects")` và `Switch::new("switch-validate-tls")`.
- **`Kbd`**:
  - `crates/app/src/components/settings_modal/shortcuts_tab.rs`: Sử dụng `Kbd::new(Keystroke::parse(...))` cho danh sách phím tắt hệ thống.
- **`TabBar` & `Tab`**:
  - `crates/app/src/app/request_composer.rs`: Thanh tab `Body`, `Params`, `Headers`, `Auth`, `Pre-request`, `Post-response` chuyển sang `TabBar::new(...).underline()`.
  - `crates/app/src/components/response_inspector.rs`: Thanh tab `Response`, `Request Sent`, `Headers`, `Cookies`, `Console` chuyển sang `TabBar::new(...).underline()`.
  - Tự động hóa khoảng cách (`gap: 16px`), sliding indicator animation và border đáy.

### 2.2 Giai đoạn 2: Feedback, Empty States & Row Actions (Hoàn thành)
- **`Alert`**:
  - `crates/app/src/components/git/common.rs`: `render_error_banner` chuyển sang `gpui_kit::component::alert::Alert::error(...).banner()`, tự động áp dụng cho 4 modal: `branch_modal.rs`, `commit_modal.rs`, `history_modal.rs`, `remotes_modal.rs`.
  - `crates/app/src/components/import_modal.rs`: Lỗi parsing spec chuyển sang `Alert::error(...)`.
- **`Empty`**:
  - `crates/app/src/components/git/common.rs`: `render_not_a_repo_view` chuyển sang `Empty::new().header(...)`.
  - `crates/app/src/components/settings_modal/cookies_tab.rs`: Empty state không có cookie.
  - `crates/app/src/components/console_panel.rs`: Empty state console log.
  - `crates/app/src/components/git/diff_view.rs`: Empty state khi chưa chọn file hoặc không có diff.
  - `crates/app/src/components/body_editor/mod.rs`: Empty state khi chọn `RequestBody::None`.
- **`Button` Actions**:
  - `Format JSON`, `Format XML`, `Add Part`, `Add Row` chuyển sang `Button::new(...)`.
  - **Nút Delete**: Kích thước chuẩn `sm` (24x24px, icon 14px), style ghost với nền trong suốt và màu chữ/icon `t.text_muted`. Khi hover chuyển sang màu đỏ nhạt mềm dịu (`t.json_null` - coral/rose red `#f87171` / `#e11d48`, tránh màu đỏ rực) kèm hiệu ứng nền `t.json_null.opacity(0.12)` và `Tooltip::new("Delete row")`. Đã giải quyết triệt để lỗi runtime panic `hover style already set`.
  - **Nút Line Wrap**: Khi disable hiển thị dưới dạng ghost button tiêu chuẩn (`btn.ghost()`), khi enable hiển thị nổi bật với màu primary (`btn.primary()`), trực quan rõ ràng 100% trên cả Dark mode và Light mode.

### 2.3 Giai đoạn 3: Command Palette & Modals Buttons (Hoàn thành)
- **`Kbd` trong Command Palette**:
  - `crates/app/src/components/command_palette.rs`: Chuẩn hóa phím tắt `cmd-enter`, `cmd-,`, `cmd-shift-g`, `cmd-i`, `escape` qua helper `render_kbd_hint`.
  - Nút ESC đóng palette được bọc trong component `Kbd`.
  - Thay thế emoji `"📜"` bằng icon `IconName::GitCommitHorizontal`.
- **Buttons trong các Modal**:
  - `crates/app/src/components/import_modal.rs`: `btn-close-import-modal`, `btn-cancel-import`, `btn-execute-import` chuyển sang `Button`.
  - `crates/app/src/components/git/commit_modal.rs`: `btn-commit-action` chuyển sang `Button::primary()` tích hợp spinner `loading(is_loading)` bản địa; `btn-git-commit-close` chuyển sang `Button::ghost()`.
  - `crates/app/src/components/git/branch_modal.rs`: `btn-cancel-create-branch`, `btn-confirm-create-branch`, `btn-git-branch-close` chuyển sang `Button`.
  - `crates/app/src/components/git/history_modal.rs`: `btn-git-history-close` chuyển sang `Button`.
  - `crates/app/src/components/git/remotes_modal.rs`: `btn-add-remote-action`, `btn-git-remotes-close` chuyển sang `Button`.

---

## 3. Phần 2: Chi tiết những gì CÒN THIẾU & CẦN LÀM TIẾP

Dưới đây là danh sách toàn bộ các component và màn hình hiện còn đang tự vẽ thủ công (ad-hoc), cần được tiếp tục migrate trong các giai đoạn tới:

### 3.1 Nhóm 1: Dialog & Modal Overlays (`gpui_kit::component::dialog::*`) (Đã hoàn thành ✅)
- **Thực trạng ban đầu**: 5 màn hình modal trong Zest tự vẽ container overlay ad-hoc:
  ```rust
  div()
      .id("modal-overlay")
      .absolute()
      .inset_0()
      .bg(gpui_kit::rgba(0x00000088))
      .flex()
      .items_center()
      .justify_center()
  ```
- **Kết quả triển khai**:
  - Tạo mới component thống nhất `crates/app/src/components/modal_dialog.rs` (`ModalDialog`) kết hợp chặt chẽ semantic layout từ `gpui_kit::component::dialog::{DialogHeader, DialogTitle, DialogDescription, DialogContent, DialogFooter}`.
  - Tự động hóa backdrop tối mờ, chặn click-through xuyên thấu (`occlude`), xử lý phím `Escape` và click backdrop để đóng modal mà không cần code lặp lại.
  - Thay thế thành công toàn bộ 5 hệ thống modal sang `ModalDialog` và semantic dialog parts:
    1. `crates/app/src/components/settings_modal/mod.rs` (`SettingsModal`)
    2. `crates/app/src/components/env_modal.rs` (`EnvironmentModal`)
    3. `crates/app/src/components/import_modal.rs` (`ImportModal` - tích hợp `DialogFooter` cho Cancel & Import actions)
    4. `crates/app/src/components/scope_settings_modal.rs` (`ScopeSettingsModal`)
    5. `crates/app/src/components/git/*_modal.rs` (`GitBranchModal`, `GitCommitModal`, `GitHistoryModal`, `GitRemotesModal` - thay thế các ký tự emoji thô bằng icon chính quy `IconName::GitBranch`, `IconName::GitCommitHorizontal`, `IconName::Clock`, `IconName::Globe`).

### 3.2 Nhóm 2: Select & DropdownMenu (`gpui_kit::component::select::*` & `menu::DropdownMenu`)
- **Thực trạng**: Nhiều vị trí chọn giá trị từ danh sách đang dùng popover hoặc floating div thủ công:
  1. **Method Selector** (`crates/app/src/components/request_bar.rs`): Nút chọn HTTP Method (`GET`, `POST`, `PUT`, `DELETE`, `PATCH`, `HEAD`, `OPTIONS`) đang dùng dropdown menu tự quản lý state mở/đóng.
  2. **Environment Selector** (`crates/app/src/components/titlebar.rs`): Dropdown chuyển đổi môi trường đang dùng popup menu thủ công.
  3. **Auth Type Selector** (`crates/app/src/app/auth_tab.rs`): Nút chọn kiểu xác thực (`Inherit`, `No Auth`, `Bearer Token`, `Basic Auth`, `API Key`).
  4. **Body Type Selector** (`crates/app/src/components/body_editor/mod.rs`): Chọn định dạng Body (`None`, `JSON`, `XML`, `Form URL-Encoded`, `Multipart Form`, `Raw Text`, `Binary File`).
- **Mục tiêu thay thế**:
  - Chuẩn hóa sang `gpui_kit::component::select::Select` hoặc `gpui_kit::component::menu::DropdownMenu`.
  - Có hỗ trợ keyboard navigation (phím mũi tên lên/xuống, Enter để chọn).

### 3.3 Nhóm 3: Tag & Status Badges (`gpui_kit::component::tag::Tag`, `badge::Badge`)
- **Thực trạng**:
  1. `crates/app/src/components/status_badge.rs`: `MethodBadge` và `StatusCodeBadge` hiện đang tự vẽ bằng `div()` thủ công với màu sắc tương ứng (`t.method_color(method)`, `t.status_color(status)`).
  2. `crates/app/src/components/tabs_with_badge.rs`: `render_tab_badge` vẽ số lượng (`count`) bằng div tròn nhỏ màu xám/xanh.
- **Mục tiêu thay thế**:
  - `MethodBadge` & `StatusCodeBadge`: Chuyển sang `gpui_kit::component::tag::Tag` với `TagVariant::Custom { color, foreground, border }`.
  - `render_tab_badge`: Sử dụng `gpui_kit::component::badge::Badge` với variant đếm số lượng bản địa.

### 3.4 Nhóm 4: Toast & System Notifications (`gpui_kit::component::notification::*`)
- **Thực trạng**: `crates/app/src/components/toast.rs` hiện tự quản lý stack mảng `toasts` và tự render các thẻ thông báo nổi ở góc dưới.
- **Mục tiêu thay thế**:
  - Tích hợp với `gpui_kit::component::notification::*`.
  - Giữ nguyên `ToastState` entity để duy trì luồng sự kiện không phá vỡ API hiện có.

### 3.5 Nhóm 5: Separator / Divider Lines (`gpui_kit::component::separator::Separator`)
- **Thực trạng**: Khá nhiều đường kẻ ngăn cách giữa các cột hoặc các phần trong modal/toolbar đang dùng `div().h(px(1.)).bg(t.border)` hoặc `div().w(px(1.)).bg(t.border)`:
  - `crates/app/src/components/scope_settings_modal.rs`
  - `crates/app/src/components/script_editor.rs`
  - Thanh phân cách toolbar và panels.
- **Mục tiêu thay thế**: Dùng `Separator::new().horizontal()` / `Separator::new().vertical()`.

### 3.6 Nhóm 6: Tooltip Toàn diện (`gpui_kit::component::tooltip::Tooltip`)
- **Thực trạng**: Vẫn còn một số nút icon nhỏ (như pin request, copy path, duplicate item trong sidebar context menu, icon trong response viewer) chưa có tooltip hoặc chỉ dùng nhãn hover thô.
- **Mục tiêu thay thế**: Chuẩn hóa 100% các action icon đều có `.tooltip(...)` sử dụng `Tooltip::new(...)`.

### 3.7 Nhóm 7: Git Diff View Component (`gpui_kit::component::diff::Diff`)
- **Thực trạng**: `crates/app/src/components/git/diff_view.rs` (~480 LOC) tự parse diff text, hunks, dòng thêm/bớt và tự render giao diện split/unified.
- **Mục tiêu thay thế**: Nghiên cứu khả năng tích hợp `gpui_kit::component::diff::Diff` của `gpui-component` để tận dụng GPU syntax highlighting và side-by-side view siêu mượt mà.

---

## 4. Phần 3: Lộ trình phân kỳ triển khai tiếp theo (Next Phases Roadmap)

```
[Hoàn thành] Giai đoạn 1: Form Controls & Tabs (Checkbox, Switch, Kbd, TabBar+Tab)
       ↓
[Hoàn thành] Giai đoạn 2: Feedback & Empty States (Alert, Empty, Row Actions, Delete & Wrap buttons)
       ↓
[Hoàn thành] Giai đoạn 3: Command Palette & Modal Buttons (Kbd hints, Dialog Buttons)
       ↓
[Hoàn thành] Giai đoạn 6: Dialog & Overlay System (Settings, Environment, Scope, Git Modals)
       ↓
[Tiếp theo] Giai đoạn 4: Select & DropdownMenu (Method, Environment, Auth & Body Type selectors)
       ↓
[Tiếp theo] Giai đoạn 5: Tag & Badge Component (MethodBadge, StatusCodeBadge, Tab Counter Badges)
       ↓
[Tiếp theo] Giai đoạn 7: Toasts, Separators & GPU Diff View (Hoàn tất toàn bộ hệ thống)
```

### Kế hoạch chi tiết Giai đoạn 6: Dialog & Modal Overlays (Đã hoàn thành ✅)
1. **Nhiệm vụ 6.1**: Chuyển đổi `SettingsModal` sang `ModalDialog` + `DialogHeader`/`DialogTitle`.
2. **Nhiệm vụ 6.2**: Chuyển đổi `EnvModal` sang `ModalDialog` + `DialogHeader`/`DialogTitle`.
3. **Nhiệm vụ 6.3**: Chuyển đổi `ScopeSettingsModal` sang `ModalDialog` + `DialogHeader`/`DialogTitle`.
4. **Nhiệm vụ 6.4**: Chuyển đổi `ImportModal` sang `ModalDialog` + `DialogFooter`.
5. **Nhiệm vụ 6.5**: Chuyển đổi các Git modal (`commit`, `branch`, `history`, `remotes`) sang `ModalDialog` và icons chuẩn.

### Kế hoạch chi tiết Giai đoạn 4: Select & Dropdown Menu
1. **Nhiệm vụ 4.1**: Chuẩn hóa HTTP Method Dropdown trong `request_bar.rs` bằng `gpui_kit::component::select::Select` hoặc `menu::DropdownMenu`.
2. **Nhiệm vụ 4.2**: Chuẩn hóa Environment Dropdown trong `titlebar.rs`.
3. **Nhiệm vụ 4.3**: Chuẩn hóa Auth Type Selector trong `app/auth_tab.rs`.
4. **Nhiệm vụ 4.4**: Chuẩn hóa Body Type Selector trong `body_editor/mod.rs`.

### Kế hoạch chi tiết Giai đoạn 5: Tag & Badge Component
1. **Nhiệm vụ 5.1**: Chuyển `MethodBadge` trong `status_badge.rs` sang `Tag::new(...)`.
2. **Nhiệm vụ 5.2**: Chuyển `StatusCodeBadge` trong `status_badge.rs` sang `Tag::new(...)`.
3. **Nhiệm vụ 5.3**: Chuyển `render_tab_badge` trong `tabs_with_badge.rs` sang `Badge::new()`.

---

## 5. Quy tắc Kiểm định & Tiêu chí Chất lượng (Verification Checklist)

Mọi thay đổi trong quá trình migration phải đáp ứng nghiêm ngặt 3 tiêu chuẩn bắt buộc:
1. **Định dạng code**: `cargo fmt --check` không có khác biệt.
2. **Clippy tĩnh**: `cargo clippy --all-targets -- -D warnings` đạt **0 cảnh báo**.
3. **Độ ổn định & Kiểm thử**: `cargo test` pass **100% test cases** (188/188 tests).
4. **Tránh Panic Hover**: Không bao giờ gọi trực tiếp `.hover(...)` trên `Button` của `gpui-kit` vì `Button::render` đã tự đăng ký hover nội bộ.
