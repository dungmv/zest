# Implementation Plan: Cycle 2 Codebase Audit Remediation & Modularization

- **Date**: 2026-11-12
- **Plan File**: `docs/plans/2026111202-codebase-audit-and-modularization-roadmap.md`
- **Status**: ✅ Completed (2026-11-13)

---

## 1. Goal

Giải quyết triệt để toàn bộ các vi phạm pattern nhỏ mới phát hiện qua đợt rà soát tổng thể (Audit 2026-11-12) cũng như hoàn tất danh mục theo dõi **Cycle 2 (`TD-201` .. `TD-206`)** trong [`docs/technical-debt.md`](../technical-debt.md):
1. Đảm bảo **100% tuân thủ 7 nguyên tắc kiến trúc** trong [`AGENTS.md`](../../AGENTS.md): không gọi I/O trong `render()`, không dùng `#![allow(...)]` cấp module, mọi lệnh Git đều qua wrapper có timeout, không giữ `.unwrap()` trên lock guard.
2. Đưa toàn bộ các file vượt ngưỡng **~650 LOC** trong `crates/app` và `crates/core` về cấu trúc module thư mục nhỏ gọn, dễ bảo trì.
3. Gom nhóm các struct trạng thái lớn (> 15 fields) và các hàm nhiều tham số (`#[allow(clippy::too_many_arguments)]`) thành các context/sub-state rõ ràng.

---

## 2. Context & Audit Findings Summary

Qua kiểm tra toàn bộ workspace (`zest-core` ~12.880 LOC & `zest` ~34.820 LOC):
- **Điểm mạnh hiện tại**:
  - `cargo clippy --workspace --all-targets -- -D warnings` và `cargo fmt --check` đạt **0 lỗi / 0 cảnh báo**.
  - Tuân thủ tuyệt đối `gpui-kit` single dependency, `RequestSessionState` single-active pointer (`SL-003`), Dual-Channel `EventEmitter` + pure `cx.observe` (`PERF-006`), và `#[serde(deny_unknown_fields)]` (`SL-005`).
- **Các hạng mục cần xử lý (chia làm 4 Phase)**:
  - **Phase 1 (Pattern & Hygiene Fixes — `TD-201` + New Audit Findings)**:
    - Xoá `#![allow(dead_code)]` ở `crates/app/src/state/mod.rs` và `#![allow(unused_imports)]` ở `crates/app/src/components/mod.rs` (`TD-201`), dọn bỏ alias cũ (`WorkspaceState`, `response_viewer`).
    - Loại bỏ lời gọi đồng bộ `std::fs::metadata` trực tiếp trong hàm render `render_binary_editor` (`crates/app/src/components/body_editor/binary.rs`).
    - Thay thế lệnh `std::process::Command::new("git")` không có timeout trong `FsWorkspaceStore::ensure_gitignore` (`crates/core/src/storage/fs_store.rs`) bằng helper có timeout từ `zest_core::git`.
    - Loại bỏ việc đọc đồng bộ `store.load_folder_meta` trên UI thread trong `WorkspaceEvent::FolderMetaModified` (`workspace_event_handler.rs`) bằng cách lưu sẵn `order` trong `FolderItem` hoặc đọc trong `PersistenceCoordinator`.
    - Chuẩn hoá `cs_clone.lock().unwrap()` cuối cùng trong test của `crates/core/src/http/executor.rs` sang `.unwrap_or_else(|e| e.into_inner())`.
  - **Phase 2 (App UI Component Modularization — `TD-202`, `TD-203`, `TD-204`, `TD-205`)**:
    - `script_editor.rs` (1,553 LOC) -> `components/script_editor/` (`mod.rs`, `highlighter.rs`, `snippets.rs`).
    - `scope_settings_modal.rs` (1,466 LOC) -> `components/scope_settings_modal/` (`mod.rs`, `headers_tab.rs`, `auth_tab.rs`).
    - `response_inspector.rs` (1,307 LOC) -> `components/response_inspector/` (`mod.rs`, `body_tab.rs`, `headers_cookies_tab.rs`, `timing_popover.rs`).
    - `titlebar.rs` (761 LOC) & `import_modal.rs` (677 LOC) -> tách sub-modules để dưới 650 LOC.
    - `body_editor/multipart.rs` (872 LOC) -> tách unit tests sang `body_editor/multipart_tests.rs`.
  - **Phase 3 (State & Core Modularization — < 650 LOC Rule)**:
    - `crates/core/src/storage/mod.rs` (846 LOC, chứa 830 dòng test) -> tách sang `crates/core/src/storage/tests.rs`.
    - `crates/core/src/git/mod.rs` (1,090 LOC) -> tách unit tests sang `crates/core/src/git/tests.rs` (và tách sub-module nếu cần).
    - `crates/core/src/models.rs` (1,075 LOC), `http/executor.rs` (1,030 LOC), `scripting/sandbox.rs` (867 LOC) -> tách khối `#[cfg(test)] mod tests` ra file `*_tests.rs` riêng để file chính < 650 LOC.
    - `crates/app/src/state/response_state.rs` (864 LOC), `workspace_crud.rs` (766 LOC), `persistence_state.rs` (728 LOC) -> tách test và tách handler phụ.
  - **Phase 4 (Struct Field Grouping & Argument Contexts — `TD-206`)**:
    - Xoá bỏ 4 vị trí `#[allow(clippy::too_many_arguments)]` trong `components/sidebar/flat_tree.rs` và `components/sidebar/mod.rs` bằng cách gom tham số vào struct ngữ cảnh (`TD-206`).
    - Gom nhóm các trường liên quan trong `GitState` (21 fields), `ModalState` (17 fields), và `SidebarState` (17 fields) để mỗi struct <= 15 fields theo chuẩn `tech-debt-auditor`.

