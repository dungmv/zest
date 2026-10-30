# Kế hoạch & Nhật ký Migration sang gpui-kit Standard Components

- **Ngày cập nhật**: 2026-10-30
- **Trạng thái**: Hoàn thành Giai đoạn 1, 2 & 3 — Lập kế hoạch toàn diện Giai đoạn 4, 5, 6, 7, 8 & 9
- **Phiên bản thư viện**: `gpui-kit` (tích hợp `gpui-component 0.8.1` & `gpui-pre 0.3.10`)

---

## 1. Bảng đối chiếu toàn diện & Trạng thái Migration (24 UI Components)

Dưới đây là bảng phân loại và đối chiếu toàn bộ 24 nhóm UI component giữa codebase Zest và bộ component chuẩn của `gpui-kit`:

### Nhóm A: Form & Interactive Controls
| STT | UI Component | Trạng thái | Vị trí hiện tại trong Zest | Component tương đương trong `gpui-kit` | Ghi chú & Tình trạng |
| :--- | :--- | :--- | :--- | :--- | :--- |
| 1 | **Checkbox** | **ĐÃ HOÀN THÀNH** | `git/common.rs`, `import_modal.rs`, `kv_table.rs`, `multipart.rs` | `gpui_kit::component::checkbox::Checkbox` | Hỗ trợ 3 trạng thái (`checked`, `unchecked`, `indeterminate`), click/keyboard event chuẩn macOS. |
| 2 | **Switch (Toggle)** | **ĐÃ HOÀN THÀNH** | `settings_modal/network_tab.rs` | `gpui_kit::component::switch::Switch` | Spring animation chuẩn macOS, xử lý checked/disabled tự động. |
| 3 | **Button Actions** | **ĐÃ HOÀN THÀNH** | `kv_table.rs`, `body_editor/mod.rs`, `response_inspector.rs`, `import_modal.rs`, `git/*_modal.rs` | `gpui_kit::component::button::Button` | Button `primary`, `ghost`, `small`, `xsmall`, tích hợp `loading`, icon và tooltip. |
| 4 | **Delete Button** | **ĐÃ HOÀN THÀNH** | `kv_table.rs`, `body_editor/multipart.rs` | Container tương tác `size(px(24.))` | Chuẩn `sm` 24x24px, icon 14px, ghost style `t.text_muted`, hover đỏ mềm `t.json_null` (`#f87171`/`#e11d48`) không gây chói mắt và không panic. |
| 5 | **Line Wrap Toggle** | **ĐÃ HOÀN THÀNH** | `response_inspector.rs` | `Button::new("btn-toggle-wrap")` | Disabled là ghost button thông thường, enabled nổi bật với `ButtonVariant::Primary` (cyan/blue). |
| 6 | **Radio / RadioGroup** | **SẴN SÀNG** | `gpui_kit::component::radio` | `gpui_kit::component::radio::{Radio, RadioGroup}` | Cơ chế Radio & RadioGroup của gpui-kit sẵn sàng sử dụng khi cần các cụm tùy chọn exclusive inline. |
| 7 | **Select / Dropdown** | **ĐÃ HOÀN THÀNH** | `general_tab.rs`, `network_tab.rs`, `import_modal.rs`, `request_bar.rs`, `titlebar.rs`, `auth_tab.rs`, `body_editor/mod.rs` | `gpui_kit::component::button::Button` + `dropdown_menu` | Chuẩn hóa toàn bộ: Theme Mode, Request Timeout, Import Format, HTTP Method, Env, Auth Type, Body Format thành Dropdown Menu đồng bộ với checkmark và icon. |
| 8 | **Timeout HTTP Selector** | **ĐÃ HOÀN THÀNH** | `settings_modal/network_tab.rs` (Timeout) | `Button::new("btn-timeout-dropdown")` | Chuẩn hóa tùy chỉnh timeout kết nối HTTP thành Dropdown Menu (`[10s, 30s, 60s, 120s]`) theo đúng yêu cầu UX/UI. |

