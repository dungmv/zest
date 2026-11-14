# Plan: 2026111502 — Protocol Source Cleanup (HTTP / WebSocket / gRPC)

> Thay thế placeholder rỗng `2026111401-websocket-grpc-protocols.md` (đã xóa).

## Goal

Làm sạch sự lẫn lộn giữa HTTP, WebSocket và gRPC trải trên 4 tầng (core → services → state → UI), trả về ranh giới module trung thực, khử mã trùng lặp trong send pipeline, và sửa 2 hành vi thiếu nhất quán (gRPC message không interpolate, frame log gRPC mất khi switch request).

## Context

- **Core**: `http/` chứa máy móc dùng chung cho cả 3 protocol — `ResolvedRequest::from_item` (`crates/core/src/http/resolved.rs`), `hierarchy.rs`, URL utils; `ws/client.rs` import trực tiếp `crate::http`; frame log "unified" được type bằng `zest_core::ws::WsMessage`, gRPC tự fabricated `WsMessage` (`grpc_engine.rs`).
- **Services**: dispatch protocol nằm ở View (`app/mod.rs`); `Runner` có 3 pipeline copy-paste (`send_current_request` / `connect_websocket` / `invoke_grpc`) lặp preamble ×3 và 3 event enum mirror nhau (`ExecEvent` / `WsUiEvent` / `GrpcUiEvent`); env-mutation merge lặp ×4; pre-script block lặp ×3.
- **State/UI**: `request_composer.rs` 672 LOC (>650) chứa 160 dòng WS Message tab inline; 3 toolbar builder + 2 Format JSON + 3 status pill trùng mã; `ResponseState` trộn field HTTP/WS/gRPC; `start_sending_internal` chỉ reset field HTTP.
- **Hành vi**: gRPC `message_json` KHÔNG chạy interpolation `{{var}}`; frame log gRPC mất khi chuyển request (WS giữ được qua snapshot).

Quyết định user (2026-11-15): refactor **toàn bộ 4 phase** + **cả 2 bản sửa hành vi**.

## Architectural Decisions

1. **Umbrella `zest_core::transport`** (theo nguyên tắc module-umbrella): phần trung thực về giao thức — resolver, hierarchy merge, error, frame log — tách khỏi `http/`; `http/` chỉ còn executor HTTP thuần.
2. **Frame log neutral hóa**: `WsMessage`/`WsDirection`/`WsMessageKind` → `transport::{TransportFrame, FrameDirection, FrameKind}`. In-memory only, không đụng YAML.
3. **Một send pipeline**: dispatch protocol chuyển từ View vào `Runner::send_current_request`; preamble gom vào `gather_send_context` + `begin_send` (tách 2 bước để guard `is_connected` của WS chạy trước khi bật sending); 3 event enum hợp nhất thành `SendEvent`; 1 pump chung.
4. **Tokio-side helpers dùng chung**: `services/pipeline.rs` với `run_pre_request_stage` + `merge_env_mutations`.
5. **Response state hygiene**: rename `ws_messages` → `frames`, thêm `reset_realtime()` gọi từ `start_sending_internal`.
6. **Hành vi mới**: gRPC frame log persist qua engine-side session log (mirror `WsService::WsSession`); gRPC `message_json` chạy interpolation env + fake vars như HTTP body.

## Step-by-Step Implementation Tasks

### Phase 1 — Core: umbrella `zest_core::transport`
- Tạo `crates/core/src/transport/{mod,resolved,hierarchy,error,frame}.rs`.
- Di chuyển nguyên văn `http/resolved.rs` (317 LOC) và `http/hierarchy.rs` (317 LOC); tách `RequestError` khỏi `http/executor.rs:17-32`.
- `frame.rs` mới: rename + relocate `WsMessage`/`WsDirection`/`WsMessageKind` từ `ws/types.rs:12-56`.
- Sửa phụ thuộc: `http/` còn executor; `ws/client.rs` import transport; `models/request.rs::apply_url` dùng `crate::transport::parse_query_from_url`; consumers app đổi import; `grpc_engine.rs` ngừng import `zest_core::ws`.
- Di chuyển test resolver sang transport; storage round-trip tests pass nguyên trạng.