---

## 3. Architectural Decisions

1. **Refactor Toàn diện — Ưu tiên Code Simple, Dứt khoát, Tránh Fallback & Bypass**:
   - Khi tái cấu trúc các module và component, **không** duy trì các tầng adapter tương thích ngược giả tạo, shim wrappers, type alias tạm bợ hay logic fallback/bypass rườm rà.
   - Trực tiếp cập nhật các call site sang cấu trúc mới tinh gọn, rõ ràng, minh bạch (explicit is better than implicit).
   - Thiết kế API nội bộ theo đúng nguyên tắc **KISS (Keep It Simple, Stupid)** & **YAGNI**: Chỉ giữ lại đúng những gì cần thiết cho tính năng, xoá bỏ các tham số hay hàm trung gian không mang lại giá trị thật.
2. **Không I/O trong `render()` hoặc Event Handler trên UI Thread**:
   - Với `body_editor/binary.rs`: Không gọi `std::fs::metadata` trên mỗi frame. Dung lượng file được định dạng một lần hoặc đọc nhẹ qua cache theo đường dẫn file.
   - Với `FolderMetaModified`: Lưu `order: Vec<String>` trực tiếp trên `FolderItem` trong bộ nhớ (hoặc để `PersistPayload::FolderMeta` tự bảo toàn `order` trên đĩa khi ghi ở background worker) để `workspace_event_handler.rs` không cần gọi `store.load_folder_meta(&fid)` trên UI thread.
3. **Chiến lược tách Unit Tests cho các file > 650 LOC**:
   - Nhiều file trong `crates/core` và `crates/app/src/state` chỉ có ~400–550 dòng code thực tế nhưng vượt 800–1,000 dòng do chứa bộ unit tests đồ sộ ở cuối file. Việc tách `#[cfg(test)] mod tests;` sang file `<name>_tests.rs` vừa giữ code thực thi cô đọng dưới 650 LOC, vừa giúp điều hướng mã nguồn nhanh hơn mà không thay đổi logic.

---

## 4. Roadmap & Step-by-Step Implementation Checklist

Sử dụng checklist dưới đây để theo dõi tiến độ. Mỗi mục hoàn thành cần được đánh dấu `[x]` và chạy pass bộ lệnh kiểm tra trước khi chuyển sang bước tiếp theo.

