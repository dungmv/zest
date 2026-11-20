# 2026112101 — Modal Registry API + `components/modals/` Umbrella

## Goal

Hoàn thiện việc quản lý modal sau refactor `51fa08e` (centralize modal state) bằng 2 bước:

1. **Registry API generic** thay ~25 method `open_x/close_x/toggle_x` copy-paste trong `state/modal_state.rs` (456 LOC → ~300 LOC, 79 call sites đổi tên method).
2. **Umbrella `components/modals/`** gom 8 modal component rải rác vào 1 thư mục (di chuyển verbatim, không restyle).

## Context

- `ModalState` (`crates/app/src/state/modal_state.rs`) đã có `active_modal: Option<ModalKind>` tốt, nhưng ~25 method open/close/toggle gần như copy-paste (chỉ khác kind + field reset), và `dismiss_top_modal` phải match 7 nhánh.
- Modal component nằm rải rác: `env_modal/`, `settings_modal/`, `workspace_setting_modal/`, `import_modal/`, `export_modal/`, `about_modal.rs`, `command_palette.rs`, `modal_dialog.rs` trong `components/`.
- Ràng buộc: `EnvModalSelectionChanged` (variant duy nhất của `ModalEvent`) được subscribe bởi `EnvVarTable::new` (`env_modal/var_table.rs:67-78`) — mọi đường open Environment phải vẫn emit event này. Git modal (`git_state.modal.is_open`) là cơ chế độc lập — **không đụng**.
- Semantics field (đã map đủ qua explore):
  - Transient (reset khi open & close): `env_modal.target_variable`, `import_modal.target_folder_rel`, `export_modal.target_folder_id/name`, `workspace_setting_target`.
  - Sticky (sống qua close): `env_modal.selected_id`, `settings_tab`, `cookie_filter_query`, `update_status`.
  - Dead: `env_modal.filter_query` + `set_env_filter_query` (0 reader/writer) → xóa.

## Architectural Decisions

1. **3 method generic + 1 payload enum** thay toàn bộ per-modal open/close/toggle:

   ```rust
   pub enum ModalPayload {
       Env { env_id: Option<String>, target_variable: Option<String> },
       WorkspaceSetting(WorkspaceSettingTarget),
       Import { target_folder_rel: Option<String> },
       Export { folder: Option<(String, String)> },
   }
   impl ModalState {
       pub fn open(&mut self, kind: ModalKind, payload: Option<ModalPayload>, cx: &mut Context<Self>);
       pub fn close(&mut self, kind: ModalKind, cx: &mut Context<Self>);
       pub fn toggle(&mut self, kind: ModalKind, cx: &mut Context<Self>);
   }
   ```

   - `open`: set `active_modal = Some(kind)` → reset transient fields của kind → apply payload (nếu có) → nếu kind == Environment thì emit `EnvModalSelectionChanged`. `payload: None` = open thường (giữ sticky state).
   - `close(kind)`: chỉ đóng nếu `active_modal == Some(kind)` + reset transient của kind (giữ đúng behavior hiện tại).
   - `toggle(kind)`: active == kind → close, else open.
   - `dismiss_top_modal` viết lại qua `close(self.active_modal?)`; giữ `has_any_modal_open`.
2. **Giữ nguyên**: `select_modal_environment`, `set_settings_tab`, `set_cookie_filter_query`, `check_for_updates`, theme/network/split-layout setters, toàn bộ GitModal.
3. **Xóa** (0 external callers): `toggle_env_modal`, `open_env_modal_with_id`, `set_env_filter_query`, `open_settings_tab`, field `env_modal.filter_query`.
4. **Chuyển `open_browser_url`** (modal_state.rs:447) → `components/about_modal.rs` (caller duy nhất: about_modal.rs:387) thành private fn.
5. **Umbrella mirror style `git/mod.rs`**: `modals/mod.rs` = `pub mod` leaves + `pub use leaf::*` globs; `components/mod.rs` thay 8 cặp `pub mod/pub use` modal bằng `pub mod modals;` + `pub use modals::*;`. Hai tầng glob giữ nguyên surface `crate::components::ModalDialog`, `crate::components::EnvVarTable`, `crate::components::env_modal`, ... → consumer (`app/mod.rs`, `app/modals.rs` dùng glob `crate::components::*`) và `git/*` import `modal_dialog` không cần sửa import.
6. Không đụng theme/network/split-layout state trong `ModalState` (nợ cũ, tách riêng dịp khác). `env_modal/var_table.rs` 676 LOC vượt quota là nợ có sẵn — di chuyển verbatim, không sửa trong refactor này.
7. `ModalPayload::Import { target_folder_rel: None }` được dùng tường minh ở `titlebar/menus.rs` + `app/mod.rs` (thay vì payload `None`) để variant luôn được construct trong code production, tránh dead_code warning trên bin target (CI deny warnings).

## Step-by-Step Implementation

### Step 1 — Registry API (1 commit)

