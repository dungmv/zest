# Danh mục Nợ Kỹ Thuật (Technical Debt Registry)

> **Mục đích:** Tài liệu duy nhất (Single Source of Truth) theo dõi các khoản nợ kỹ thuật **đang mở** và **đang defer có chủ đích** trong codebase `zest` (`zest-core` và `zest`). Các khoản đã xử lý xong sẽ được gỡ bỏ khỏi tài liệu này (lịch sử lưu trong git).

---

## 1. Nợ kỹ thuật đang mở

Hiện không có khoản nợ kỹ thuật nào đang mở (toàn bộ 9 mục `TD-401..409` từ Audit 2026-11-25 đã được xử lý ngày 2026-11-27).

---

## 2. Nợ hiệu năng & thiết kế đã chủ động defer

Các khoản dưới đây đã được đánh giá và chủ động trì hoãn vì chi phí thực tế thấp, phạm vi churn không tương xứng, hoặc phụ thuộc API bên ngoài:

| ID | Vấn đề | Lý do defer | Gợi ý xử lý |
| :--- | :--- | :--- | :--- |
| **TD-301** | `inherited_readonly_rows` re-walk hierarchy + re-mask secrets per frame khi Headers tab mở | Chi phí thực tế thấp (O(depth×F) ~ vài trăm iter); cache-key đúng đắn phải bao gồm header keys + auth discriminant (header edits không bump `tree_version`) — phức tạp hơn lợi ích | Memoize với key `(tree_version, req.id, folder_id, header-keys, auth-discriminant)` nếu workspace ~hàng nghìn folder |
| **TD-302** | `request_sent_tab` build `escape_md` + `TextView::markdown` per frame | Chi phí bounded (URL + headers nhỏ); `TextView` có element id, có thể tự cache nội bộ | Memoize per response pointer (`CachedResponseFormat` pattern) nếu thấy hitch |
| **TD-303** | `KvTable` nhận owned `Vec<KeyValuePair>` → headers/params/gRPC payload clone per frame ở composer tabs | Cần refactor borrow-based input cho `KvTable` (chạm nhiều component) | `KvTable` chấp nhận `&[KeyValuePair]` hoặc `Arc`-share rows |
| **TD-304** | Mỗi keystroke clone full editor value (`value().to_string()`) trong `subscriptions.rs` handlers | Giới hạn API `gpui_kit` `InputState` (`value()` trả owned `String`) | Đẩy lên `gpui-kit`: API borrow/view hoặc callback nhận `&str` |
| **TD-305** | Folder/file mutation vẫn ghi đĩa sync trên UI thread (đã thu hẹp retry budget) | Chuyển async cần đảm bảo thứ tự với watcher fingerprint staging | Đưa qua background executor per call site, giữ staging-before-write |
| **TD-306** | Re-validate toàn bộ gRPC payload + variables JSON mỗi keystroke (`subscriptions.rs`) | Body nhỏ: chấp nhận được; body lớn mới thấy | Debounce theo độ dài payload (vd >64KB) hoặc validate on-idle |
| **TD-406b** | `RequestItem` DTO có 19 flat fields (`crates/core/src/models/request.rs`) | Domain DTO chuẩn được truy cập trực tiếp ở >75 call sites xuyên suốt `core` và `app`; tách/flatten sẽ gây churn lớn mà không cải thiện tính đóng gói | Gom `grpc`/`graphql`/`ws_message_format` thành enum `ProtocolPayload` nếu thêm protocol mới |

---

## 3. Quy trình bảo trì (Maintenance Protocol)

1. **Một nguồn sự thật duy nhất:** Mọi khoản nợ kỹ thuật mới đều ghi trực tiếp vào `docs/technical-debt.md` (không tạo file phụ).
2. **Dọn gọn sau khi xử lý:** Khi một mục đã được giải quyết xong, xoá hẳn khỏi danh sách đang mở thay vì tích luỹ mục lưu trữ dài.
3. **Kiểm chứng chống hồi quy:** Mọi PR/commit xử lý nợ kỹ thuật phải kèm unit test phù hợp và vượt qua đầy đủ pipeline:
   - `cargo fmt --check`
   - `cargo test --workspace`
   - `cargo clippy --workspace --all-targets -- -D warnings`
