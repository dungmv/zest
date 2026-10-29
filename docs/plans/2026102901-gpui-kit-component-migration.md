# Plan: Migrate các custom UI component sang gpui-kit standard components

- **Ngày**: 2026-10-29
- **Trạng thái**: Hoàn thành Giai đoạn 1, 2 & 3 (Phase 1, 2 & 3 Completed)
- **Phạm vi**: 
  - Giai đoạn 1: `crates/app/src/components/` (`git/common.rs`, `import_modal.rs`, `kv_table.rs`, `body_editor/multipart.rs`, `settings_modal/network_tab.rs`, `settings_modal/shortcuts_tab.rs`, `tabs_with_badge.rs`, `app/request_composer.rs`, `components/response_inspector.rs`)
  - Giai đoạn 2: `crates/app/src/components/` (`git/common.rs`, `import_modal.rs`, `settings_modal/cookies_tab.rs`, `console_panel.rs`, `git/diff_view.rs`)
  - Giai đoạn 3: `crates/app/src/components/` (`command_palette.rs`, `import_modal.rs`, `git/commit_modal.rs`, `git/branch_modal.rs`, `git/history_modal.rs`, `git/remotes_modal.rs`)

---

## 1. Goal

Thay thế các component UI tự viết lặp lại (reinventing the wheel) như `Checkbox`, `Switch`, `Kbd`, `TabItem` (Tabs) bằng các component chuẩn từ `gpui-kit` (`gpui_kit::component::*`). Mục tiêu:
- Loại bỏ mã nguồn dư thừa, loại bỏ trùng lặp logic render và sự kiện.
- Chuẩn hóa animation (spring), tương tác bàn phím (tab navigation, space/enter), accessibility (screen reader) và giao diện đồng nhất với theme.
- Giữ 100% độ ổn định và tính tương thích của ứng dụng, vượt qua toàn bộ test suite và `clippy -D warnings`.

---

## 2. Context

Trong quá trình phát triển ban đầu của `zest`, nhiều UI element được tạo thủ công bằng các khối `div()` với inline styling:
1. **`Checkbox`**: Được triển khai độc lập ở 4 vị trí:
   - `crates/app/src/components/git/common.rs` (`render_checkbox`): vẽ div với ký tự `"✓"` hoặc `"−"`.
   - `crates/app/src/components/import_modal.rs` (`render_checkbox`): vẽ div với ký tự `"✓"`.
   - `crates/app/src/components/kv_table.rs`: vẽ div checkbox để toggle enable/disable row.
   - `crates/app/src/components/body_editor/multipart.rs`: vẽ div checkbox để toggle multipart form field.
2. **`Switch`**:
   - `crates/app/src/components/settings_modal/network_tab.rs`: tự vẽ các nút toggle cho `Follow HTTP Redirects` và `Validate TLS Certificates` bằng `div()` đổi background khi bật/tắt.
3. **`Kbd`**:
   - `crates/app/src/components/settings_modal/shortcuts_tab.rs`: tự vẽ khung chữ nhật phím tắt với monospace font.
   - `crates/app/src/components/command_palette.rs`: tự vẽ shortcut hint badge.
4. **`TabItem` (Tabs)**:
   - `crates/app/src/components/tabs_with_badge.rs`: tự viết `TabItem` với `border_b_2` cho active tab.

Trong khi đó, `gpui-kit` (phiên bản `0.8.1`, tích hợp sẵn `gpui-component`) cung cấp đầy đủ các component đã được tối ưu hóa, hỗ trợ tương tác bàn phím và accessibility chuẩn macOS:
- `gpui_kit::component::checkbox::Checkbox`
- `gpui_kit::component::switch::Switch`
- `gpui_kit::component::kbd::Kbd`
- `gpui_kit::component::tab::{Tab, TabBar, TabVariant}`

---

## 3. Architectural Decisions

1. **Phân kỳ thực thi an toàn (Phased Rollout)**:
   - **Giai đoạn 1 (Đợt này - Quick Wins & High ROI)**:
     - Migrate `render_checkbox` trong `git/common.rs` và `import_modal.rs`.
     - Migrate Checkbox trong `kv_table.rs` và `body_editor/multipart.rs`.
     - Migrate Switch trong `settings_modal/network_tab.rs`.
     - Migrate Kbd trong `settings_modal/shortcuts_tab.rs`.
     - Chuẩn hóa `TabItem` trong `tabs_with_badge.rs` với styling và semantic của `gpui-kit` Tab.
   - **Giai đoạn tiếp theo (Tách riêng để giảm rủi ro)**:
     - Overlays & Dialogs (`SettingsModal`, `EnvModal`, `ImportModal` sang `gpui_kit::component::dialog::Dialog`).
     - Command Palette (`gpui_kit::component::command::Command`).
     - Git Diff View (`gpui_kit::component::diff::Diff`).

