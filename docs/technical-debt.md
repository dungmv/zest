# Danh mục Nợ Kỹ Thuật Tập Trung (Technical Debt Registry)

> **Mục đích:** Tài liệu duy nhất và chính thức ghi nhận toàn diện các khoản nợ kỹ thuật (Technical Debt), các anti-pattern, điểm nghẽn kiến trúc và danh sách theo dõi modularization trong codebase `zest` (`zest-core` và `zest`). Mọi cập nhật về nợ kỹ thuật bắt buộc phải ghi nhận trực tiếp vào tài liệu này.

---

## 🏅 1. Cycle 2 Watchlist (TD-201..TD-206 — ✅ 100% Resolved, 2026-11-13)

Toàn bộ **31/31 khoản nợ kỹ thuật của Vòng 1 (Cycle 1)** và **6/6 khoản của Cycle 2** đã hoàn thành. Chi tiết giải pháp từng khoản:

| ID | Vấn đề / Khoản nợ | Kết quả |
| :--- | :--- | :--- |
| **TD-201** | Module-wide `#![allow(dead_code)]` / `#![allow(unused_imports)]` | ✅ Đã xoá ở `state/mod.rs`, `components/mod.rs`; alias cũ (`WorkspaceState`, `response_viewer`) dọn sạch; `#[allow(dead_code)]` scope hẹp có chú thích chỉ còn ở helper/barrier thật sự |
| **TD-202** | `scope_settings_modal.rs` ~1.515 LOC | ✅ Tách thành `components/scope_settings_modal/` (`mod.rs`, `general_tab.rs`, `headers_tab.rs`, `auth_tab.rs`, `auth_inputs.rs`) |
| **TD-203** | `script_editor.rs` ~1.443 LOC | ✅ Tách thành `components/script_editor/` (`mod.rs`, `highlighter.rs`, `completion.rs`, `editor.rs`, `tests.rs`) |
| **TD-204** | `response_inspector.rs` ~1.334 LOC | ✅ Tách thành `components/response_inspector/` (`mod.rs`, `body_tab.rs`, `headers_tab.rs`, `cookies_tab.rs`, `console_tab.rs`, `request_sent_tab.rs`, `timing_popover.rs`, `json_highlight.rs`) |
| **TD-205** | Các component 650–875 LOC (`titlebar`, `import_modal`, `body_editor/multipart`, `git/diff_view`) | ✅ `titlebar/` (mod+menus+breadcrumb), `import_modal/` (mod+preview), `body_editor/multipart/` (mod+state+row), `git/diff_view.rs` + `git/diff_parser.rs` |
| **TD-206** | `#[allow(clippy::too_many_arguments)]` ở sidebar | ✅ `flatten_sidebar_tree(FlatTreeInput)` + `FlatTreeCtx` methods; `SidebarTree` construct qua struct literal; 0 allow còn lại trong workspace. Đồng thời gom field: `GitState` 21→13 (`GitModalUiState`), `ModalState` 17→12 (`NetworkPrefs`, `EnvModalUi`, `ImportModalUi`), `SidebarState` 17→14 (`SidebarRenaming`) |

Ngoài Cycle 2, các file core vượt ngưỡng cũng đã được modular hóa trong cùng đợt: `storage/` (mod 15 LOC + `tests.rs`, `fs_store.rs` + `fs_store_ops.rs`, `models.rs` + `models_tests.rs`), `git/` (mod + `status.rs` + `refs.rs` + `tests.rs`), `models/` (request + workspace + response), `http/executor.rs` + `http/resolved.rs`, `scripting/sandbox_tests.rs`, `curl/tests.rs`, `importers/tests.rs`, `state/` (`response_state_tests.rs`, `workspace_folder_crud.rs`, `watcher_reconciler.rs`). Verification: `cargo test --workspace` 223/223 pass, `cargo clippy --workspace --all-targets -- -D warnings` 0 warnings, toàn bộ file production (ngoại trừ theme palette) ≤ 650 LOC.

---

## 🏆 2. Lưu trữ Lịch sử Vòng 1 (Cycle 1 — 100% Resolved Archive)

Toàn bộ các nền tảng kiến trúc dưới đây đã được hoàn tất, kiểm thử tự động và tích hợp vào `.agents/skills/` cùng `AGENTS.md`:

