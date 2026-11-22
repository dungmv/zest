# Plan 2026112202 — Hiệu chuẩn hiệu năng (Perf Remediation)

> Ngày: 2026-11-22 · Nguồn: audit hiệu năng toàn workspace (clippy/test + 3 luồng audit: render-path, UI-thread blocking, memory growth)
> Trạng thái: 📋 Chưa triển khai

## 1. Goal

Khắc phục toàn bộ 14 findings hiệu năng đã xác minh, chia 4 phase theo độ ưu tiên:

- **P0 — Ngăn hang/freeze cảm nhận được** (script treo vĩnh viễn, disk I/O trên UI thread khi nhận response, format body không cap)
- **P1 — Hot path tương tác** (streaming O(n²), command palette O(R×depth×F)/keystroke, startup/rescan trên main thread)
- **P2 — Clone sâu per-frame** (dọn dần, mechanical)
- **P3 — Vệ sinh** (doc drift, ghi nhận debt còn lại)

Nguyên tắc xuyên suốt: không phá các quyết định kiến trúc hiện có (SL-002..006, PERF-006, PersistenceCoordinator, dual-channel events), không restyle UI (tham chiếu [[refactor-must-not-restyle-ui]]).

## 2. Context — findings đã xác minh

| # | Finding | Vị trí | Mức |
|---|---------|--------|-----|
| F1 | Startup/open_workspace: full scan tree + YAML parse + `git ls-files` (timeout 30s) sync trên main thread | `workspace_data_state.rs:65-117`, `:91`, `:159`; `fs_store_ops.rs:92` | HIGH |
| F2 | Watcher `TreeChanged` → `scan_tree()` full lại trên UI thread (kể cả do chính app tạo/rename folder) | `watcher_reconciler.rs:164,215` | HIGH |
| F3 | Mỗi response thành công: JSON serialize (≤512KB) + SHA-256 + 2 disk writes trên UI thread | `runner.rs:274-275` | HIGH |
| F4 | QuickJS không có interrupt handler/memory limit → `while(true){}` treo tokio worker vĩnh viễn; Cancel không dừng được post-script (chạy ngoài `select!`) | `sandbox.rs:116,266`; `http_engine.rs:156-164` | HIGH |
| F5 | `build_cached_format`: parse + pretty-print + line-split toàn body, không size cap, trên UI thread | `response_state.rs:388-426` | HIGH |
| F6 | `with_retry` spin-sleep tới 500ms trên UI thread khi delete/rename (Windows sharing violation) | `serialize.rs:29-43` | MED |
| R1 | Messages tab (WS/gRPC/SSE): clone toàn bộ frames + render không virtualize + `from_utf8_lossy` full payload per card per frame → O(n²) khi streaming | `messages_tab.rs:21,65,173`; `response_state.rs:542-549` | HIGH |
| R2 | Console panel: clone toàn bộ logs/tests, không virtualize, `console_logs` không có cap trong 1 lần chạy script | `console_panel.rs:31-44` | MED |
| P1 | Command palette: O(R×depth×F) folder-walk + `to_lowercase()` mọi path mỗi keystroke + `cx.refresh_windows()` toàn cửa sổ mỗi keystroke | `command_palette.rs:62-70,100-124` | HIGH |
| P3 | `find_request` linear scan 6-10 lần mỗi keystroke | `workspace_data_state.rs:502` | MED |
| R3 | BodyEditor clone toàn bộ body mỗi frame chỉ để đọc variant | `body_editor/mod.rs:103-107` | MED |
| R4 | Titlebar env switcher clone toàn bộ `environments` (kèm variables) mỗi frame | `titlebar/menus.rs:210` | MED |
| R5 | gRPC schema clone per frame | `grpc_message_tab.rs:37-42` | MED |
| R6 | `inherited_readonly_rows` re-walk hierarchy + re-mask secrets per frame | `inherited_headers.rs:22-73` | MED |

Đã sạch, không đụng tới: LRU cache dual-bound, frame log caps (view+engine), sidebar flat-cache versioned + virtualized, `cx.observe` chỉ `cx.notify()`, persistence debounced background, git/HTTP engines off-thread, lock poisoning-safe.

## 3. Architectural Decisions