### Nhóm B: Navigation & Layout
| STT | UI Component | Trạng thái | Vị trí hiện tại trong Zest | Component tương đương trong `gpui-kit` | Ghi chú & Tình trạng |
| :--- | :--- | :--- | :--- | :--- | :--- |
| 9 | **Tab & TabBar** | **ĐÃ HOÀN THÀNH** | `request_composer.rs`, `response_inspector.rs` | `gpui_kit::component::tab::{Tab, TabBar}` | TabBar underline variant, sliding indicator bounds, padding và gap đồng bộ. |
| 10 | **Breadcrumb** | **ĐÃ HOÀN THÀNH** | `titlebar.rs` (Trung tâm Titlebar - Vị trí 2) | `gpui_kit::component::breadcrumb::{Breadcrumb, BreadcrumbItem}` | Hiển thị phân cấp thư mục và đường dẫn request (`Folder › Request › Example`) trực quan, gọn gàng trong ô tìm kiếm trung tâm Titlebar. |
| 11 | **Collapsible / Accordion** | **ĐÃ HOÀN THÀNH** | `git/history_modal.rs` | `gpui_kit::component::collapsible::Collapsible` | Thu gọn/mở rộng file changes diff trong lịch sử git với hiệu ứng spring reveal mượt mà bản địa. |
| 12 | **Resizable Panels** | **ĐÃ HOÀN THÀNH** | `app/mod.rs` (Sidebar, Composer/Inspector splits) | `gpui_kit::component::resizable::{h_resizable, v_resizable, resizable_panel}` | Thanh kéo chia tỷ lệ kích thước Sidebar và chia tách ngang/dọc giữa Composer và Inspector. |

### Nhóm C: Feedback, Status & Loading
| STT | UI Component | Trạng thái | Vị trí hiện tại trong Zest | Component tương đương trong `gpui-kit` | Ghi chú & Tình trạng |
| :--- | :--- | :--- | :--- | :--- | :--- |
| 13 | **Alert (Banner)** | **ĐÃ HOÀN THÀNH** | `git/common.rs`, `import_modal.rs` | `gpui_kit::component::alert::Alert` | Banner cảnh báo trạng thái, hỗ trợ error/warning/info và nút close tự động. |
| 14 | **Empty States** | **ĐÃ HOÀN THÀNH** | `git/common.rs`, `cookies_tab.rs`, `console_panel.rs`, `diff_view.rs`, `body_editor/mod.rs` | `gpui_kit::component::empty::Empty` | Chuẩn hóa header, icon, title, description và content actions. |
| 15 | **Tag (Method & Status)** | **CÒN THIẾU** | `status_badge.rs` (`MethodBadge`, `StatusCodeBadge`) | `gpui_kit::component::tag::Tag` | Hiện tự vẽ bằng `div()` bo tròn thủ công. Cần chuyển sang `Tag::new().variant(TagVariant::Custom)`. |
| 16 | **Badge (Counters)** | **CÒN THIẾU** | `tabs_with_badge.rs` (`render_tab_badge`), sidebar counts | `gpui_kit::component::badge::Badge` | Đếm số lượng headers `[15]`, cookies `[0]`, console logs `[2]`. |
| 17 | **Notification / Toast** | **CÒN THIẾU** | `components/toast.rs` | `gpui_kit::component::notification::*` | Hiện tự render danh sách toast mảng thủ công ở góc màn hình. |
| 18 | **Progress / ProgressBar** | **CÒN THIẾU** | Request in-flight progress bar, collection runner | `gpui_kit::component::progress::Progress` | Thanh tiến trình chạy collection tests và upload/download payload lớn. |
| 19 | **Spinner** | **CÒN THIẾU** | `response_inspector.rs` (loading request) | `gpui_kit::component::spinner::Spinner` | Một số vị trí dùng icon xoay thủ công `IconName::RefreshCw`. |
| 20 | **Skeleton / Shimmer** | **CÒN THIẾU** | Response inspector placeholder, Git diff placeholder | `gpui_kit::component::skeleton::Skeleton` | Hiệu ứng shimmer khi đang đợi kết quả phản hồi mạng hoặc đọc cây thư mục lớn. |

