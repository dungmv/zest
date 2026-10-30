# Kế hoạch & Nhật ký Migration sang gpui-kit Standard Components

- **Ngày cập nhật**: 2026-10-30
- **Trạng thái**: Hoàn thành toàn diện 24/24 UI Components (Nhóm A, B, C, D, E)
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
| 15 | **Tag (Method & Status)** | **ĐÃ HOÀN THÀNH** | `status_badge.rs`, `cookies_tab.rs` | `gpui_kit::component::tag::Tag` | MethodBadge, StatusCodeBadge dùng `Tag::custom`, Cookie properties (Secure, HttpOnly, SameSite, domain, path) dùng `Tag::success`, `Tag::info`, `Tag::secondary`. |
| 16 | **Badge (Counters)** | **ĐÃ HOÀN THÀNH** | `tabs_with_badge.rs` | `gpui_kit::component::tag::Tag` / `badge::Badge` | Đếm số lượng headers `[15]`, cookies `[0]`, console logs `[2]` trên tab bar qua tag bo tròn chuẩn. |
| 17 | **Notification / Toast** | **ĐÃ HOÀN THÀNH** | `components/toast.rs` | `ToastContainer` + `gpui_kit` icons & buttons | Khung toast thông báo Info, Success, Error với icon chuẩn, button dismiss ghost và shadow tự nhiên. |
| 18 | **Progress / ProgressBar** | **ĐÃ HOÀN THÀNH** | `response_inspector.rs` | `gpui_kit::component::progress::Progress` | Thanh tiến trình ngang indeterminate sliding animation khi request đang in-flight gửi tới server. |
| 19 | **Spinner** | **ĐÃ HOÀN THÀNH** | `response_inspector.rs`, `sidebar/request_node.rs` | `gpui_kit::component::spinner::Spinner` | Animated spinner xoay mượt mà với GPU animation khi request đang chạy trên Sidebar và Response Inspector. |
| 20 | **Skeleton / Shimmer** | **ĐÃ HOÀN THÀNH** | `response_inspector.rs`, `git/history_modal.rs` | `gpui_kit::component::skeleton::Skeleton` | Hiệu ứng shimmer placeholder với bounce ease animation khi đang tải response payload và commit history list. |

### Nhóm D: Overlays, Modals & Menus
| STT | UI Component | Trạng thái | Vị trí hiện tại trong Zest | Component tương đương trong `gpui-kit` | Ghi chú & Tình trạng |
| :--- | :--- | :--- | :--- | :--- | :--- |
| 21 | **Dialog / Modals** | **ĐÃ HOÀN THÀNH** | `settings_modal/`, `env_modal.rs`, `import_modal.rs`, `scope_settings_modal.rs`, `git/*_modal.rs` | `gpui_kit::component::dialog::{Dialog, DialogHeader, DialogTitle, DialogContent, DialogFooter}` | Toàn bộ 8 modal hệ thống (`SettingsModal`, `EnvModal`, `ImportModal`, `ScopeSettingsModal`, `CommitModal`, `BranchModal`, `HistoryModal`, `RemotesModal`) đã được chuẩn hóa qua `ModalDialog` tích hợp semantic `DialogHeader`, `DialogTitle`, `DialogDescription`, `DialogContent`, `DialogFooter`. |
| 22 | **Command Palette** | **ĐÃ HOÀN THÀNH** | `command_palette.rs` | `gpui_kit::component::command` / `kbd` / `separator` | Đã chuẩn hóa phím tắt `Kbd`, search input, và sử dụng `Separator::horizontal()` phân cách rõ ràng các khối lệnh: Actions, Switch Request, và Switch Environment. |
| 23 | **Dropdown & Context Menu** | **ĐÃ HOÀN THÀNH** | Sidebar context menu (`request_node.rs`, `folder_node.rs`, `example_node.rs`) | `gpui_kit::component::menu::{ContextMenuExt, PopupMenuItem}` | Chuẩn hóa 100% menu chuột phải: Rename, Duplicate, Delete, Pin, Move Up/Down, Discard Changes, Copy YAML/cURL kèm đầy đủ icon tương ứng. |
| 24 | **Tooltip Unification** | **ĐÃ HOÀN THÀNH** | Titlebar, Sidebar footer, Request bar, Response tabs toolbar | `gpui_kit::component::tooltip::Tooltip` | Chuẩn hóa 100% action icons và toolbar buttons: Workspace switcher, Environment dropdown, Split layout toggle, Settings, Response actions, và Git footer đều có `.tooltip(...)`. |