- **AD-1 — Script guard một chỗ**: interrupt handler + memory limit đặt tại nơi tạo QuickJS `Runtime` trong `ScriptSandbox` (phủ cả pre/post script, WS/gRPC/SSE vì mọi đường đi script đều qua sandbox này). Deadline + cancel flag qua `Arc<AtomicBool>`; interrupt handler trả `false` khi (a) cancel flag bật, hoặc (b) quá deadline. Không thêm config UI trong đợt này — hằng số có doc comment.
- **AD-2 — Disk write background, giữ thứ tự**: `save_response_cache` + `save_cookies` chạy trong **một** task `background_executor` duy nhất cho mỗi lần response về (ghi tuần tự trong 1 task ⇒ không race cùng file; guard `Sending` đã chặn double-send cùng request). In-memory state (`last_status_code`, `cookies`) vẫn cập nhật sync trên UI thread.
- **AD-3 — Cap trước, offload sau**: `build_cached_format` cap ở 2MB (`MAX_FORMAT_BODY_BYTES`) — body lớn skip JSON parse/pretty, line-split tối đa 2000 dòng + marker cắt. Không chuyển sang async trong đợt này (đo lại sau khi cap; offload để debt nếu vẫn hitch).
- **AD-4 — Virtualize theo pattern `body_tab.rs`**: dùng `uniform_list` + đọc state trong item closure theo index (không clone vec); áp dụng bài học [[gpui-scrollable-noop-pitfall]] — scroll_to_item trên list div có `.id()`, scrollbar trên parent không track.
- **AD-5 — Palette cache-in-struct, không convert Entity (v1)**: `CommandPalette` giữ struct nhưng cache folder-path map + kết quả filter keyed `(query_lower, tree_version)` trong `RefCell`; thay `cx.refresh_windows()` bằng `modal_state.update(cx, |_, cx| cx.notify())` (`modal_state` đã được observe tại `subscriptions.rs:527`). Convert Entity chỉ làm nếu v1 vẫn lag.
- **AD-6 — Startup chia 2 bước**: bước 1 (nhanh, an toàn) đưa `ensure_gitignore` + `settings.save` xuống background; bước 2 (riêng) offload `reload_all`/`scan_tree` sang background + pump kết quả qua `RequestsReloaded` event, có re-entrancy guard cho rescan. Không lazy-load từng request (giữ hành vi local-first: toàn bộ tree vẫn nạp về bộ nhớ, chỉ chuyển sang thread khác).
- **AD-7 — `list_version` tách khỏi `tree_version`** cho index O(1): chỉ bump khi danh sách requests thêm/bớt/sắp xếp lại, tránh rebuild index mỗi lần bump `tree_version` từ nhánh khác (vd. status update).
- **AD-8 — Không restyle, move verbatim**: các task P2 chỉ thay cách lấy dữ liệu (borrow/discriminant/Arc), diff phải không đổi style call.

## 4. Step-by-Step Implementation Tasks

### Phase P0 — Ngăn hang/freeze

**T1. QuickJS sandbox guard** *(S — core)*
- [ ] `crates/core/src/scripting/sandbox.rs`: tại nơi tạo `Runtime` — `set_memory_limit(256MB)`, `set_max_stack_size(1MB)`, `set_interrupt_handler` với `Arc<AtomicBool>` cancel flag + `Instant` deadline (`SCRIPT_TIMEOUT: Duration = 30s`, doc comment giải thích).
- [ ] Thêm `ScriptSandbox::cancel_flag()` trả clone `Arc<AtomicBool>`; pre/post script eval nhận flag.
- [ ] `crates/app/src/services/http_engine.rs`: đưa post-script vào phạm vi cancel — giữ flag, bật khi nhánh Cancel của `select!` thắng; post-script eval trả error "Script cancelled" thay vì treo. Áp dụng tương tự cho `pipeline.rs` stages nếu pre-script cũng nằm ngoài `select!`.
- [ ] Error type: eval bị interrupt → `RequestError`/script error có message rõ ràng ("script timed out after 30s" / "script cancelled").
- [ ] Tests (core): script `while(true){}` → error timeout; script cấp phát loop → error memory limit; script cancel flag → error cancelled. (Chạy trong `#[test]` với timeout ngắn — injectable deadline.)

