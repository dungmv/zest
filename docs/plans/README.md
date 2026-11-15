# Kế hoạch Thực thi & Thiết kế (Implementation Plans)

Thư mục `docs/plans/` lưu trữ các bản kế hoạch triển khai tính năng và thiết kế kiến trúc cho từng tác vụ trong quá trình phát triển **Zest**.

---

## 📋 Quy ước đặt tên và cấu trúc (Plan Protocol)

Mọi kế hoạch mới tạo trong vòng phát triển hiện tại bắt buộc tuân thủ định dạng:

```text
docs/plans/YYYYMMDDSeq-<plan-name>.md
```

- **`YYYYMMDD`**: Ngày hiện tại theo giờ địa phương (ví dụ: `20261027`).
- **`Seq`**: Số thứ tự 2 chữ số tăng dần trong ngày (`01`, `02`, ..., `17`). Kế hoạch đầu tiên trong ngày là `01`.
- **`<plan-name>`**: Tên ngắn gọn dạng `kebab-case` mô tả tính năng/thay đổi (ví dụ: `2026102701-collection-runner-phase6.md`).

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
- Hệ thống skills tại [`.agents/skills/`](../../.agents/skills/)
- [`docs/technical-debt.md`](../technical-debt.md)

---

## 🗂️ Lưu trữ Vòng 2 (Cycle 2 Synthesis — 2026-10-27 → 2026-11-15)

Toàn bộ **28 bản kế hoạch của Cycle 2** đã hoàn tất 100%. Kiến thức bền vững được "squash" vào skills và specs; file plan gốc được dọn khỏi thư mục (khôi phục được qua git history). Bản đồ tổng hợp:

### Skills (`.agents/skills/`)
| Skill | Kiến thức tiếp nhận từ Cycle 2 |
| :--- | :--- |
| `gpui-architecture` | Migration 24/24 component `gpui-kit` (Button/dropdown, ModalDialog, Tag/Spinner/Skeleton/Progress/Empty/Alert, Button hover panic), theme token centralization + `sync_component_theme` full-sync, FocusHandle + global shortcut dispatch (Ctrl+K `ToUnicode 0x0B`), Windows titlebar drag-region `.occlude()` + Popup caveat, fixed-height modals, toggle icon-tint, sidebar pinned/multi-select/`DragMultiItem`, `RetokenizeGuard` + caret restore + lean scanner token attach, native Tooltip cho token hover, umbrella `completion/` + `request_composer/` + `common/`, sort-at-state-layer, API quirks gpui-kit 0.9.x |
| `http-pipeline-engine` | Tokio runtime bridge (`global_tokio_runtime` + oneshot, không reactor trên GPUI executor) |
| `local-first-storage-vcs` | Slug ASCII qua `deunicode`, tên hiển thị giữ nguyên, slug theo segment |
| `interop-spec-converters` | `split_folder_segments`/`push_folder_chain`, form body import (MIME chuẩn hóa, schema đệ quy, `is_file`, Swagger 2 `formData`), `ApiKeyLocation` mapping, layout `importers/openapi/` |
| `tech-debt-auditor` | Playbook refactor (inherent vs trait impl E0119, `#[path]`, raw-string-aware dedent, move verbatim, umbrella modules), test isolation khỏi user config thật, archive TD-201..206 |

### Specs (`docs/specs/`)
| Spec | Nội dung cập nhật |
| :--- | :--- |
| `01-import-export-compatibility` | Trạng thái form body import, slug ASCII + folder lồng nhau, `ApiKeyLocation`, layout module `openapi/` |
| `03-protocols-graphql-websocket-grpc` | WebSocket ✅ & gRPC ✅ (send pipeline hợp nhất, `TransportFrame`, reflection, validation diagnostics); gap: WS binary, gRPC client-streaming/bidi, SSE |
| `04-advanced-auth-and-inheritance` | `ApiKeyLocation` (YAML `add_to: header\|query`), inheritance ✅ qua `WorkspaceSettingModal` + `transport::hierarchy` |
| `07-ui-ux-enhancements` | §6 tổng hợp 10 nhóm tính năng UI Cycle 2 (sidebar, palette, titlebar, modals, env, inspector, git, body, script editor, about/update) |

### Vệ sinh & Kiến trúc
- **Audit 2026-11-12** (`TD-201..206`): toàn bộ giải quyết — modular hóa ~20 file vượt 650 LOC, gom state struct ≤ 15 fields, xoá module-level `#![allow(...)]`, không I/O trong render, git subprocess có timeout. Chi tiết tại [`docs/technical-debt.md`](../technical-debt.md).
- **Protocol source cleanup** (2026-11-15): umbrella `zest_core::transport` (ResolvedRequest, hierarchy, `TransportFrame`), MỘT send pipeline (`Runner::send_current_request` + `SendEvent` + `services/pipeline.rs`), gRPC interpolation + frame-log persistence.
- **Watcher typing race** (2026-11-03): `DiskFingerprints` + `AtomicWriter` thay time-based suppression; dirty-aware reload (`reload_tree_preserving`, `apply_disk_changes`, conflict banner).
- Nền tảng mới: `zest_core::updater` (GitHub Releases), `version.rs` + `build.rs` version injection, AppSettings test isolation, about modal/update checker.