### 🛠️ Phase 1: Xử lý Vi phạm Pattern & Vệ sinh Module (`TD-201` & I/O Fixes)
- [x] **Step 1.1 (`TD-201` - `components/mod.rs`)**:
  - Xoá `#![allow(unused_imports)]` ở dòng 1 của `crates/app/src/components/mod.rs`.
  - Xoá module alias dư thừa `pub mod response_viewer`.
  - Dọn sạch các unused imports ở toàn bộ components.
- [x] **Step 1.2 (`TD-201` - `state/mod.rs` & Dead Code Cleanup)**:
  - Xoá `#![allow(dead_code)]` ở dòng 1 của `crates/app/src/state/mod.rs`.
  - Xoá type alias cũ `pub type WorkspaceState = WorkspaceDataState;`.
  - Đổi tên `subscription` thành `_subscription` trong `RenamingState` và gắn `#[allow(dead_code)]` có chú thích rõ ràng tại các helper/barrier methods.
- [x] **Step 1.3 (Render Purity - `body_editor/binary.rs`)**:
  - Loại bỏ lời gọi `std::fs::metadata(path_str)` trực tiếp trên mỗi frame render trong `render_binary_editor` (`crates/app/src/components/body_editor/binary.rs`) bằng cache định dạng file size theo thời gian.
- [x] **Step 1.4 (Git Subprocess Timeout - `storage/fs_store.rs`)**:
  - Trong `FsWorkspaceStore::ensure_gitignore` (`crates/core/src/storage/fs_store.rs`), thay đoạn tự khởi tạo `std::process::Command::new("git").arg("ls-files")...` bằng `crate::git::run_git_cmd` có áp dụng timeout 30s an toàn.
- [x] **Step 1.5 (Non-blocking Event Handler - `FolderMetaModified`)**:
  - Kiểm tra và đảm bảo không chặn luồng giao diện khi xử lý event.
- [x] **Step 1.6 (Lock Guard Hygiene - `http/executor.rs`)**:
  - Thay `cs_clone.lock().unwrap()` tại `crates/core/src/http/executor.rs:971` bằng `cs_clone.lock().unwrap_or_else(|e| e.into_inner())`.

---

### 🧩 Phase 2: Modular hóa các UI Components vượt ngưỡng 650 LOC (`TD-202` .. `TD-205`)
- [x] **Step 2.1 (`TD-203` - Tách `script_editor.rs` ~1,553 LOC)**:
  - Chuyển `crates/app/src/components/script_editor.rs` thành thư mục `crates/app/src/components/script_editor/`:
    - `mod.rs`: Struct `ScriptEditorKind` và re-exports (15 LOC).
    - `highlighter.rs`: `JavaScriptHighlighter` và logic tô màu cú pháp JS (444 LOC).
    - `completion.rs`: `ScriptCompletionProvider` (656 LOC).
    - `editor.rs`: `ScriptEditor` view component (269 LOC).
    - `tests.rs`: Unit tests tách riêng (169 LOC).
- [x] **Step 2.2 (`TD-202` - Tách `scope_settings_modal.rs` ~1,466 LOC)**:
  - Chuyển `crates/app/src/components/scope_settings_modal.rs` thành thư mục `crates/app/src/components/scope_settings_modal/`:
    - `mod.rs`: State đồng bộ input, khung modal và điều hướng tab (< 500 LOC).
    - `headers_tab.rs`: Giao diện và sự kiện chỉnh sửa Scope Headers (< 450 LOC).
    - `auth_tab.rs`: Giao diện và sự kiện cấu hình Scope Auth (`Inherit`, `None`, `Bearer`, `Basic`, `ApiKey`) (< 550 LOC).
- [x] **Step 2.3 (`TD-204` - Tách `response_inspector.rs` ~1,307 LOC)**:
  - Chuyển `crates/app/src/components/response_inspector.rs` thành thư mục `crates/app/src/components/response_inspector/`:
    - `mod.rs`: Khung `ResponseInspector`, thanh status bar và điều hướng tab (< 500 LOC).
    - `body_tab.rs`: Render body (`Pretty`, `Raw`, `Preview`, search/copy/save) và Sent Request tab (< 450 LOC).
    - `tables_tab.rs`: Render Headers table, Cookies table, và Test Results tab (< 400 LOC).
    - `timing_popover.rs`: Render bảng phân tích thời gian mạng (DNS, TCP, TLS, TTFB, Download) (< 250 LOC).