2. **Duy trì API Interface & Callback Signature**:
   - `render_checkbox` giữ signature tiện dụng nhưng phần ruột delegate sang `gpui_kit::component::checkbox::Checkbox`.
   - Các handler callback `on_click` / `on_change` cập nhật state đúng mô hình single-directional data flow của `zest`.
   - Đối với switch, chuyển callback toggle sang `on_click` của `Switch`.

3. **Bảo tồn Theme & Visual Styling**:
   - Tận dụng `cx.theme()` tự động từ `gpui-kit` kết hợp với token màu sắc hiện tại của `theme()` trong `zest`.

---

## 4. Step-by-Step Implementation Tasks

### Nhiệm vụ 1: Migrate Checkbox
- **`crates/app/src/components/git/common.rs`**:
  - Cập nhật `render_checkbox`: Sử dụng `gpui_kit::component::checkbox::Checkbox`.
  - Hỗ trợ cả trạng thái `checked`, `partial` (indeterminate) và callback click.
- **`crates/app/src/components/import_modal.rs`**:
  - Cập nhật `render_checkbox`: Dùng `Checkbox::new(id).checked(checked).label(label).on_click(...)`.
- **`crates/app/src/components/kv_table.rs`**:
  - Thay thế khối div checkbox thủ công bằng `Checkbox::new(...).checked(enabled).on_click(...)`.
- **`crates/app/src/components/body_editor/multipart.rs`**:
  - Thay thế khối div checkbox thủ công bằng `Checkbox::new(...).checked(enabled).on_click(...)`.

### Nhiệm vụ 2: Migrate Switch (Toggle)
- **`crates/app/src/components/settings_modal/network_tab.rs`**:
  - Thay thế `#toggle-follow-redirects` bằng `Switch::new("switch-follow-redirects").checked(cur_follow).on_click(...)`.
  - Thay thế `#toggle-validate-tls` bằng `Switch::new("switch-validate-tls").checked(cur_tls).on_click(...)`.

### Nhiệm vụ 3: Migrate Kbd (Keyboard Shortcut)
- **`crates/app/src/components/settings_modal/shortcuts_tab.rs`**:
  - Import `gpui_kit::component::kbd::Kbd`.
  - Thay thế div badge thủ công bằng `Kbd::new(Keystroke::parse(...))` hoặc hiển thị chuẩn.

### Nhiệm vụ 4: Combo TabBar & Tab
- Chuyển đổi thanh tab của Request Composer (`request_composer.rs`) và Response Inspector (`response_inspector.rs`) sang `TabBar::new(...).underline()` + `Tab::new()`.
- Tự động hóa khoảng cách (`gap: 16px`), sliding indicator animation và border đáy.
- Hoàn thành và vượt qua toàn bộ test suite.

---

## 4.2 Giai đoạn 2: Feedback (Alert) & Empty States (Empty)

### Nhiệm vụ 5: Migrate Alert (Error Banners & Parse Errors)
- **`crates/app/src/components/git/common.rs`**:
  - Thay thế `render_error_banner` bằng `gpui_kit::component::alert::Alert::error("git-error-banner", format!("Git: {}", err.trim())).banner().on_close(...)`.
  - Tự động nâng cấp giao diện thông báo lỗi cho 4 git modal: `branch_modal.rs`, `commit_modal.rs`, `history_modal.rs`, `remotes_modal.rs`.
- **`crates/app/src/components/import_modal.rs`**:
  - Thay thế khối div hiển thị lỗi `Parsing Error` bằng `Alert::error("import-parse-error", format!("Parsing Error: {}", err))`.

### Nhiệm vụ 6: Migrate Empty States & Actions
- **`crates/app/src/components/git/common.rs` (`render_not_a_repo_view`)**:
  - Chuyển sang `Empty::new().header(...).content(...)` với `Button::new("btn-git-init").primary().loading(is_loading)`.
- **`crates/app/src/components/settings_modal/cookies_tab.rs`**:
  - Thay thế khối div empty state bằng `Empty::new().header(...)` với icon `Cookie`, tiêu đề và mô tả giải thích.
- **`crates/app/src/components/console_panel.rs`**:
  - Thay thế `render_empty_state` bằng `Empty::new().header(...).content(...)` chứa code snippet mẫu.
- **`crates/app/src/components/git/diff_view.rs`**:
  - Thay thế các empty div hiển thị khi chưa chọn file hoặc file không có thay đổi bằng `Empty::new().header(...)`.
- **`crates/app/src/components/body_editor/mod.rs`**:
  - Thay thế khối div empty state khi `RequestBody::None` bằng `Empty::new().header(...)` với icon `FileText`.