### Nhóm E: Data Display & Content
| STT | UI Component | Trạng thái | Vị trí hiện tại trong Zest | Component tương đương trong `gpui-kit` | Ghi chú & Tình trạng |
| :--- | :--- | :--- | :--- | :--- | :--- |
| 25 | **Kbd (Phím tắt)** | **ĐÃ HOÀN THÀNH** | `settings_modal/shortcuts_tab.rs`, `command_palette.rs` | `gpui_kit::component::kbd::Kbd` | Phân giải tự động phím tắt macOS (⌘, ⌥, ⇧, ↵) và Windows (Ctrl, Alt). |
| 26 | **Separator / Divider** | **ĐÃ HOÀN THÀNH** | `settings_modal/`, `git/commit_modal.rs`, `git/history_modal.rs`, `command_palette.rs`, `scope_settings_modal.rs`, `request_bar.rs` | `gpui_kit::component::separator::Separator` | Chuẩn hóa toàn bộ đường phân cách ngang/dọc giữa các panel và section bằng `Separator::horizontal()` và `Separator::vertical()`. |
| 27 | **Git Diff View** | **ĐÃ HOÀN THÀNH** | `git/diff_view.rs` | `gpui_kit::component::diff::Diff` | Hỗ trợ 2 chế độ hiển thị (Side-by-side Split và Unified) với `Button` toggle, tooltip hướng dẫn, và kiến trúc phân tích hunk tương thích với diff parser của gpui-kit. |
| 28 | **Table** | **ĐÃ HOÀN THÀNH** | `kv_table.rs`, `multipart.rs` | `gpui_kit::component::table::Table` | Bảng Key-Value inputs đồng bộ hóa theo row index với InputState pool, hỗ trợ checkbox 3 trạng thái, autocomplete biến môi trường, và row action delete chuẩn. |

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

### 2.6 Giai đoạn 6: Hoàn thiện toàn bộ Nhóm C (Feedback, Status & Loading) (Hoàn thành)
- **`Tag` (`gpui_kit::component::tag::Tag`)**:
  - `status_badge.rs`: `MethodBadge` và `StatusCodeBadge` sử dụng `Tag::custom` với màu nền và viền theo theme.
  - `settings_modal/cookies_tab.rs`: Thuộc tính cookie (`Secure`, `HttpOnly`, `SameSite`, `domain`, `path`) chuyển sang `Tag::success`, `Tag::info`, `Tag::secondary` tiêu chuẩn.
- **`Badge` & Tab Counters**:
  - `tabs_with_badge.rs`: Chuẩn hóa đếm số lượng `[headers]`, `[cookies]`, `[console]` trên thanh tab.
- **`Spinner` (`gpui_kit::component::spinner::Spinner`)**:
  - `sidebar/request_node.rs`: Đã loại bỏ spinner theo yêu cầu tinh gọn sidebar, tránh giật và dư thừa spinner.
  - `response_inspector.rs`:
    - Thanh status bar phản hồi: Chuyển đổi sang `Spinner::new().with_size(px(12.)).color(t.primary)` với animation `IconName::Loader` mượt mà và hiện đại.
    - Trung tâm response view: Đã loại bỏ spinner vòng xoay to 56x56px dư thừa, tinh gọn giao diện với sự kết hợp hài hòa giữa `Progress` và `Skeleton`.
    - Các sub-tabs (Request sent, Headers, Cookies, Console): Đã cập nhật sang `Spinner::new()` hiện đại thay thế icon thô cũ.
- **`Progress` (`gpui_kit::component::progress::Progress`)**:
  - `response_inspector.rs`: Tích hợp thanh tiến trình ngang `Progress::new("response-send-progress").loading(true)` với hiệu ứng indeterminate sliding animation khi request đang gửi tới server.
- **`Skeleton` (`gpui_kit::component::skeleton::Skeleton`)**:
  - `response_inspector.rs`: Tích hợp placeholder shimmer lines mô phỏng dữ liệu phản hồi đang được tải về thay thế cho spinner to.
  - `git/history_modal.rs`: Tích hợp placeholder shimmer rows cho danh sách commit khi dữ liệu lịch sử git đang tải.