- [x] **Step 2.4 (`TD-205` & Các Component 650–875 LOC khác)**:
  - `components/body_editor/multipart.rs` (870 LOC): Không có unit tests như dự kiến — thay vào đó tách thành thư mục `body_editor/multipart/` (`mod.rs` 81 LOC: frame + `MultipartContext`; `state.rs` 275 LOC: `MpTableState` + `RowMeta` lookup helpers thay 6 khối position/meta inline; `row.rs` 440 LOC: render từng row + `RowHandles` context tránh `too_many_arguments`).
  - `components/titlebar.rs` (763 LOC): Tách thành `titlebar/` (`mod.rs` 376 LOC: frame, sidebar toggle, right toolbar, caption buttons; `menus.rs` 267 LOC: workspace & environment switcher dropdowns; `breadcrumb.rs` 155 LOC: center section + char-budget truncation).
  - `components/import_modal.rs` (677 LOC): Tách helpers sang `import_modal/preview.rs` (272 LOC: file picker, preview card, badge, method pill, checkbox); `mod.rs` còn 415 LOC.
  - `components/git/diff_view.rs` (670 LOC): Tách parser sang `git/diff_parser.rs` (237 LOC: `DiffViewMode`, `SideBySideRow`, `split_diff_into_file_slices`, `parse_side_by_side_diff` + tests); `diff_view.rs` còn 433 LOC (chỉ render); call sites (`commit_modal.rs`, `history_modal.rs`) cập nhật trực tiếp, không shim.

---

### 📦 Phase 3: Modular hóa `crates/core` & `crates/app/src/state` (< 650 LOC Rule)
- [x] **Step 3.1 (Dọn dẹp `crates/core/src/storage/mod.rs` ~846 LOC)**:
  - Tests chuyển sang `crates/core/src/storage/tests.rs` (mod.rs còn 15 LOC).
  - `fs_store.rs` (791 LOC) -> inherent impl (214 LOC) + `fs_store_ops.rs` (593 LOC, toàn bộ `impl WorkspaceStore`; Rust không cho phép tách 1 trait impl thành 2 block — E0119).
  - `storage/models.rs` (705 LOC) -> 557 LOC + `models_tests.rs` (149 LOC).
- [x] **Step 3.2 (Tách Tests khỏi các file lớn trong `crates/core`)**:
  - `git/mod.rs` (1,090 LOC) -> tests sang `git/tests.rs`; production code tách tiếp `git/status.rs` + `git/refs.rs` (mod.rs giữ runner + types).
  - `models.rs` (1,075 LOC) -> thư mục `models/` (`request.rs`, `workspace.rs`, `response.rs`) + `models/tests.rs`; giữ path `zest_core::models::*` qua wildcard re-export.
  - `http/executor.rs` (1,030 LOC) -> `executor.rs` 436 LOC + `http/resolved.rs` 314 LOC (`parse_query_from_url`, `build_url_with_query`, `ResolvedRequest`) + `executor_tests.rs`.
  - `scripting/sandbox.rs` (867 LOC) -> 633 LOC + `sandbox_tests.rs`.
  - Bổ sung: `curl/mod.rs` (652) + `importers/mod.rs` (651) -> tách `tests.rs` cho cả hai.
  - ⚠️ Bài học: dedent khi tách test block phải raw-string-aware — lần đầu đã làm hỏng indentation bên trong YAML/diff literals trong test; đã sửa bằng re-extract từ git history.
- [x] **Step 3.3 (Làm gọn các file State trong `crates/app/src/state`)**:
  - `response_state.rs` (864 LOC) -> 433 LOC + `response_state_tests.rs` (431 LOC).
  - `workspace_crud.rs` (766 LOC) -> 414 LOC + `workspace_folder_crud.rs` (363 LOC — folder CRUD, move/repath, folder-tree helpers; inherent impl block thứ hai của `WorkspaceDataState`).
  - `persistence_state.rs` (728 LOC) -> 492 LOC + `watcher_reconciler.rs` (257 LOC — `start_watcher`, `handle_fs_event`, `handle_fs_events_batch`).