### Nhiệm vụ 7: Migrate Action Buttons (Button)
- **`crates/app/src/components/kv_table.rs`**:
  - Thay thế nút `+ Add Row` bằng `Button::new(...).small().icon(IconName::Plus).label("Add Row")`.
  - Nút xóa hàng giữ nguyên kích thước chuẩn `sm` (24x24px, icon 14px), style là ghost với nền trong suốt và màu chữ/icon `t.text_muted`. Khi hover chuyển sang màu đỏ nhạt mềm dịu (`t.json_null` - coral/rose red, tránh màu đỏ rực) kèm hiệu ứng nền `t.json_null.opacity(0.12)` và `Tooltip::new("Delete row")`.
- **`crates/app/src/components/body_editor/mod.rs`**:
  - Chuyển `btn-format-json-body`, `btn-format-xml-body`, `btn-add-multipart-part`, `btn-add-form-row` sang `Button::new(...).ghost().xsmall()`.
- **`crates/app/src/components/body_editor/multipart.rs`**:
  - Nút xóa hàng multipart đồng bộ với KV Table: kích thước chuẩn `sm` (24x24px, icon 14px), style ghost bình thường màu `t.text_muted`, hover chuyển sang đỏ mềm dịu `t.json_null` với tooltip "Delete field".
- **`crates/app/src/components/response_inspector.rs`**:
  - Nút Line Wrap (`btn-toggle-wrap`): Khi disable hiển thị dưới dạng ghost button tiêu chuẩn (`btn.ghost()`); khi enable hiển thị nổi bật với màu primary (`btn.primary()`). Hiển thị rõ ràng 100% trên cả Dark và Light mode.

---

## 4.3 Giai đoạn 3: Command Palette & Modals Buttons (Đã hoàn thành)

### Nhiệm vụ 8: Chuẩn hóa Kbd trong Command Palette
- **`crates/app/src/components/command_palette.rs`**:
  - Thay thế các chuỗi shortcut thô (`Ctrl+Enter`, `Ctrl+,`, `Ctrl+Shift+G`, `Ctrl+I`) bằng `Kbd::new(Keystroke::parse(...))` chuẩn hóa glyph nền tảng macOS / Windows.
  - Nút thoát ESC dùng `Kbd::new` thay vì nhãn text thô.
  - Thay thế emoji `"📜"` bằng icon `IconName::GitCommitHorizontal`.

### Nhiệm vụ 9: Migrate Action & Close Buttons trong Modals
- **`crates/app/src/components/import_modal.rs`**:
  - `btn-close-import-modal`: Dùng `Button::new(...).ghost().xsmall().icon(IconName::Close)`.
  - `btn-cancel-import`: Dùng `Button::new(...).ghost().small().label("Cancel")`.
  - `btn-execute-import`: Dùng `Button::new(...).primary().small().disabled(!is_valid).label("Import Now")`.
- **`crates/app/src/components/git/commit_modal.rs`**:
  - `btn-commit-action`: Dùng `Button::new(...).primary().small().disabled(!can_commit || is_loading).loading(is_loading)`. Tận dụng spinner loading native của `Button`.
  - `btn-git-commit-close`: Dùng `Button::new(...).ghost().xsmall().icon(IconName::Close)`.
- **`crates/app/src/components/git/branch_modal.rs`**:
  - `btn-cancel-create-branch` & `btn-confirm-create-branch`: Chuyển sang `Button` với loading indicator.
  - `btn-git-branch-close`: Dùng `Button::new(...).ghost().xsmall().icon(IconName::Close)`.
- **`crates/app/src/components/git/history_modal.rs`**:
  - `btn-git-history-close`: Dùng `Button::new(...).ghost().xsmall().icon(IconName::Close)`.
- **`crates/app/src/components/git/remotes_modal.rs`**:
  - `btn-add-remote-action`: Dùng `Button::new(...).primary().small().loading(is_loading)`.
  - `btn-git-remotes-close`: Dùng `Button::new(...).ghost().xsmall().icon(IconName::Close)`.

---

## 5. Risk Mitigation

- **Stop Propagation trên Checkbox**: Checkbox trong hàng bảng (`kv_table`, `multipart`) cần ngăn việc click kích hoạt selection của row cha nếu có. `Checkbox` của `gpui-kit` xử lý click event riêng, nhưng cần kiểm tra đảm bảo không kích hoạt select nhầm.
- **Controlled vs Uncontrolled**: Toàn bộ UI controls trong `zest` tuân thủ controlled state (trạng thái đọc từ GPUI State entity, callback mutate state và emit/notify để re-render). `Checkbox::on_click` và `Switch::on_click` nhận giá trị mới từ tương tác và cập nhật state chính xác.

---

## 6. Verification Steps

1. `cargo test` — đảm bảo toàn bộ unit test và integration test pass 100%.
2. `cargo clippy --all-targets -- -D warnings` — đảm bảo 0 warning.
3. Kiểm tra trực quan các màn hình: Settings Modal (Network tab toggle switch, Shortcuts tab kbd), Git commit checkbox, KV table checkbox, Multipart form checkbox.
