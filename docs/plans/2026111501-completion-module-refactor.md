# Plan: Refactor module Auto Completion — umbrella `components/completion/`

- **Date**: 2026-11-15 (seq 01)
- **Scope**: `crates/app/src/components/`
- **Type**: Pure structural refactor — behavior giữ nguyên 100%

## Goal

Tách module Auto Completion (hiện là 1 file `proto_json_completion.rs` 507 LOC trộn 4 mối quan tâm) thành folder umbrella `components/completion/` gồm: `env_autocomplete/` (chuyển vào nguyên vẹn), `proto_json/` (provider + JSON walker + descriptor helpers), `validation.rs` (validate + gutter/diagnostics).

## Context

- Hiện trạng: `crates/app/src/components/proto_json_completion.rs` (507 LOC) + `proto_json_completion_tests.rs` (261 LOC) trộn: CompletionProvider cho gRPC JSON keys, JSON prefix walker thuần (`locate_message_context`), proto descriptor helpers (`type_label`, `scalar_label`), và validation/diagnostics (`validate_message`, `refresh_grpc_validation`) — validation không phải completion.
- 3 CompletionProvider đang rải rác: `env_autocomplete/` (mod 612 + render 203 + var_tokens 392 LOC), `proto_json_completion` (507), `script_editor/completion.rs` (giữ nguyên — là editor module hoàn chỉnh theo TD-203).
- Dup thật: `env_trigger_context` (proto_json_completion.rs:130-138) ≡ inline check trong `EnvCompletionProvider::completions` (env_autocomplete/mod.rs:350-352) — cùng điều kiện `}}` | `\n` | `"`.
- Wiring: provider attach tại composer_inputs.rs:74-85; `refresh_grpc_validation` gọi từ subscriptions.rs:201, grpc_message_tab.rs:201, app/mod.rs:~255; schema đọc từ `session_state.grpc_schema.pool`.
- components/mod.rs: cả `env_autocomplete` lẫn `proto_json_completion` KHÔNG được glob re-export → ít churn.

## Architectural Decisions

1. **Umbrella `components/completion/mod.rs`** (~15 LOC): `pub mod env_autocomplete; pub mod proto_json; pub mod validation;` + re-export chọn lọc (`ProtoJsonCompletionProvider`, `validate_message`, `refresh_grpc_validation`), không glob.
2. **`env_autocomplete/` giữ nguyên tên + nội dung** (3 file), chỉ đổi vị trí qua `git mv` (giữ history) + thêm 1 helper dùng chung.
3. **Shared helper `unclosed_env_span(prefix: &str) -> Option<usize>`** trong env_autocomplete/mod.rs: trả về byte offset của `{{` chưa đóng theo semantics JSON-editor (`}}` | `\n` | `"` chấm dứt). Cả `EnvCompletionProvider::completions` và `ProtoJsonCompletionProvider` dùng chung — điều kiện hiện tại của 2 chỗ byte-identical nên behavior không đổi. Thêm unit tests khóa semantics.
4. **`proto_json/`** tách theo concern: `mod.rs` (provider + env delegation + `current_message`), `context.rs` (`json_field_candidates`, `locate_message_context`, `KeySuggestion`, `KeyCompletionSpan` — thuần logic), `proto_fields.rs` (`FieldRef`, `find_field`, `field_as_message`, `type_label`, `scalar_label`), `tests.rs`.
5. **`validation.rs`**: `MessageValidationError`, `validate_message`, `refresh_grpc_validation`, `line_byte_range` + sibling `validation_tests.rs` qua `#[cfg(test)] #[path]`.
6. **Dọn `Rc<Entity<...>>` thừa** trong `ProtoJsonCompletionProvider` → `Entity<...>` trực tiếp (giống `EnvCompletionProvider`).
7. Mọi file sau split đều < 650 LOC.

## Step-by-Step Implementation Tasks

1. `git mv env_autocomplete` → `completion/env_autocomplete`; tạo `completion/mod.rs`; sửa `components/mod.rs`.
2. Cập nhật import sites của env_autocomplete (grep chốt danh sách): app/composer_inputs.rs, app/env_input.rs, app/request_bar.rs, components/kv_table.rs, components/body_editor/multipart/state.rs, components/scope_settings_modal/auth_inputs.rs, app/subscriptions.rs.
3. Tạo `completion/proto_json/{mod.rs, context.rs, proto_fields.rs, tests.rs}` — di chuyển VERBATIM; xóa file cũ.
4. Tạo `completion/validation.rs` + `validation_tests.rs`; cập nhật 3 call site `refresh_grpc_validation`.
5. Thêm `unclosed_env_span` + tests; refactor 2 provider dùng helper; xóa `env_trigger_context`.
6. Bỏ wrapper `Rc` trong `ProtoJsonCompletionProvider`.
7. `cargo fmt` + full verification; commit cùng plan doc.

## Risk Mitigation

- **Pure code motion**: logic di chuyển byte-identical; đối chiếu `git diff` — chỉ imports/module docs khác.
- **Test visibility**: tests.rs dùng `use super::*` — mod.rs giữ `use zest_core::grpc::MessageDescriptor` và re-export `json_field_candidates`.
- **Raw strings trong tests** di chuyển nguyên vẹn, không dedent lại.
- **Dedup env-span**: KHÔNG đổi sang `detect_unclosed_env_var_at` (stop-set khác, sẽ đổi behavior delegation).
- Clippy unused imports sau split — chặn bằng `-D warnings`.

## Verification Steps

1. `cargo test` — toàn green: 7 completion + 6 validation tests (di chuyển) + tests mới `unclosed_env_span`; không sửa assertion nào.
2. `cargo clippy --all-targets -- -D warnings` — 0 warning.
3. `cargo fmt --check` — sạch.
4. Smoke thủ công: gRPC Message tab — key suggestions (`{`, nested, partial ident), `{{` env delegation, invalid JSON → gutter mark + hover tooltip.