### Nhóm D: Overlays, Modals & Menus
| STT | UI Component | Trạng thái | Vị trí hiện tại trong Zest | Component tương đương trong `gpui-kit` | Ghi chú & Tình trạng |
| :--- | :--- | :--- | :--- | :--- | :--- |
| 21 | **Dialog / Modals** | **CÒN THIẾU** | `settings_modal/`, `env_modal.rs`, `import_modal.rs`, `scope_settings_modal.rs`, `git/*_modal.rs` | `gpui_kit::component::dialog::{Dialog, DialogHeader, DialogTitle, DialogContent, DialogFooter}` | Hiện đang tự vẽ backdrop thủ công `div().absolute().inset_0().bg(rgba(0x00000088))`. |
| 22 | **Command Palette** | **CÒN THIẾU** | `command_palette.rs` | `gpui_kit::component::command::{Command, CommandInput, CommandList, CommandItem, CommandGroup}` | Đã chuẩn hóa phím tắt `Kbd`, nhưng khung dialog và filter list vẫn viết thủ công. |
| 23 | **Dropdown & Context Menu** | **CÒN THIẾU** | Sidebar context menu (folder, request, example nodes) | `gpui_kit::component::menu::{DropdownMenu, PopupMenu}` | Chuẩn hóa menu chuột phải: Rename, Duplicate, Delete, Pin, Copy cURL. |
| 24 | **Tooltip Unification** | **CÒN THIẾU** | Sidebar icon actions, response tabs toolbar | `gpui_kit::component::tooltip::Tooltip` | Chuẩn hóa 100% action icons đều có `.tooltip(...)` thay vì hover text title thô. |

### Nhóm E: Data Display & Content
| STT | UI Component | Trạng thái | Vị trí hiện tại trong Zest | Component tương đương trong `gpui-kit` | Ghi chú & Tình trạng |
| :--- | :--- | :--- | :--- | :--- | :--- |
| 25 | **Kbd (Phím tắt)** | **ĐÃ HOÀN THÀNH** | `settings_modal/shortcuts_tab.rs`, `command_palette.rs` | `gpui_kit::component::kbd::Kbd` | Phân giải tự động phím tắt macOS (⌘, ⌥, ⇧, ↵) và Windows (Ctrl, Alt). |
| 26 | **Separator / Divider** | **CÒN THIẾU** | `scope_settings_modal.rs`, `script_editor.rs`, toolbar lines | `gpui_kit::component::separator::Separator` | Đang dùng các khối `div().h(px(1.)).bg(t.border)`. |
| 27 | **Git Diff View** | **CÒN THIẾU** | `git/diff_view.rs` | `gpui_kit::component::diff::Diff` | Đang tự phân tích diff hunks thủ công bằng regex và tự render table dòng code. |
| 28 | **Table** | **CÒN THIẾU** | `kv_table.rs`, `multipart.rs` | `gpui_kit::component::table::Table` | Hiện dùng flex row. Có thể nâng cấp lên `Table` có column header resize. |

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

### 2.4 Giai đoạn 4: Hoàn thiện toàn bộ Nhóm A (Dropdown Select & Form Controls) (Hoàn thành)
- **Select & Dropdown Menus (`Button::dropdown_menu`)**:
  - `crates/app/src/components/settings_modal/general_tab.rs`: Lựa chọn Theme Mode (`[System (Follow OS), Dark, Light]`) sử dụng Dropdown Menu với icon và checkmark đồng bộ.
  - `crates/app/src/components/settings_modal/network_tab.rs`: Tùy chỉnh timeout kết nối HTTP (`[10s, 30s, 60s, 120s]`) với chiều rộng cố định `w(px(88.))` tránh giật lag layout UI khi chuyển giá trị, đơn vị hiển thị chuẩn hóa `s`.
  - `crates/app/src/components/import_modal.rs`: Lựa chọn Specification Format (`[OpenAPI 3.1 / 3.0, Postman Collection v2.1]`) đặt gọn ở góc phải, bên trái là nhãn thông tin và mô tả giúp giao diện thoáng đãng, đồng bộ.
  - HTTP Method selector (`request_bar.rs`), Environment selector (`titlebar.rs`), Auth Type selector (`auth_tab.rs`), Body Type selector (`body_editor/mod.rs`): Toàn bộ sử dụng chuẩn `Button` kết hợp menu popup `gpui-kit`.