**T2. Chuyển disk write response/cookies xuống background** *(S — app)*
- [ ] `runner.rs:269-277`: giữ `last_status_code` + `bump_tree_version` + `sync_cookies_in_memory()` trên UI thread; clone `Arc<dyn WorkspaceStore>`, `LocFile`, cookies → một task `cx.background_executor().spawn` làm lần lượt `save_response_cache` + `save_cookies`. Log/toast lỗi write (không panic).
- [ ] `workspace_crud.rs:354-359`: tách `sync_cookies_from_executor` thành `sync_cookies_in_memory` (UI) + background `save_cookies`; gọi lại tại `remove_cookie_internal` etc.
- [ ] Verify: đặt breakpoint/log — không còn `fs::write` trong path UI khi response về; test hiện có `runner` vẫn pass.

**T3. Cap format body** *(S — app)*
- [ ] `response_state.rs`: const `MAX_FORMAT_BODY_BYTES = 2MB`, `MAX_FORMAT_LINES = 2000`. `build_cached_format`: body > cap → skip `from_str`/`to_string_pretty`/XML format; line-split giới hạn 2000 dòng, append dòng marker "(truncated, N lines total)".
- [ ] Đảm bảo body_tab render raw oversized body vẫn hoạt động qua virtualized list (không rơi vào nhánh re-parse per frame — `body_tab.rs:174-199`).
- [ ] Test: body 3MB → `parsed_json` None, lines ≤ 2001, không panic; body nhỏ → hành vi giữ nguyên.

**T4. Giảm retry-sleep trên UI thread** *(S — core)*
- [ ] `serialize.rs with_retry`: giảm budget (vd 4 attempts × 10/20/40ms ≈ 70ms tối đa) + doc comment ghi rõ helper có thể chạy trên UI thread. (Chuyển toàn bộ folder/file mutation sang async = debt follow-up, xem §6.)

### Phase P1 — Hot path tương tác

**T5. Messages tab: virtualize + bỏ clone/lossy per frame** *(M — app)*
- [ ] `transport/frame.rs`: thêm `#[serde(skip)] pub preview: Option<String>` — engines/pump set preview (≤400 chars) khi build frame; `text_preview()` giữ cho nơi khác.
- [ ] `response_state.rs push_frame`: KHÔNG reset `scroll_handle` mỗi frame (chỉ tạo mới khi `frames` rỗng hoặc `restore_realtime_session`); giữ cap drain 1000.
- [ ] `messages_tab.rs`: thay `.children(messages.into_iter().map(...))` bằng `uniform_list` (pattern `body_tab.rs:542`) — item closure theo index đọc `resp_state.read(cx).frames[idx]`, render card từ `preview` (bỏ `String::from_utf8_lossy` full payload tại `:173`).
- [ ] Bỏ `frames.clone()` tại `:21` — borrow trong render qua các item closure.
- [ ] Verify thủ công: WS echo server stream 1000 frames — CPU không tăng dần (không còn O(n²)), scroll mượt, follow-latest vẫn hoạt động.
- [ ] Áp [[gpui-scrollable-noop-pitfall]]: nếu cần `scroll_to_item` phải track đúng div, scrollbar trên parent untracked.

**T6. Command palette: cache + notify có chủ đích** *(M — app)*
- [ ] `command_palette.rs`: thêm `RefCell<PaletteCache { key: Option<(String /*query_lower*/, u64 /*tree_version*/)>, folder_paths: HashMap<String,String>, results: Vec<...> }>` — build folder-path map 1 lần/cache key (O(F) với parent-map memo thay vì while-loop find mỗi request); filter tái dùng results khi key unchanged.
- [ ] Env list: clone `Vec<(String /*id*/, String /*name*/)>` 1 lần per open thay vì `environments.clone()` mỗi keystroke (`:145`).
- [ ] Thay `cx.refresh_windows()` (`:63-67`) bằng `self.modal_state.update(cx, |_, cx| cx.notify())`.
- [ ] Verify thủ công: workspace ≥300 requests + 50 folders — gõ từng ký tự, không thấy lag; kết quả đúng theo folder path.