1. `state/modal_state.rs`: thêm `ModalPayload`, `open/close/toggle` + `reset_transient_ui`/`apply_payload` (kèm comment bảng transient/sticky), viết lại `dismiss_top_modal`; xóa ~25 method per-modal + `filter_query` + `set_env_filter_query`; chuyển `open_browser_url` ra `about_modal.rs`.
2. Migrate call sites (compiler-guided):
   - Toggle: `app/mod.rs:388,491` (CommandPalette), `:404` + `titlebar/mod.rs:268` (Settings), `:417` (About).
   - Open: `platform/menu.rs:45,51` (About, Settings); `titlebar/menus.rs:63,265` (About, Environment-None), `:73` (WorkspaceSetting::Workspace), `:100` (Import payload None), `:109` (Export None); `sidebar/folder_node.rs:404` (WorkspaceSetting::Folder), `:420` (Export folder payload); `env_input.rs:193`, `env_autocomplete/mod.rs:357` (Environment None), `:371` (Environment payload target_variable); `app/mod.rs:428` (Import payload None).
   - Close: `env_modal/mod.rs:81`, `settings_modal/mod.rs:83`, `workspace_setting_modal/mod.rs:183`, `import_modal/mod.rs:136,353,391`, `export_modal/mod.rs:105,283,339`, `about_modal.rs:58,213`, `command_palette.rs` ×7, `settings_modal/about_tab.rs:131-132` (close Settings + open About).
   - Tests: `state/workspace_data_tests.rs`, `env_modal/var_table.rs` tests (:660,667 → payload Env).
3. Gate: `cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings` → commit.

### Step 2 — Umbrella move (1 commit)

1. `git mv` vào `components/modals/`: `env_modal/` (4 file), `settings_modal/` (6), `workspace_setting_modal/` (5), `import_modal/` (2), `export_modal/` (1), `about_modal.rs`, `command_palette.rs`, `modal_dialog.rs`. Nội dung verbatim 100%.
2. Tạo `components/modals/mod.rs` theo style `git/mod.rs` (pub mod + pub use glob từng leaf).
3. `components/mod.rs`: thay 8 cặp dòng modal bằng `pub mod modals;` + `pub use modals::*;`.
4. Sửa path chỉ khi compiler báo (dự kiến ~0 nhờ hai tầng glob).
5. Gate như trên → commit.

## Risk Mitigation

- **Mất emit `EnvModalSelectionChanged`** → var_table không rebuild/focus: emit giữ ở mọi đường open Environment + `select_modal_environment` không đổi; test `test_env_var_table_event_driven_sync` và `test_modal_selection_and_target_variable` chặn.
- **Semantics transient/sticky lệch** (tab settings, env selection phải sống qua close): bảng reset ghi rõ bằng comment trong `open/close` + tests assert hành vi.
- **Glob hai tầng lỗi re-export**: compile bắt ngay; fallback thêm `pub use modals::env_modal::*;`-style tường minh vào `components/mod.rs`.
- **Rename-input DismissModal interception** (`env_list.rs:301` v.v.): `close(kind)` giữ nguyên semantics.
- **Restyle trong lúc move**: chỉ `git mv`, kiểm `git diff --stat -M` bảo đảm không có thay đổi nội dung.

## Verification

1. `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings` — 0 warning sau mỗi step.
2. Smoke test thủ công (`cargo run`): mỗi modal mở/đóng đúng qua đúng trigger — ctrl-k palette, ctrl-, settings, ctrl-i import, click token `{{var}}` → env modal focus đúng biến, folder context menu → Folder Settings / Export OpenAPI (scope folder), About từ Settings→About tab, Escape dismiss, Ctrl+Enter bị chặn khi modal mở.
3. UI không đổi pixel-wise (không restyle — lesson TD-202).

## Follow-up (cùng ngày, user request)

Chuyển umbrella từ `components/modals/` lên **`src/modals/`** (đồng cấp với `components/`):

1. `git mv crates/app/src/components/modals crates/app/src/modals` — nội dung verbatim.
2. `main.rs` khai báo `mod modals;` (top-level, style private như các mod khác); `components/mod.rs` bỏ 2 dòng `pub mod modals;` + `pub use modals::*;`.
3. Retarget import của consumer: `app/mod.rs` + `app/modals.rs` thêm `use crate::modals::*;` (bên cạnh glob components cho GitModal); 8 file đổi `crate::components::ModalDialog` → `crate::modals::modal_dialog::ModalDialog` (gồm 4 file git modals); `about_tab.rs` đổi `crate::modals::about_modal::render_update_section`.
4. 2 `include_bytes!` lùi lại 1 cấp `../` (path tính từ vị trí file mới).
5. `modals/mod.rs`: bỏ glob `pub use modal_dialog::*;` (dead — mọi consumer dùng full path sau retarget; cùng hiện tượng dead-glob như bước 2).
6. Gate như trên → commit.
7. **Follow-up 2**: đổi `modal_dialog.rs` → `modal.rs`, struct `ModalDialog` → `Dialog` (tên `modals::modal::ModalDialog` lặp 3 lần "modal"); `DialogHeader/DialogTitle/DialogContent/DialogDescription` giữ nguyên (re-export từ `gpui_kit::component::dialog`). Lưu ý: `gpui_component::dialog::Dialog` tồn tại nhưng không file nào import trực tiếp — nếu sau này cần thì alias `as GpuiDialog`.