### 2.1 Các trụ cột kiến trúc đã hoàn thành
1. **Multi-Entity State & Single-Active Request (`SL-001`, `SL-003`, `TD-02`, `TD-04`):**
   - Phân rã God Struct `WorkspaceState` (>3.300 LOC) và `AppView` (32 fields) thành 9 GPUI Entities độc lập trong `AppState` cùng `ComposerInputs`, `InspectorDisplays`, `HttpEngine` và `Runner`.
   - `RequestSessionState` chỉ giữ `active_request_id: Option<String>`, mượn `&HttpRequestItem` trực tiếp từ `WorkspaceDataState`.
2. **Dual-Channel Typed Event Emitter & Pure Redraw (`PERF-006`, `TD-05`):**
   - Tách biệt hoàn toàn kênh vẽ lại giao diện (`cx.observe` chỉ gọi `cx.notify()`) và kênh đồng bộ dữ liệu liên-state (`cx.emit` + `cx.subscribe_in` với `WorkspaceEvent`, `SessionEvent`, `ResponseEvent`, `EnvironmentEvent` xử lý tập trung tại `workspace_event_handler.rs`).
   - Xoá bỏ hoàn toàn việc đọc/so sánh cờ tạm (`last_synced_*`, `*_version`, `is_dirty`) trong render loop và observer.
3. **Unified Write-Behind Persistence (`PersistenceCoordinator`):**
   - Hợp nhất toàn bộ luồng ghi YAML (`Request`, `Environment`, `Globals`, `WorkspaceMeta`, `FolderMeta`, `Pins`, `LocalState`) qua `PersistenceCoordinator` (`state/persistence_coordinator.rs`).
   - Hỗ trợ coalesce theo target, quản lý thứ tự bằng `revision` đơn điệu và `workspace_gen`, ghi nhóm nguyên tử (environment + secrets), và remap/cancel khi rename/move/delete/discard.
4. **File Watcher Defense-in-Depth & GitService Extraction (`PERF-005`, `CC-002`, `CC-003`, `TD-003`, `TD-018`, `TD-019`):**
   - Tách toàn bộ lệnh Git CLI sang `GitService` (`services/git_service.rs`) chạy bất đồng bộ với timeout 30s (`run_git_cmd_timeout`).
   - Bảo vệ file untracked khi discard từ sidebar (`delete_untracked = false`) và tự động hạ cấp sự kiện `RequestDeleted` -> `RequestChanged` khi file vẫn tồn tại trên đĩa (chống mất request khi `git restore` hoặc atomic save).
   - Theo dõi chọn lọc `.git/` (`FsEvent::GitChanged`), gom lô sự kiện watcher (debounce 100ms–200ms), nhận diện self-echo bằng SHA-256 `DiskFingerprints` + `AtomicWriter` (stage file tạm tại `.local/tmp/`), bảo toàn dữ liệu đang gõ (`apply_disk_changes` / `reload_tree_preserving`) và đồng bộ `EnvVarTable` thuần qua kênh sự kiện.
5. **Tối ưu Mạng, Bộ nhớ & Schema (`PERF-001..004`, `PERF-007..010`, `SL-002`, `SL-005..007`, `TD-005..017`):**
   - Chia sẻ connection pool trong `ReqwestExecutor`, xoá bỏ TCP probe thừa.
   - Cache phản hồi giới hạn kép `LruMemoryCache` (100 entries / 50MB RAM) bọc trong `Arc<HttpResponseRecord>`, kèm cache định dạng `Pretty`/`Raw` (`CachedResponseFormat`).
   - Áp dụng `#[serde(deny_unknown_fields)]` trên toàn bộ YAML models, vòng lặp an toàn chu kỳ cho OpenAPI `$ref` (`depth < 32`), và chuẩn hoá 100% comment tiếng Anh trong source code.

---

## 🛠️ 3. Quy trình quản lý Nợ Kỹ Thuật (Maintenance Protocol)

1. **Một nguồn sự thật duy nhất (Single Source of Truth):** Mọi khoản nợ kỹ thuật phát hiện mới hoặc cập nhật trạng thái đều ghi trực tiếp vào [`docs/technical-debt.md`](file:///Users/dungmv/Documents/macos-native-apps/zest/docs/technical-debt.md). Không tạo thêm các file phụ (`technical-debt-02.md`, v.v.).
2. **Kèm Unit Test chống hồi quy:** Khi giải quyết xong bất kỳ mục nợ kỹ thuật nào, bắt buộc bổ sung unit test kiểm chứng và chạy xanh toàn bộ pipeline (`cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`).
