# Danh mục Nợ Kỹ Thuật Tập Trung (Technical Debt Registry)

> **Mục đích:** Tài liệu duy nhất và chính thức ghi nhận toàn diện các khoản nợ kỹ thuật (Technical Debt), các anti-pattern, điểm nghẽn kiến trúc và danh sách theo dõi modularization trong codebase `zest` (`zest-core` và `zest`). Mọi cập nhật về nợ kỹ thuật bắt buộc phải ghi nhận trực tiếp vào tài liệu này.

---

## 🔧 0. Cycle 3 Watchlist — Hiệu chuẩn hiệu năng (2026-11-22/23, plan `docs/plans/2026112202-perf-remediation.md`)

Đợt audit hiệu năng toàn workspace (render-path purity, UI-thread blocking, memory growth) đã xử lý xong P0–P2; các khoản defer có chủ đích ghi nhận dưới đây.

### 0.1 Đã xử lý (tóm tắt)
- **Script sandbox**: QuickJS `set_interrupt_handler` (deadline 30s + `ScriptCancel` flag), `set_memory_limit(256MB)`, stack cap; Cancel giờ ngắt được cả pre/post script (bridge oneshot → flag, transport giữ oneshot riêng). Tests: cancel/timeout/memory-bomb.
- **Disk I/O khỏi UI thread**: `save_response_cache` + `save_cookies` chạy 1 task nền tuần tự per response; `ensure_gitignore` (git subprocess ≤30s) nền ở startup + open_workspace; watcher `TreeChanged`/rescan quét nền (`reload_tree_preserving_async`, coalesce in-flight).
- **Startup async**: `WorkspaceDataState::new` không còn quét đồng bộ — `load_workspace_async` + replay selection qua handler `RequestsReloaded` (flag `initial_restore_done`); stale-store guard `Arc::ptr_eq`.
- **Format body cap**: >2MB skip pretty-print; line cap 2000 dòng / 1000 ký tự + marker.
- **Streaming O(n²)**: Messages tab virtualized (`ListState` + `FollowMode::Tail`), preview capped head-scan; bỏ reset scroll handle per frame.
- **Palette O(R×depth×F)/keystroke**: cache keyed (query, tree_version, url_version, recent_len) + memo folder-path; `refresh_windows` → notify qua `modal_state`.
- **`find_request` O(1)**: index id→position keyed `tree_version` (cùng contract với sidebar flat-cache).
- **Clone per-frame**: BodyEditor `BodyView` (Json/Raw distill), titlebar env (id, name), `grpc_schema` → `Arc`, console panel virtualized + cap 1000 logs/run.
- **`with_retry`**: budget 500ms → ~60ms.

### 0.2 Nợ defer có chủ đích

| ID | Vấn đề | Lý do defer | Gợi ý xử lý |
| :--- | :--- | :--- | :--- |
| **TD-301** | `inherited_readonly_rows` re-walk hierarchy + re-mask secrets per frame khi Headers tab mở | Chi phí thực tế thấp (O(depth×F) ~ vài trăm iter); cache-key đúng đắn phải bao gồm header keys + auth discriminant (header edits không bump `tree_version`) — phức tạp hơn lợi ích | Memoize với key `(tree_version, req.id, folder_id, header-keys, auth-discriminant)` nếu workspace ~hàng nghìn folder |
| **TD-302** | `request_sent_tab` build `escape_md` + `TextView::markdown` per frame | Chi phí bounded (URL + headers nhỏ); `TextView` có element id, có thể tự cache nội bộ | Memoize per response pointer (`CachedResponseFormat` pattern) nếu thấy hitch |
| **TD-303** | KvTable nhận owned `Vec<KeyValuePair>` → headers/params/gRPC payload clone per frame ở composer tabs | Cần refactor borrow-based input cho KvTable (chạm nhiều component) | `KvTable` chấp nhận `&[KeyValuePair]` hoặc Arc-share rows |
| **TD-304** | Mỗi keystroke clone full editor value (`value().to_string()`) trong `subscriptions.rs` handlers | Giới hạn API `gpui_kit` InputState (`value()` trả owned String) | Đẩy lên gpui-kit: API borrow/view hoặc callback nhận `&str` |
| **TD-305** | Folder/file mutation vẫn ghi đĩa sync trên UI thread (chỉ thu hẹp retry budget) | Chuyển async cần đảm bảo thứ tự với watcher fingerprint staging | Đưa qua background executor per call site, giữ staging-before-write |
| **TD-306** | Re-validate toàn bộ gRPC payload + variables JSON mỗi keystroke (`subscriptions.rs`) | Body nhỏ: chấp nhận được; body lớn mới thấy | Debounce theo độ dài payload (vd >64KB) hoặc validate on-idle |

### 0.3 Drift đã dọn
- `AGENTS.md` §2.1: bỏ entity `toast: Entity<ToastState>` (không còn tồn tại trong code), đếm lại 8 entities.

---

## 🏅 1. Cycle 2 Watchlist (TD-201..TD-206 — ✅ 100% Resolved, 2026-11-13)

Toàn bộ **31/31 khoản nợ kỹ thuật của Vòng 1 (Cycle 1)** và **6/6 khoản của Cycle 2** đã hoàn thành. Chi tiết giải pháp từng khoản:

| ID | Vấn đề / Khoản nợ | Kết quả |
| :--- | :--- | :--- |
| **TD-201** | Module-wide `#![allow(dead_code)]` / `#![allow(unused_imports)]` | ✅ Đã xoá ở `state/mod.rs`, `components/mod.rs`; alias cũ (`WorkspaceState`, `response_viewer`) dọn sạch; `#[allow(dead_code)]` scope hẹp có chú thích chỉ còn ở helper/barrier thật sự |
| **TD-202** | `workspace_setting_modal.rs` ~1.515 LOC | ✅ Tách thành `components/workspace_setting_modal/` (`mod.rs`, `general_tab.rs`, `headers_tab.rs`, `auth_tab.rs`, `auth_inputs.rs`) |
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