### 2.5 Giai đoạn 5: Hoàn thiện toàn bộ Nhóm B (Navigation & Layout) (Hoàn thành)
- **`Breadcrumb` (`gpui_kit::component::breadcrumb::{Breadcrumb, BreadcrumbItem}`)**:
  - `crates/app/src/components/titlebar.rs`: Áp dụng tại Vị trí 2 (Trung tâm Titlebar). Cố định chiều rộng khung search `w(px(340.))` tránh rung lắc/resize UI khi chuyển request. Áp dụng cơ chế **Smart Head Truncation** (rút gọn phần đầu thành `… › Folder › Request` khi có từ 3 cấp trở lên) kèm giới hạn chiều rộng và `truncate()` từng mục để không bao giờ bị tràn. Hover hiển thị tooltip toàn bộ đường dẫn đầy đủ.
- **`Collapsible` (`gpui_kit::component::collapsible::Collapsible`)**:
  - `crates/app/src/components/git/history_modal.rs`: Tích hợp component `Collapsible` cho khối diff chi tiết các file thay đổi trong commit, hỗ trợ spring animation đóng/mở mượt mà.
- **`Resizable` (`gpui_kit::component::resizable::{h_resizable, v_resizable, resizable_panel}`)**:
  - `crates/app/src/app/mod.rs`: Toàn bộ phân vùng ứng dụng (Sidebar split, Composer/Response split theo chiều dọc và chiều ngang) được quản lý bằng bộ điều khiển Resizable bản địa của gpui-kit.
- **`Tab` & `TabBar` (`gpui_kit::component::tab::{Tab, TabBar}`)**:
  - Đã hoàn thành từ Giai đoạn 2 cho cả Request Composer và Response Inspector.

---

## 3. Phần 2: Chi tiết những gì CÒN THIẾU & CẦN LÀM TIẾP (Các giai đoạn tiếp theo)

### 3.1 Giai đoạn 6: Tag & Badge Component (Thực hiện tiếp theo)
1. **`MethodBadge`** (`crates/app/src/components/status_badge.rs`):
   - Chuyển sang `gpui_kit::component::tag::Tag` với `TagVariant::Custom { color, foreground, border }`.
2. **`StatusCodeBadge`** (`crates/app/src/components/status_badge.rs`):
   - Chuyển sang `gpui_kit::component::tag::Tag` hiển thị mã trạng thái HTTP (200 OK, 404 Not Found, 500 Internal Server Error).
3. **Badge Counter** (`crates/app/src/components/tabs_with_badge.rs`):
   - Thay thế `render_tab_badge` bằng `gpui_kit::component::badge::Badge` với variant đếm số lượng bản địa.

### 3.3 Giai đoạn 6: Dialog & Overlay System (Modals chuẩn hóa)
1. **`SettingsModal`** (`crates/app/src/components/settings_modal/mod.rs`):
   - Chuyển khung modal và backdrop `div().absolute().inset_0()` sang `gpui_kit::component::dialog::Dialog`.
2. **`EnvModal`** (`crates/app/src/components/env_modal.rs`):
   - Chuyển đổi khung quản lý biến môi trường sang `Dialog`.
3. **`ImportModal`** (`crates/app/src/components/import_modal.rs`):
   - Chuyển đổi hộp thoại import OpenAPI/Postman sang `Dialog`.
4. **`ScopeSettingsModal`** (`crates/app/src/components/scope_settings_modal.rs`):
   - Chuyển đổi sang `Dialog`.
5. **Git Modals** (`commit_modal.rs`, `branch_modal.rs`, `history_modal.rs`, `remotes_modal.rs`):
   - Chuẩn hóa layout khung hội thoại bằng `Dialog`.

