# Danh mục Nợ Kỹ Thuật (Technical Debt Registry)

> **Mục đích:** Tài liệu duy nhất (Single Source of Truth) theo dõi các khoản nợ kỹ thuật **đang mở** và **đang defer có chủ đích** trong codebase `zest` (`zest-core` và `zest`). Các khoản đã xử lý xong sẽ được gỡ bỏ khỏi tài liệu này (lịch sử lưu trong git).

---

## 1. Nợ kỹ thuật đang mở (Audit 2026-11-25)

### 1.1 Ưu tiên cao (P1 — Kiến trúc, Hiệu năng, Xử lý lỗi)

| ID | Vấn đề | Vị trí | Gợi ý xử lý |
| :--- | :--- | :--- | :--- |
| **TD-401** | Sync Disk I/O trong `render()` (`std::fs::metadata` mỗi 5s trên UI thread) + `thread_local! FILE_SIZE_CACHE: HashMap` không có giới hạn/eviction | `crates/app/src/components/body_editor/binary.rs` (L15–61) | Chuyển `metadata` sang background task khi chọn file hoặc đổi request; lưu kết quả vào state hoặc bounded cache. |
| **TD-402** | Nuốt lỗi ghi file âm thầm (`let _ = std::fs::write(path, &bytes)`) khi lưu response ra đĩa | `crates/app/src/components/response_inspector/body_tab.rs` (L484–488) | Bắt `Err` và hiển thị thông báo lỗi cho người dùng (toast/status badge). |
| **TD-403** | Thiếu `#[serde(deny_unknown_fields)]` trên 8 local storage models (`LocRequestInfo`, `LocResponseInfo`, `LocFile`, `PinsFile`, `SidebarState`, `LayoutState`, `PerRequestState`, `LocalState`) | `crates/core/src/storage/models.rs` (L350–534) | Bổ sung `#[serde(deny_unknown_fields)]` theo §2.6 `AGENTS.md`, hoặc ghi doc comment rõ nếu chủ đích giữ tương thích ngược cho `.local/`. |

### 1.2 Ưu tiên trung bình (P2 — Modularity, Bloat & Duplication)

| ID | Vấn đề | Vị trí | Gợi ý xử lý |
| :--- | :--- | :--- | :--- |
| **TD-404** | Trùng lặp lớn ở `render_auth_tab` (555 vs 560 LOC) và helper đặt nhãn `Auth` bị viết lặp ở 3 nơi (`auth_kind_label`, `auth_type_label`, match inline) | `crates/app/src/components/request_composer/auth_tab.rs`, `crates/app/src/modals/workspace_setting_modal/auth_tab.rs` | Tách form Auth dùng chung (Bearer/Basic/API Key/Location), chỉ truyền callback lưu khác nhau giữa Request và Folder/Workspace; dùng chung 1 helper nhãn. |
| **TD-405** | File vượt ngưỡng 650 LOC: production (`workspace_data_state.rs` 1,068, `response_state.rs` 876, `models/request.rs` 823, `request_composer/auth_tab.rs` 705, `graphql/complete.rs` 681, `body_tab.rs` 655, `sidebar/mod.rs` 654) và inline `#[cfg(test)]` (`scripting/sandbox.rs` 743 / 638 test, `environment_state.rs` 699 / 143 test) | `crates/app/src/state/`, `crates/app/src/components/`, `crates/core/src/` | Tách inline test sang `*_tests.rs`; tách các file production vượt ngưỡng theo module con. |
| **TD-406** | Struct vượt ngưỡng 15 fields: `ResponseState` (22), `SidebarRenderContext` (20), `WorkspaceDataState` (19), `RequestItem` (19), `ComposerInputs` (16) | `response_state.rs`, `sidebar/mod.rs`, `workspace_data_state.rs`, `models/request.rs`, `composer_inputs.rs` | Gom nhóm field liên quan thành sub-struct (vd: realtime/WS/gRPC state & popover flags trong `ResponseState`; cờ `load_*`/`rescan_*` trong `WorkspaceDataState`; 5 auth inputs thành `AuthComposerFields`). |
| **TD-407** | Hàm quá dài (> 450 LOC): `render_body_tab` (637), `render_auth_tab` (560 / 555), `setup_app_subscriptions` (548), `render_folder_row` (520), `BodyEditor::render` (497), `render_request_inner_pill` (494) | `body_tab.rs`, `auth_tab.rs`, `subscriptions.rs`, `sidebar/`, `body_editor/mod.rs` | Tách thành các helper theo từng section/toolbar/tab nhỏ. |