### Phase 2 — Services: một send pipeline duy nhất
- `services/send_event.rs`: enum `SendEvent` hợp nhất (Started có `env_mutations` cho cả HTTP; `HttpSuccess` chỉ còn post mutations).
- `services/pipeline.rs`: `run_pre_request_stage`, `merge_env_mutations` — 1 bản thay các bản lặp.
- `runner.rs`: `gather_send_context` → `begin_send` → router `match protocol` → 1 pump chung `pump_send_events`. Xóa method công khai `connect_websocket`/`invoke_grpc`.
- `app/mod.rs::send_current_request` delegate vào `runner.send_current_request`; call site UI không đổi.

### Phase 3 — State hygiene + sửa hành vi
- `response_state.rs`: `ws_messages` → `frames`, `push_ws_message` → `push_frame`, `restore_ws_session` → `restore_realtime_session`; `start_sending_internal` gọi `reset_realtime()` (clear frames + status + handshake headers + grpc_last_call).
- `grpc_engine.rs`: bounded per-request frame log (cap 1000) + `register_frame`/`snapshot`; Runner pump luôn register; `workspace_event_handler.rs` restore frames gRPC qua `grpc.snapshot` song song nhánh WS.
- gRPC interpolation: `zest_core::interpolation::interpolate(&message_json, env)` trong pre-stage + mở rộng `resolve_fake_vars_in_request` cover `grpc.message_json`.

### Phase 4 — UI: tách tab + dedup helper
- `app/ws_message_tab.rs` mới (~180 LOC) tách block inline `request_composer.rs:282-441` theo pattern context-struct của `grpc_message_tab.rs`.
- `messages_tab.rs` render `Vec<TransportFrame>`.
- `components/common/` umbrella: `toolbar.rs`, `format_json.rs`, `status_pill.rs` — khử 3+2+3 bản trùng.
- `state/workspace_crud.rs`: free fn `edit_active_request(ws, session, id, cx, edit)` thay boilerplate ~15 chỗ.
- `composer_inputs.rs`: gom field gRPC thành `GrpcComposerFields`.
- Cập nhật `.agents/skills/http-pipeline-engine/SKILL.md` (dispatch về Runner, event enum hợp nhất).

## Non-goals

SSE/GraphQL; importers ghi `grpc: None`; lưu response history WS/gRPC ra `.loc`; optional hóa `method` cho WS/gRPC (giữ tương thích YAML); tách `ComposerInputs` hoàn toàn theo protocol.

## Risk Mitigation

- Mỗi phase commit riêng, bắt buộc `cargo fmt --check` + `cargo test` + `cargo clippy --all-targets -- -D warnings` = 0 warning trước khi commit.
- Di chuyển file core là rename nguyên văn — giảm thiểu diff review được.
- Storage round-trip (`storage/tests.rs::test_protocol_defaults_and_roundtrip`) là rào chắn tương thích YAML — phải pass không sửa assertion.
- Hành vi HTTP mới: pre-script env mutations áp tại `Started` thay vì `Success` (đồng bộ WS/gRPC) — chấp nhận, ghi rõ trong commit.

## Verification Steps

- Mỗi phase: fmt + test + clippy (toàn workspace).
- Smoke test thủ công: HTTP send; WS connect ↔ frames ↔ switch request (log còn); gRPC unary + server-streaming + auto-fetch schema + `{{var}}` trong message resolve đúng + switch request (log còn — hành vi mới); Format JSON ở Body tab và gRPC Message tab; YAML save/reload không đổi cấu trúc.

## Commits

1. `refactor(core): extract protocol-neutral transport module` (+ plan doc)
2. `refactor(app): unify send pipeline and event channel under Runner`
3. `fix(app): persist grpc frame log across request switches and interpolate grpc messages`
4. `refactor(app): extract ws message tab and deduplicate composer ui helpers`