### 3.4 Giai đoạn 7: Command Palette Component
1. **`CommandPalette`** (`crates/app/src/components/command_palette.rs`):
   - Chuyển đổi toàn bộ component từ custom dialog & floating list sang `gpui_kit::component::command::{Command, CommandInput, CommandList, CommandItem, CommandGroup, CommandEmpty}`.
   - Hỗ trợ keyboard filtering, arrow key navigation và highlight tự động.

### 3.5 Giai đoạn 8: Notifications, Separator & Loading Shimmer
1. **`Toast` / Notifications** (`crates/app/src/components/toast.rs`):
   - Tích hợp với `gpui_kit::component::notification::*`.
2. **`Separator` / Dividers**:
   - Thay thế các đường kẻ `div().h(px(1.)).bg(t.border)` trong `scope_settings_modal.rs`, `script_editor.rs` và toolbar bằng `Separator::new()`.
3. **`Skeleton` / Shimmer Loading**:
   - Thêm hiệu ứng loading shimmer khi response đang in-flight hoặc khi parse collections dung lượng lớn.
4. **`Tooltip` Unification**:
   - Chuẩn hóa 100% action icons trong sidebar và panels có tooltip giải thích thao tác.

### 3.6 Giai đoạn 9: Advanced Components (Collapsible, ContextMenu, Resizable & GPU Diff)
1. **`Collapsible` trong Sidebar Tree** (`crates/app/src/components/sidebar/folder_node.rs`):
   - Tích hợp `Collapsible` cho việc đóng/mở thư mục mượt mà.
2. **`ContextMenu` trong Sidebar**:
   - Chuẩn hóa menu chuột phải trên request/folder nodes bằng `menu::PopupMenu`.
3. **`Resizable` Panels**:
   - Sử dụng `gpui_kit::component::resizable::*` cho thanh kéo co giãn sidebar và split view.
4. **`Diff` GPU-Accelerated Viewer** (`crates/app/src/components/git/diff_view.rs`):
   - Nghiên cứu tích hợp diff engine bản địa từ `gpui-component`.

---

## 4. Lộ trình phân kỳ tổng thể (Comprehensive Roadmap)

```
[Hoàn thành] Giai đoạn 1: Form Controls & Tabs (Checkbox, Switch, Kbd, TabBar+Tab)
       ↓
[Hoàn thành] Giai đoạn 2: Feedback & Empty States (Alert, Empty, Row Actions, Delete & Wrap buttons)
       ↓
[Hoàn thành] Giai đoạn 3: Command Palette & Modal Buttons (Kbd hints, Dialog Buttons)
       ↓
[Tiếp theo] Giai đoạn 4: Select, DropdownMenu & Radio (Method, Env, Auth, Body format selectors & Theme Radio)
       ↓
[Tiếp theo] Giai đoạn 5: Tag & Badge Component (MethodBadge, StatusCodeBadge, Tab Counter Badges)
       ↓
[Tiếp theo] Giai đoạn 6: Dialog & Overlay System (Settings, Environment, Import, Scope & Git Modals)
       ↓
[Tiếp theo] Giai đoạn 7: Command Palette System (Command, CommandInput, CommandList, CommandGroup)
       ↓
[Tiếp theo] Giai đoạn 8: Notifications, Separators, Skeleton Shimmer & Tooltip Unification
       ↓
[Tiếp theo] Giai đoạn 9: Collapsible Tree, ContextMenu, Resizable Panels & GPU Diff Viewer
```

---

## 5. Quy tắc Kiểm định & Tiêu chí Chất lượng (Verification Checklist)

Mọi thay đổi trong quá trình migration phải đáp ứng nghiêm ngặt 4 tiêu chuẩn bắt buộc:
1. **Định dạng code**: `cargo fmt --check` không có khác biệt.
2. **Clippy tĩnh**: `cargo clippy --all-targets -- -D warnings` đạt **0 cảnh báo**.
3. **Độ ổn định & Kiểm thử**: `cargo test` pass **100% test cases** (188/188 tests).
4. **Tránh Panic Hover**: Không bao giờ gọi trực tiếp `.hover(...)` trên `Button` của `gpui-kit` vì `Button::render` đã tự đăng ký hover nội bộ.
