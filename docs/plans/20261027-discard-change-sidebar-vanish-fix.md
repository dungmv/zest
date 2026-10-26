# Kế hoạch: Sửa lỗi request biến mất khỏi sidebar khi Discard Changes

- **Date**: 2026-10-27
- **Status**: Implemented (chưa commit — chờ user chạy thử xác nhận)
- **Scope**: `zest-core::git`, `zest-core::storage::watcher`, `zest` app (`services/git_service.rs`, `components/sidebar/request_node.rs`, `components/git/commit_modal.rs`, `state/persistence_state.rs`)

## Goal

Sửa lỗi: request **đã commit** có thay đổi, chuột phải → "Discard Changes" thì
request biến mất khỏi sidebar, nhưng tắt app mở lại thì hiện lại bình thường
(file trên đĩa không bao giờ bị mất).

Nhân tiện hardening một lỗ hổng phá dữ liệu liên quan phát hiện trong quá trình
chẩn đoán: sidebar discard không được phép xóa file request chưa từng commit.

## Context — Root Causes

### Root cause chính (lỗi được report): watcher delete-vs-replace race

1. Khi discard, `GitService::discard_file` chạy `git restore` **bất đồng bộ** ở
   background executor. Trên Windows, `git restore` thay thế file theo kiểu
   **remove + tạo lại**, nên watcher (notify) phát ra **cả `RequestDeleted` lẫn
   `RequestChanged` cho cùng một rel_path**.
2. Debouncer trong `crates/core/src/storage/watcher.rs` giữ event trong
   `HashSet<FsEvent>` — hai variant cho cùng một path có thể cùng được drain
   trong **một batch với thứ tự tùy hứng**, hoặc tách thành **hai batch** nếu
   khoảng cách remove/create vượt cửa sổ debounce 200 ms (ví dụ antivirus scan
   file tạm).
3. Trong `PersistenceState::handle_fs_events_batch`, nếu `RequestDeleted` được
   xử lý trước thì `RequestChanged` bị nuốt (`if !req_deleted.contains`), sau
   đó nhánh `req_deleted` chạy `ws.requests.retain(...)` — **request bị loại
   khỏi memory trong khi file vẫn nguyên trên đĩa**.
4. Đây là đường **duy nhất** có thể loại một row sidebar mà file vẫn tồn tại.
   Mở lại app quét đĩa từ đầu nên request "hồi phục" — khớp chính xác triệu
   chứng report.
5. Lỗi này không chỉ xảy ra với discard: **mọi atomic replace bên ngoài**
   (editor lưu bằng save-by-rename, v.v.) đều có thể trigger cùng race.

### Root cause phụ (hardening): sidebar discard xóa file untracked

`zest_core::git::git_discard_file` áp dụng git-clean semantics: file không tồn
tại trong HEAD (untracked / staged-added) bị **xóa khỏi đĩa**. Sidebar hiển thị
"Discard Changes" cho cả request mới (chấm xanh) nên discard một request chưa
commit xóa hẳn file — phá dữ liệu người dùng, sau đó `reload_all()` làm request
biến mất. Ngữ nghĩa xóa này chỉ phù hợp cho git panel (chuẩn git GUI như VS
Code), không phù hợp cho sidebar nơi người dùng quản lý dữ liệu workspace.

## Architectural Decisions

1. **Filesystem là nguồn sự thật tại thời điểm xử lý event.** Event xóa chỉ
   được tin khi file thực sự không còn trên đĩa; replace bên ngoài phải hiển
   thị như một change, không phải deletion.
2. **Sửa ở cả hai tầng watcher và app handler** (defense in depth):
   - Core watcher tự hạ cấp `Deleted` → `Changed` khi file còn tồn tại tại thời
     điểm drain — mọi consumer tương lai đều nhận event đúng.
   - App handler vẫn tự xác minh lại deletion với đĩa, vì core watcher không thể
     xử lý case remove/create **tách batch** (Deleted phát ra khi file thật sự
     chưa được tạo lại; `Changed` đến ở batch sau).
3. **Request được tạo lại phải được thêm lại bằng rescan toàn cây**
   (`reload_tree()`), không insert thủ công — để thứ tự (`_folder.yaml` order
   list), folder parent và slug được scan_tree xử lý nhất quán.
