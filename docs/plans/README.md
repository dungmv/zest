# Kế hoạch Thực thi & Thiết kế (Implementation Plans — Cycle 2)

Thư mục `docs/plans/` lưu trữ các bản kế hoạch triển khai tính năng và thiết kế kiến trúc cho từng tác vụ trong quá trình phát triển **Zest**.

---

## 📋 Quy ước đặt tên và cấu trúc (Plan Protocol)

Mọi kế hoạch mới tạo trong vòng phát triển hiện tại bắt buộc tuân thủ định dạng:

```text
docs/plans/YYYYMMDDSeq-<plan-name>.md
```

- **`YYYYMMDD`**: Ngày hiện tại theo giờ địa phương (ví dụ: `20261027`).
- **`Seq`**: Số thứ tự 2 chữ số tăng dần trong ngày (`01`, `02`, ..., `17`). Kế hoạch đầu tiên trong ngày là `01`.
- **`<plan-name>`**: Tên ngắn gọn dạng `kebab-case` mô tả tính năng/thay đổi (ví dụ: `2026102701-collection-runner-phase6.md`, `2026102702-insomnia-v4-importer.md`).

### Cấu trúc chuẩn của mỗi file Plan:
1. **Goal**: Mục tiêu cần đạt được và phạm vi tác động.
2. **Context**: Ngữ cảnh hiện tại, tham chiếu tới `docs/specs/` hoặc `docs/technical-debt.md`.
3. **Architectural Decisions**: Các quyết định thiết kế, luồng sự kiện (`WorkspaceEvent` / `SessionEvent`), và chiến lược lưu trữ (`PersistenceCoordinator`).
4. **Step-by-Step Implementation Tasks**: Các bước thực thi nhỏ, rõ ràng.
5. **Risk Mitigation**: Các rủi ro biên và phương án phòng tránh.
6. **Verification Steps**: Các bài test và lệnh kiểm tra (`cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`).

---

## 🗂️ Lưu trữ Vòng 1 (Cycle 1 Synthesis)

Toàn bộ 24 bản kế hoạch của **Cycle 1** (gồm phân rã `SL-001`, Typed `EventEmitter`, `PersistenceCoordinator` write-behind, tách `GitService`, đồng bộ `git discard` & external changes, `EnvInput` autocomplete/token highlight, kế thừa Headers/Auth UI, tối ưu `flat_tree` sidebar, và giải quyết 31/31 khoản nợ kỹ thuật) đã hoàn tất 100% và được tổng hợp trực tiếp vào:
- [`AGENTS.md`](../../AGENTS.md)
- Hệ thống 6 skills tại [`.agents/skills/`](../../.agents/skills/)
- [`docs/technical-debt.md`](../technical-debt.md)
