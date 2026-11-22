# 2026112201 — Modal Package Cleanup (P1–P4)

## Goal

Cải tiến package `crates/app/src/modals/` sau đợt audit 2026-11-22: dọn dead code, giảm
boilerplate lặp, tuân thủ ngưỡng 650 LOC, tách service khỏi state, và chuẩn hóa checklist
khi thêm modal mới. Không thay đổi behavior hay restyle UI.

## Context

- Đợt refactor trước (2026112101-modal-registry-and-umbrella) đã đưa registry API
  `ModalState::open/close/toggle(kind, ModalPayload)` vào `state/modal_state.rs`, gom
  components vào `src/modals/` umbrella, và đổi `Dialog` → `Modal` (2026-11-22).
- Audit phát hiện: hai cơ chế ESC song song (raw `on_key_down` trong `Modal` shell +
  global `escape → DismissModal` action); tham số chết `_send_request_cb`; 6 modal lặp
  cùng closure `on_close`; icon loader trùng nhau ở About modal/tab; 4 setter lặp
  `AppSettings::load/mutate/save`; `var_table.rs` 684 LOC vượt ngưỡng; logic updater
  (spawn tokio) nằm trong `ModalState`.

## Architectural Decisions

1. **Một cơ chế ESC duy nhất**: giữ global `DismissModal` action (đã cover GitModal),
   xóa nhánh ESC raw `on_key_down` trong `Modal` shell. Căn cứ: global action ăn ESC
   trước raw key handler (đã verify trong investigation trước, xem memory
   gpui-escape-action-intercept). Cần smoke test thủ công sau khi xóa.
2. **`Modal::for_kind(id, kind, &Entity<ModalState>)`**: builder tiện ích pre-wire
   `on_close` qua registry. Giữ `Modal::new` + `.on_close` cho GitModal (quản lý bởi
   `GitState`, không thuộc registry).
3. **Service tách khỏi state**: `check_for_updates` chuyển sang
   `services/update_service.rs` (Runner-style như `GitService` — struct giữ
   `Entity<ModalState>`, spawn tokio từ `&mut App`). `UpdateStatus` vẫn là state, nằm ở
   `ModalState`.
4. **Dedupe primitives, không gộp layout**: About modal và Settings About tab giữ hai
   layout riêng (modal = hero centered, tab = sections); chỉ share icon loader và row
   helper. Tuân thủ nguyên tắc "refactor không restyle UI".
5. **Split theo umbrella**: `var_table.rs` → `var_table/mod.rs` (entity + sync logic +
   tests) + `var_table/rows.rs` (render element tree, move verbatim). Module con là
   descendant nên truy cập được private fields của struct định nghĩa ở cha.
6. **`ModalState` giữ theme/network prefs** theo đúng định nghĩa AGENTS.md §2.1; chỉ
   thêm helper `persist_setting` để gọn 4 setter.

## Step-by-Step Implementation Tasks

1. **P1.2 + P4.10 — Router cleanup**: bỏ `_send_request_cb` khỏi
   `attach_modal_overlays` + call site; bỏ block clone eager 8 entity, clone theo arm.
2. **P1.1 — ESC path**: xóa `close_cb_escape` + `on_key_down` ESC trong `modal.rs`,
   cập nhật doc comment.
3. **P2.3 — `Modal::for_kind`**: thêm constructor; đổi 6 modal registry
   (env/settings/workspace_setting/import/export/about) sang `for_kind`, dọn import
   `ModalKind` thừa.
4. **P2.4 + P4.9 — About dedupe**: gộp `about_app_icon` (1 loader dùng chung), gộp
   `render_meta_row`/`render_spec_row` thành `render_info_row` pub ở `about_modal.rs`;
   xóa bản copy ở `about_tab.rs` (kèm `#[derive(IntoElement)]` sai vị trí trên fn).
5. **P2.5 — `persist_setting`**: helper load/mutate/save dùng chung cho 4 setter.
6. **P4.8 — `UpdateService`**: tạo `services/update_service.rs`, chuyển
   `check_for_updates`; 3 call site trong `render_update_section` gọi service; xóa
   method khỏi `ModalState`.
7. **P3.6 — Split `var_table`**: tạo `var_table/mod.rs` + `var_table/rows.rs`, move
   render body verbatim; tests giữ nguyên ở mod.rs.
8. **P3.7 — Checklist**: doc comment trên `ModalKind` liệt kê 6 touch points khi thêm
   modal mới; bổ sung mục tương tự vào `.agents/skills/gpui-architecture/SKILL.md`.
9. Verify + commit theo từng đơn vị logic.

## Risk Mitigation

- ESC: nếu smoke test phát hiện ESC trong modal không còn hoạt động (global action bị
  input nào nuốt), khôi phục nhánh ESC trong `Modal` shell.
- Split `var_table`: render body move nguyên văn, không đụng logic sync; test
  `test_env_var_table_event_driven_sync` phải vẫn pass.
- UpdateService: giữ nguyên guard `Checking` + oneshot channel; `App::spawn` closure
  single-param (gpui-kit 0.10), entity capture thay cho `this` của `Context::spawn`.
- Không restyle: mọi style call giữ nguyên khi move.

## Verification Steps

```bash
cargo fmt --check
cargo test          # gồm test EnvVarTable hiện hữu
cargo clippy --all-targets -- -D warnings
```

Smoke test thủ công: mở từng modal (env/settings/workspace/import/export/about,
command palette, git modals) → ESC đóng modal; backdrop click + nút X vẫn đóng.