4. **Tách hai ngữ nghĩa discard bằng cờ `delete_untracked`**: `git_discard_file`
   giữ hành vi git-clean (xóa file không có trong HEAD) khi cờ `true` — chỉ git
   panel dùng; mọi caller quản lý dữ liệu workspace (sidebar) phải truyền
   `false` — khi `!in_head && !delete_untracked` là no-op an toàn, kể cả khi
   git status bên sidebar đã stale.
5. **Sidebar discard vẫn giữ vai trò revert chỉnh sửa chưa lưu**: với file
   untracked, git call là no-op; việc revert composer về nội dung lần lưu cuối
   do `mark_clean` (hủy write-behind pending) + `reload_all` đảm nhiệm — không
   đổi flow hiện có của handler.

## Step-by-Step Implementation Tasks

1. ✅ **Core git**: `git_discard_file(repo_dir, file_path, delete_untracked)` —
   kiểm tra `cat-file -e HEAD:<path>` trước; `!in_head && !delete_untracked` →
   return sớm giữ nguyên file; khối git-clean/remove_file chỉ chạy khi
   `delete_untracked` (crates/core/src/git/mod.rs:497).
2. ✅ **GitService**: `discard_file(path, delete_untracked, cx)` truyền cờ xuống
   (crates/app/src/services/git_service.rs:158).
3. ✅ **Sidebar**: context menu "Discard Changes" truyền `false` — comment giải
   thích việc giữ file chưa commit (crates/app/src/components/sidebar/request_node.rs:288).
4. ✅ **Git commit modal**: nút ↺ truyền `true` — giữ git-clean semantics
   (crates/app/src/components/git/commit_modal.rs:251).
5. ✅ **Core watcher**: thêm `downgrade_existing_deletions(pending, root)` gọi
   tại cả hai điểm drain (Timeout + Disconnected): `RequestDeleted(rel)` có
   `root/collections/rel` tồn tại → thay bằng `RequestChanged(rel)`
   (crates/core/src/storage/watcher.rs).
6. ✅ **App handler**: trong `handle_fs_events_batch`, sau khi phân loại, xác
   minh từng `req_deleted` với `workspace_path/collections/<rel>`: file còn →
   `tree_changed = true` + đưa về `req_changed` (giữ conflict detection); file
   mất thật → giữ nguyên đường xóa (crates/app/src/state/persistence_state.rs).
7. ✅ **App handler**: nhánh per-file update — `RequestChanged` cho rel_path
   **vắng mặt** trong `ws.requests` (đã bị batch xóa trước đó, file được tạo
   lại sau) → `needs_rescan` → `reload_tree()` + emit
   `WorkspaceEvent::RequestsReloaded { req_ids: None }`.
8. ✅ **Tests**:
   - Core: `test_existing_deletions_downgraded_to_changes` (watcher.rs) — file
     còn → Changed, file mất → giữ Deleted.
   - Core: cập nhật `test_git_operations_lifecycle` assert cả hai ngữ nghĩa:
     cờ `false` giữ file untracked + nội dung nguyên vẹn; cờ `true` xóa.
   - App: `test_fs_batch_recreated_request_not_dropped`
     (persistence_tests.rs) — replace cùng batch không mất row; xóa thật loại
     khỏi memory; tạo lại tách batch được re-add qua rescan.

## Risk Mitigation

- **Không thay đổi hành vi git panel**: cờ `true` bảo toàn 100% logic cũ
  (restore thất bại + clean fallback + remove_file có canonicalize guard).
- **Không chạm vào write-behind coordinator**: `mark_clean` vẫn là cơ chế hủy
  pending write; các edit chưa lưu trước discard vẫn bị hủy đúng như cũ.
- **Rescan hiếm và rẻ**: `reload_tree()` chỉ chạy khi request thực sự vắng mặt
  trong memory hoặc file deleted-vẫn-còn; đường per-file update thông thường
  (external edit) không đổi.
- **Event sai không còn phát ra từ nguồn**: downgrade ở core watcher ngăn
  consumer tương lai lặp lại cùng lỗi phân loại.

## Verification Steps

```bash
cargo fmt --check                                   # OK
cargo test                                          # 95 app + 84 core — OK
cargo clippy --all-targets -- -D warnings           # 0 warnings — OK
```

Kiểm thử thủ công: tạo request → commit → sửa nội dung → chuột phải → Discard
Changes → request phải còn trong sidebar với nội dung HEAD; git panel (↺) trên
file untracked vẫn xóa như git GUI chuẩn.