**T7. Startup + rescan off main thread** *(L — app+core)*
- [ ] **Bước 7a**: `workspace_data_state.rs:91` + `:159` — `ensure_gitignore` chạy trong `cx.background_executor().spawn` (self-contained); `settings.add_recent_workspace` + save cũng spawn background (kết quả chỉ phục vụ recents UI).
- [ ] **Bước 7b**: tách load nặng — `WorkspaceDataState::new()` chỉ dựng store/scratch/metadata rỗng; thêm `load_workspace_async(store, path)` chạy `reload_all` + `reload_tree` trên background executor, pump kết quả (folders/requests/parse_errors/pins/meta) vào entity rồi emit `WorkspaceEvent::RequestsReloaded`; `AppView::new` spawn nó sau khi dựng entities. UI: sidebar hiển thị trạng thái loading (re-use footer/skeleton hiện có, không restyle).
- [ ] `open_workspace` dùng cùng đường 7b.
- [ ] **Bước 7c**: `watcher_reconciler.rs:164,215` — `reload_tree_preserving` chạy trên background executor với flag `rescan_in_flight` (bỏ qua/đánh dấu rescan-lại nếu có event mới trong lúc đang scan; scan lại sau khi xong); kết quả apply trong 1 update + emit `RequestsReloaded`.
- [ ] Rà soát consumer của `RequestsReloaded` (`workspace_event_handler.rs:100-123` `sync_inputs`) đảm bảo restore active request/env sau load async không phụ thuộc thứ tự khởi tạo đã bỏ (tham chiếu [[startup-restore-pre-subscription-events]]).
- [ ] Tests: giữ `new_for_path` cho unit tests; thêm test async load (tmp workspace, assert emit event + data đầy đủ); test watcher rescan guard (2 events liền nhau → 1 scan + 1 follow-up).

**T8. `find_request` O(1) index** *(M — app)*
- [ ] Audit toàn bộ call site `bump_tree_version` — tách `bump_list_version` chỉ gọi tại add/remove/move/reorder requests (`workspace_crud.rs`, `workspace_folder_crud.rs`, import, watcher apply).
- [ ] `WorkspaceDataState`: thêm `list_version: u64` + `request_index: HashMap<String, usize>` (lazy rebuild khi `list_version` đổi); `find_request`/`find_request_mut` tra index (fall back rebuild).
- [ ] Test: thêm/xoá/move request → index đúng; request không tồn tại → None; index rebuild sau watcher apply.

### Phase P2 — Clone per-frame (mechanical, không restyle)

**T9. BodyEditor đọc variant không clone** *(S)*
- [ ] `models/request.rs`: thêm `RequestBody::kind() -> BodyKind` (Copy enum: None/Json/FormUrlEncoded/Multipart/Raw/Binary/GraphQL...) — nếu cần thêm `content_type` trả `&str`/small clone.
- [ ] `body_editor/mod.rs:103-107`: dùng `kind()` + content_type thay `r.body.clone()`.

**T10. Titlebar env switcher** *(S)*
- [ ] `titlebar/menus.rs:210`: chỉ clone `Vec<(String, String)>` (id, name) + active id.

**T11. Console panel: virtualize + cap** *(M)*
- [ ] `response_state.rs`: cap `console_logs` ở 1000 entry (drain overflow, tương tự `push_frame`) tại nơi nhận logs từ script (`set_test_results`/`set_pre_request_logs`).
- [ ] `console_panel.rs`: bỏ clone toàn bộ (`:31-32`, `:194`) — `uniform_list` per-item read closure; bỏ clone `entry.message`.

**T12. gRPC schema Arc** *(S)*
- [ ] `editor_state`: đổi `grpc_schema: Option<GrpcSchemaSummary>` → `Option<Arc<GrpcSchemaSummary>>`; `grpc_message_tab.rs:37-42` clone Arc thay vì clone pool. Cập nhật producer/consumer (auto-fetch on send, validation).

**T13. Memoize inherited rows** *(S-M)*
- [ ] `inherited_headers.rs`/`headers_tab.rs`: cache `Vec<KvReadonlyRow>` keyed `(tree_version, request_id)` trong component hoặc `WorkspaceDataState` (RefCell cache, invalidate theo version) — re-walk hierarchy chỉ khi version đổi.

**T14. (Tuỳ chọn) request_sent_tab memoize markdown** *(S)*
- [ ] `escape_md` + `TextView::markdown` chuẩn bị 1 lần per response pointer (cache giống `CachedResponseFormat`), không phải per frame.