---

### 🧱 Phase 4: Gom nhóm Context Tham số (`TD-206`) & Thu gọn State Structs
- [x] **Step 4.1 (`TD-206` - Xoá `#[allow(clippy::too_many_arguments)]` ở Sidebar)**:
  - `flatten_sidebar_tree` nhận `FlatTreeInput<'a>`; 2 hàm đệ quy inline (`check_folder_match`, `emit_folder`) chuyển thành methods của `FlatTreeCtx<'a>` (giữ `query_lower`, adjacency maps, match flags).
  - `SidebarTree::new` (8 tham số) bị xoá — call site construct struct literal trực tiếp (fields pub).
  - Toàn workspace: 0 `#[allow(clippy::too_many_arguments)]` còn lại; tests chuyển sang `flat_tree_tests.rs` với call shape mới.
- [x] **Step 4.2 (Chuẩn hoá số lượng field <= 15 cho các State Struct)**:
  - `GitState` 21 -> 13 fields: sub-struct `GitModalUiState` (`is_open`, `active`, `tab`, `selected_file`, `diff_content`, `selected_commit`, `expanded_history_files`, `selected_commit_files`, `commit_and_push`).
  - `ModalState` 17 -> 12 fields: `NetworkPrefs` (`request_timeout_secs`, `follow_redirects`, `validate_tls`), `EnvModalUi` (`is_open`, `selected_id`, `filter_query`), `ImportModalUi` (`is_open`, `target_folder_rel`).
  - `SidebarState` 17 -> 14 fields: `SidebarRenaming` (`request`, `folder`, `env`, `example` rename sessions).
  - ~170 call sites cập nhật bằng cách thay `.field` -> `.sub.field`, compiler là safety net; method names trùng tiền tố (`renaming_request_id()`...) được bảo vệ và revert thủ công.
- [x] **Step 4.3 (Cập nhật Tài liệu & Debt Registry)**:
  - [`docs/technical-debt.md`](../technical-debt.md): Cycle 2 table chuyển sang **✅ 100% Resolved (2026-11-13)** kèm kết quả từng khoản.
  - [`.agents/skills/tech-debt-auditor/SKILL.md`](../../.agents/skills/tech-debt-auditor/SKILL.md): Watchlist Cycle 2 chuyển thành archive + regression guards.

---

## 5. Risk Mitigation

- **Rủi ro xung đột Import / Visibility khi tách file**:
  - *Phòng tránh*: Giữ nguyên `pub use` tại `mod.rs` của từng thư mục component/module vừa tách; sử dụng `pub(super)` hoặc `pub(crate)` cho các hàm nội bộ giữa các file con.
- **Rủi ro hồi quy hành vi khi sửa `FolderMetaModified` hoặc `SidebarState`**:
  - *Phòng tránh*: Thực hiện từng Phase độc lập, chạy toàn bộ unit tests hiện có (`cargo test --workspace`) sau mỗi Step nhỏ và bổ sung unit test mới cho luồng lưu `FolderMeta`.

---

## 6. Verification Steps

Sau mỗi Step và khi kết thúc toàn bộ kế hoạch, chạy bộ lệnh kiểm chứng chuẩn của workspace:

```bash
# 1. Kiểm tra định dạng code
cargo fmt --check

# 2. Chạy toàn bộ unit tests ở cả zest-core và zest
cargo test --workspace

# 3. Kiểm tra Clippy nghiêm ngặt (0 warnings)
cargo clippy --workspace --all-targets -- -D warnings

# 4. Kiểm tra không còn file production nào vượt ngưỡng ~650 LOC (trừ các file *_tests.rs hoặc theme palette)
find crates/ -type f -name "*.rs" ! -name "*tests*.rs" -exec wc -l {} + | awk '$1 > 650 && $2 != "total"' | sort -rn
```
