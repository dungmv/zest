# Danh mục Nợ Kỹ Thuật Tập Trung (Technical Debt Registry)

> **Mục đích:** Tài liệu duy nhất và chính thức ghi nhận toàn diện các khoản nợ kỹ thuật (Technical Debt), các anti-pattern, điểm nghẽn kiến trúc và danh sách theo dõi modularization trong codebase `zest` (`zest-core` và `zest`). Mọi cập nhật về nợ kỹ thuật bắt buộc phải ghi nhận trực tiếp vào tài liệu này.

---

## 📊 1. Bảng tổng hợp trạng thái hiện tại (Cycle 2 — Active Watchlist)

Toàn bộ **31/31 khoản nợ kỹ thuật của Vòng 1 (Cycle 1)** cùng các đợt đại phẫu kiến trúc (Typed Event Emitter, Unified Write-Behind Persistence, GitService Extraction, External Change Reconciliation) đã hoàn thành 100%.

Dưới đây là danh sách theo dõi (Watchlist) cho **Vòng phát triển mới (Cycle 2)**, tập trung vào việc loại bỏ các thuộc tính `#![allow(...)]` cấp module và chia nhỏ các component lớn vượt ngưỡng `~650 LOC` khi phát triển tính năng mới:

| ID | Vấn đề / Khoản nợ | Khu vực ảnh hưởng | Mức độ | Độ phức tạp | Trạng thái |
| :--- | :--- | :--- | :---: | :---: | :---: |
| **TD-201** | Module-wide `#![allow(dead_code)]` & `#![allow(unused_imports)]` | `crates/app/src/state/mod.rs`, `crates/app/src/components/mod.rs` | 🟡 **Medium** | Thấp | 📋 Theo dõi (Cycle 2) |
| **TD-202** | `scope_settings_modal.rs` vượt ngưỡng 650 LOC (~1.515 LOC) | `crates/app/src/components/scope_settings_modal.rs` | 🔵 **Low** | Trung bình | 📋 Theo dõi (Cycle 2) |
| **TD-203** | `script_editor.rs` vượt ngưỡng 650 LOC (~1.443 LOC) | `crates/app/src/components/script_editor.rs` | 🔵 **Low** | Trung bình | 📋 Theo dõi (Cycle 2) |
| **TD-204** | `response_inspector.rs` vượt ngưỡng 650 LOC (~1.334 LOC) | `crates/app/src/components/response_inspector.rs` | 🔵 **Low** | Trung bình | 📋 Theo dõi (Cycle 2) |
| **TD-205** | `env_modal.rs` (~1.050 LOC) & `import_modal.rs` (~934 LOC) vượt ngưỡng 650 LOC | `crates/app/src/components/` | 🔵 **Low** | Trung bình | 📋 Theo dõi (Cycle 2) |
| **TD-206** | `#[allow(clippy::too_many_arguments)]` còn sót ở sidebar & command palette | `crates/app/src/components/sidebar/`, `command_palette.rs` | 🔵 **Low** | Thấp | 📋 Theo dõi (Cycle 2) |

### Hướng dẫn xử lý cho Cycle 2:
- **TD-201:** Gỡ bỏ `#![allow(dead_code)]` ở đầu `crates/app/src/state/mod.rs` và `#![allow(unused_imports)]` ở `crates/app/src/components/mod.rs`; dọn dẹp các hàm/re-export không còn sử dụng hoặc gắn `#[allow(dead_code)]` có chú thích tại đúng hàm dự phòng.
- **TD-202..TD-205:** Khi bổ sung tính năng mới vào các modal/component này (như OAuth 2.0 trong `scope_settings_modal`, Insomnia/HAR trong `import_modal`, JSONPath/Diff trong `response_inspector`), thực hiện tách thành thư mục module con (`components/<name>/mod.rs` + các tab/sub-views) tương tự như `components/body_editor/` và `components/settings_modal/`.
- **TD-206:** Gom các tham số rời rạc vào struct ngữ cảnh (theo mẫu `AuthTabContext<'a>` và `ModalOverlayEntities<'a>`).

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
   - Theo dõi chọn lọc `.git/` (`FsEvent::GitChanged`) và gom lô sự kiện watcher (debounce 100ms–200ms), dùng duy nhất 1 luồng dọn `suppressed_paths` (`workspace-suppress-cleaner`).
5. **Tối ưu Mạng, Bộ nhớ & Schema (`PERF-001..004`, `PERF-007..010`, `SL-002`, `SL-005..007`, `TD-005..017`):**
   - Chia sẻ connection pool trong `ReqwestExecutor`, xoá bỏ TCP probe thừa.
   - Cache phản hồi giới hạn kép `LruMemoryCache` (100 entries / 50MB RAM) bọc trong `Arc<HttpResponseRecord>`, kèm cache định dạng `Pretty`/`Raw` (`CachedResponseFormat`).
   - Áp dụng `#[serde(deny_unknown_fields)]` trên toàn bộ YAML models, vòng lặp an toàn chu kỳ cho OpenAPI `$ref` (`depth < 32`), và chuẩn hoá 100% comment tiếng Anh trong source code.

---

## 🛠️ 3. Quy trình quản lý Nợ Kỹ Thuật (Maintenance Protocol)

1. **Một nguồn sự thật duy nhất (Single Source of Truth):** Mọi khoản nợ kỹ thuật phát hiện mới hoặc cập nhật trạng thái đều ghi trực tiếp vào [`docs/technical-debt.md`](file:///Users/dungmv/Documents/macos-native-apps/zest/docs/technical-debt.md). Không tạo thêm các file phụ (`technical-debt-02.md`, v.v.).
2. **Kèm Unit Test chống hồi quy:** Khi giải quyết xong bất kỳ mục nợ kỹ thuật nào, bắt buộc bổ sung unit test kiểm chứng và chạy xanh toàn bộ pipeline (`cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`).