### Phase P3 — Vệ sinh

- [ ] **T15**: AGENTS.md §2.1 — bỏ `toast: Entity<ToastState>` (không còn tồn tại trong code), đếm lại số entity (8). Ghi chú entity list khớp `app_state.rs`.
- [ ] **T16**: Ghi debt registry `docs/technical-debt.md` các mục chủ động defer: KvTable borrow-based refactor (R6 params/headers clone), keystroke full-value clone (giới hạn API `value()` của gpui_kit), async conversion cho folder/file mutations (F6), startup full offload nếu 7b chưa đạt. Không tạo file secondary.

## 5. Risk Mitigation

| Rủi ro | Giảm thiểu |
|---|---|
| T1 interrupt handler làm hỏng script hợp pháp chạy lâu | Deadline 30s generous + injectable cho test; log warning khi interrupt; chỉ áp dụng tại eval boundary |
| T2 write background trễ hơn lần response sau | 1 task tuần tự per response; guard `Sending` chặn double-send; write idempotent snapshot |
| T3 cap làm mất pretty JSON cho body lớn | Raw view vẫn virtualized + marker cắt rõ ràng; đo lại trước khi cân nhắc offload async |
| T7b restore/subscription phụ thuộc thứ tự khởi tạo | Giữ event `RequestsReloaded` làm seam duy nhất; rà [[startup-restore-pre-subscription-events]]; test async load |
| T7c rescan background đua với UI mutation | re-entrancy guard + follow-up scan; fingerprint reconcile đã có sẵn chống self-echo |
| T5 scroll_to_item lệch index sau virtualize | Pattern đã ghi trong [[gpui-scrollable-noop-pitfall]]; test thủ công follow-latest |
| AD-5 notify qua modal_state gây re-render rộng hơn cần | Vẫn rẻ hơn `refresh_windows()`; nếu vẫn lag → convert Entity (follow-up) |
| Restyle vô ý khi sửa render | Mỗi task P2 diff so với `git show <commit>^` cho style calls ([[refactor-must-not-restyle-ui]]) |

## 6. Debt ghi nhận (không làm trong đợt này)

- KvTable borrow-based input (bỏ clone headers/params/gRPC payload per frame)
- Folder/file mutation write hoàn toàn async (F6 tận gốc)
- Keystroke full-value clone trong `subscriptions.rs` (chờ gpui_kit API)
- 7 file production >650 LOC (workspace_data_state 822, models/request 797, theme 730, env_autocomplete 717, environment_state 689, graphql/complete 681, sidebar/mod 654) — tách theo quy trình [[rust-impl-split-rules]] khi chạm tới
- KvTable reconcile-mutation trong render (đã guard, chấp nhận lệch rule nhỏ)

## 7. Verification Steps

Mỗi task: `cargo fmt --check` → `cargo test` → `cargo clippy --workspace --all-targets -- -D warnings` (0 warning).

Kiểm thử thủ công cuối phase:
1. **Streaming**: WS echo server 1000 frames → CPU flat, không hitch khi frame tới (T5)
2. **Body lớn**: response 20MB → nhận + switch tab không hitch >1 frame (T2/T3)
3. **Script treo**: `while(true){}` pre-script → error sau 30s; Cancel dừng post-script ngay (T1)
4. **Palette**: workspace 300+ requests, gõ nhanh không lag (T6)
5. **Folder ops**: create/rename/move/delete folder trong workspace lớn → không hitch dài (T7c, T4)
6. **Startup**: cold start workspace lớn → UI hiện sớm, không freeze >100ms trên main thread (T7a/b)
7. Regression: toàn bộ suite test + smoke các luồng HTTP/WS/gRPC/GraphQL/SSE send.

## 8. Commit Protocol

- Plan này commit cùng batch P0 đầu tiên; mỗi phase một commit (hoặc nhóm task nhỏ cùng chủ đề) với message tiếng Anh, dùng `git commit -F` ([[git-commit-message-backticks]]).
- Sau khi hoàn tất cả P0-P2: cập nhật `docs/technical-debt.md` (T16) và squo plan vào skill/spec theo [[agents-plan-storage-protocol]].