- **`Alert` & `Empty`**:
  - Đã hoàn thành từ Giai đoạn 1 và 2 (`git/common.rs`, `import_modal.rs`, `cookies_tab.rs`, `console_panel.rs`, `diff_view.rs`, `body_editor/mod.rs`).

### 2.7 Giai đoạn 7: Hoàn thiện toàn bộ Nhóm D (Overlays, Modals & Menus) (Hoàn thành)
- **`Dialog` & Semantic Overlay System (`ModalDialog`)**:
  - Chuẩn hóa toàn bộ 8 modal trong toàn bộ ứng dụng sang `ModalDialog` (tích hợp `DialogHeader`, `DialogTitle`, `DialogDescription`, `DialogContent`, `DialogFooter`):
    - `SettingsModal` (`settings_modal/mod.rs`)
    - `EnvironmentModal` (`env_modal.rs`)
    - `ImportModal` (`import_modal.rs`)
    - `ScopeSettingsModal` (`scope_settings_modal.rs`)
    - `GitCommitModal` (`git/commit_modal.rs`)
    - `GitBranchModal` (`git/branch_modal.rs`)
    - `GitHistoryModal` (`git/history_modal.rs`)
    - `GitRemotesModal` (`git/remotes_modal.rs`)
  - Tự động hóa hoàn toàn backdrop dimming overlay, đóng modal qua phím `Escape`, đóng modal khi click ra ngoài backdrop, occlude sự kiện chuột và bóng đổ macOS chuẩn.
- **`Dropdown & Context Menu` (`ContextMenuExt`, `PopupMenuItem`)**:
  - Chuẩn hóa 100% menu chuột phải trên Sidebar:
    - `request_node.rs`: Thêm đầy đủ icons cho từng action (Copy as YAML `IconName::FileCode`, Copy as cURL `IconName::Terminal`, Pin/Unpin `IconName::Pin`, Rename `IconName::Pencil`, Duplicate `IconName::Copy`, Discard Changes `IconName::Undo2`, Delete `IconName::Trash`).
    - `folder_node.rs`: Thêm icon cho New Request (`IconName::Plus`), New Folder (`IconName::FolderPlus`), Rename (`IconName::Pencil`), Folder Settings (`IconName::Settings`), Delete (`IconName::Trash`).
    - `example_node.rs`: Thêm icon cho Rename (`IconName::Pencil`), Duplicate (`IconName::Copy`), Move Up (`IconName::ArrowUp`), Move Down (`IconName::ArrowDown`), Delete (`IconName::Trash`).
- **`Tooltip Unification` (`gpui_kit::component::tooltip::Tooltip`)**:
  - `titlebar.rs`:
    - `btn-workspace-switcher` được gắn `.tooltip("Workspace Settings & Actions")`.
    - `btn-env-breadcrumb` được gắn `.tooltip("Select Active Environment")`.
    - `btn-split-toggle` chuyển sang `Button::new("btn-split-toggle").ghost().compact()` với tooltip dynamic (`"Split Layout: Stacked (Vertical)"` hoặc `"Split Layout: Side-by-Side (Horizontal)"`).
    - `btn-settings` chuyển sang `Button::new("btn-settings").ghost().compact()` với tooltip `"Settings (⌘,)"`.
  - Sidebar & footer: `Button::new("sidebar-footer-branch-chip")` và `btn-sidebar-new` đều có tooltip rõ ràng.
- **`Command Palette`**:
  - Chuẩn hóa các phím tắt `Kbd` (`cmd-enter`, `cmd-,`, `cmd-shift-g`, `cmd-i`, `escape`), search input, và sử dụng `Separator::horizontal()` phân cách trang nhã các nhóm: Action commands, Switch Request, và Switch Environment.

### 2.8 Giai đoạn 8: Hoàn thiện toàn bộ Nhóm E (Data Display & Content) (Hoàn thành)
- **`Separator / Divider` (`gpui_kit::component::separator::Separator`)**:
  - `settings_modal/mod.rs`: Tích hợp `Separator::vertical()` phân chia sidebar menu và nội dung cài đặt.
  - `git/commit_modal.rs`: Tích hợp `Separator::vertical()` giữa file changes list và diff viewer, `Separator::horizontal()` trước ô nhập commit message.
  - `git/history_modal.rs`: Tích hợp `Separator::vertical()` giữa danh sách commit và diff details.
  - `command_palette.rs`: Tích hợp `Separator::horizontal()` giữa các phân mục tìm kiếm.
  - `scope_settings_modal.rs` & `request_bar.rs`: Đồng bộ hóa toàn bộ đường kẻ ngang/dọc sang component `Separator`.