### 1.3 Ưu tiên nhẹ (P3 — Clean-up)

| ID | Vấn đề | Vị trí | Gợi ý xử lý |
| :--- | :--- | :--- | :--- |
| **TD-408** | Một số `#[allow(dead_code)]` / `#[allow(unused_mut)]` chưa có comment giải thích (hoặc có thể xoá nếu không còn cần) | `script_editor/completion.rs:27`, `status_badge.rs:72`, `request_session_state.rs:278,292`, `theme.rs:52,461`, `titlebar/mod.rs:286` | Xoá nếu dư thừa, hoặc thêm comment giải thích ngắn theo §3.1 `AGENTS.md`. |
| **TD-409** | Mẫu `if x.is_string() { x.as_str().unwrap() }` thừa `.unwrap()` | `crates/core/src/importers/openapi/sample.rs` (L6–17) | Thay bằng `if let Some(s) = x.as_str()`. |

---

## 2. Nợ hiệu năng đã chủ động defer

Các khoản dưới đây đã được đánh giá trong đợt hiệu chuẩn hiệu năng (2026-11-22/23) và chủ động trì hoãn vì chi phí thực tế thấp hoặc phụ thuộc API bên ngoài:

| ID | Vấn đề | Lý do defer | Gợi ý xử lý |
| :--- | :--- | :--- | :--- |
| **TD-301** | `inherited_readonly_rows` re-walk hierarchy + re-mask secrets per frame khi Headers tab mở | Chi phí thực tế thấp (O(depth×F) ~ vài trăm iter); cache-key đúng đắn phải bao gồm header keys + auth discriminant (header edits không bump `tree_version`) — phức tạp hơn lợi ích | Memoize với key `(tree_version, req.id, folder_id, header-keys, auth-discriminant)` nếu workspace ~hàng nghìn folder |
| **TD-302** | `request_sent_tab` build `escape_md` + `TextView::markdown` per frame | Chi phí bounded (URL + headers nhỏ); `TextView` có element id, có thể tự cache nội bộ | Memoize per response pointer (`CachedResponseFormat` pattern) nếu thấy hitch |
| **TD-303** | `KvTable` nhận owned `Vec<KeyValuePair>` → headers/params/gRPC payload clone per frame ở composer tabs | Cần refactor borrow-based input cho `KvTable` (chạm nhiều component) | `KvTable` chấp nhận `&[KeyValuePair]` hoặc `Arc`-share rows |
| **TD-304** | Mỗi keystroke clone full editor value (`value().to_string()`) trong `subscriptions.rs` handlers | Giới hạn API `gpui_kit` `InputState` (`value()` trả owned `String`) | Đẩy lên `gpui-kit`: API borrow/view hoặc callback nhận `&str` |
| **TD-305** | Folder/file mutation vẫn ghi đĩa sync trên UI thread (đã thu hẹp retry budget) | Chuyển async cần đảm bảo thứ tự với watcher fingerprint staging | Đưa qua background executor per call site, giữ staging-before-write |
| **TD-306** | Re-validate toàn bộ gRPC payload + variables JSON mỗi keystroke (`subscriptions.rs`) | Body nhỏ: chấp nhận được; body lớn mới thấy | Debounce theo độ dài payload (vd >64KB) hoặc validate on-idle |

---

## 3. Quy trình bảo trì (Maintenance Protocol)

1. **Một nguồn sự thật duy nhất:** Mọi khoản nợ kỹ thuật mới đều ghi trực tiếp vào `docs/technical-debt.md` (không tạo file phụ).
2. **Dọn gọn sau khi xử lý:** Khi một mục đã được giải quyết xong, xoá hẳn khỏi danh sách đang mở thay vì tích luỹ mục lưu trữ dài.
3. **Kiểm chứng chống hồi quy:** Mọi PR/commit xử lý nợ kỹ thuật phải kèm unit test phù hợp và vượt qua đầy đủ pipeline:
   - `cargo fmt --check`
   - `cargo test --workspace`
   - `cargo clippy --workspace --all-targets -- -D warnings`