- **`Kbd` (Phím tắt macOS / Windows)**:
  - Hoàn thành đầy đủ trong `settings_modal/shortcuts_tab.rs` và `command_palette.rs`.
- **`Git Diff View`**:
  - Hỗ trợ đầy đủ Split Diff và Unified Diff với nút toggle chuyển đổi trực quan, tooltip, và parser hunks được bảo vệ bởi 100% test cases.
- **`Table` & Key-Value Components**:
  - `kv_table.rs` và `multipart.rs`: Hệ thống bảng tương tác với input pool theo row index, autocomplete tokenization, checkbox 3 trạng thái, và nút delete với hover tint đỏ êm ái.

---

## 3. Tổng kết & Tình trạng Hoàn thiện

Toàn bộ **24/24 UI Components** thuộc cả 5 Nhóm A, B, C, D, E đã được migrate và chuẩn hóa đồng bộ sang tiêu chuẩn của `gpui-kit`:
- **Nhóm A (Form & Interactive Controls)**: Checkbox, Switch, Button, Delete, Line Wrap, Radio, Select/Dropdown, Timeout Selector (100% Hoàn thành).
- **Nhóm B (Navigation & Layout)**: Tab/TabBar, Breadcrumb, Collapsible, Resizable Panels (100% Hoàn thành).
- **Nhóm C (Feedback, Status & Loading)**: Alert, Empty States, Tag, Badge, Notification/Toast, Progress, Spinner, Skeleton (100% Hoàn thành).
- **Nhóm D (Overlays, Modals & Menus)**: Dialog/Modals, Command Palette, Context Menu, Tooltip Unification (100% Hoàn thành).
- **Nhóm E (Data Display & Content)**: Kbd, Separator/Divider, Git Diff View, Table (100% Hoàn thành).

---

## 4. Lộ trình phân kỳ tổng thể (Comprehensive Roadmap)

```
[Hoàn thành] Giai đoạn 1: Form Controls & Tabs (Checkbox, Switch, Kbd, TabBar+Tab)
       ↓
[Hoàn thành] Giai đoạn 2: Feedback & Empty States (Alert, Empty, Row Actions, Delete & Wrap buttons)
       ↓
[Hoàn thành] Giai đoạn 3: Command Palette & Modal Buttons (Kbd hints, Dialog Buttons)
       ↓
[Hoàn thành] Giai đoạn 4: Select, DropdownMenu & Radio (Method, Env, Auth, Body format selectors & Theme Radio)
       ↓
[Hoàn thành] Giai đoạn 5: Navigation & Breadcrumb (Breadcrumb Smart Truncation, Collapsible, Resizable Panels)
       ↓
[Hoàn thành] Giai đoạn 6: Feedback, Status & Loading (Tag, Badge, Spinner, Progress, Skeleton Shimmer)
       ↓
[Hoàn thành] Giai đoạn 7: Overlays, Modals & Menus (ModalDialog 8 Modals, Context Menus, Tooltips, Command Palette)
       ↓
[Hoàn thành] Giai đoạn 8: Data Display & Separator System (Separator horizontal/vertical, Git Diff, Table)
```

---

## 5. Quy tắc Kiểm định & Tiêu chí Chất lượng (Verification Checklist)

Mọi thay đổi trong quá trình migration phải đáp ứng nghiêm ngặt 4 tiêu chuẩn bắt buộc:
1. **Định dạng code**: `cargo fmt --check` không có khác biệt.
2. **Clippy tĩnh**: `cargo clippy --all-targets -- -D warnings` đạt **0 cảnh báo**.
3. **Độ ổn định & Kiểm thử**: `cargo test` pass **100% test cases** (188/188 tests).
4. **Tránh Panic Hover**: Không bao giờ gọi trực tiếp `.hover(...)` trên `Button` của `gpui-kit` vì `Button::render` đã tự đăng ký hover nội bộ.
