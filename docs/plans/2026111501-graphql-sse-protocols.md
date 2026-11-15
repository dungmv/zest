# Plan 2026111501 — GraphQL & SSE Protocols

## Goal

Hoàn tất bộ 4 giao thức của Zest: thêm **GraphQL** (POST JSON + introspection schema + deep autocomplete theo type-context) và **SSE** (Server-Sent Events streaming với frame log hợp nhất), theo đúng kiến trúc send pipeline hợp nhất đã dùng cho WebSocket/gRPC.

## Context

- WS + gRPC đã ship (plan Cycle 2, spec `docs/specs/03-protocols-graphql-websocket-grpc.md` §3, §5). Còn thiếu: GraphQL (§2), SSE (§4).
- Máy móc trung lập giao thức sẵn có: `zest_core::transport` (TransportFrame, ResolvedRequest, hierarchy), `SendEvent` hợp nhất, dispatch tại `Runner::send_current_request` (runner.rs:93).
- User chọn **Full deep autocomplete** cho GraphQL: introspection + gợi ý field lồng nhau theo context (cần mini-parser tracking current type).
- Monochrome theme: KHÔNG tint icon mới bằng protocol_color; phân biệt bằng icon shape (`ChartNetwork` cho GraphQL, `Zap` cho SSE).

## Architectural Decisions

1. **GraphQL = HTTP POST dưới nền**: `Protocol::GraphQL` dispatch qua `HttpEngine` với một item HTTP tổng hợp (`to_graphql_http_item`: method=Post, body=Json `{query, variables?, operationName?}`) → tái dùng toàn bộ cancel/progress/HttpSuccess/.loc cache/scripts/tests.
2. **`GraphQLPayload { query, variables, operation_name }`** trên `RequestItem.graphql: Option<...>` + `RequestFile.graphql` (deny_unknown_fields) — mirror `GrpcPayload`.
3. **SSE = nhánh streaming trong HttpEngine**: `ReqwestExecutor` tách `build_request()` riêng; nhánh SSE parse `bytes_stream()` bằng `SseParser` (core, W3C spec, hand-rolled — không thêm crate), mỗi event bắn `SendEvent::Frame` (TransportFrame Text/Inbound, `label` = event name), kết thúc/cancel → HttpSuccess/Cancelled. Content-type không phải `text/event-stream` → fallback HTTP thường.
4. **SSE timeout**: `timeout_ms` là **idle timeout** giữa các chunk (tokio::time::timeout per chunk), không phải tổng request — stream dài không bị giết ở 30s. Body tích lũy cap 2 MB; frame log cap 1000.
5. **Deep autocomplete**: core module `crates/core/src/graphql/` gồm lexer (strings/comments/block-strings safe, char-boundary) + context resolver (stack type-context qua selection sets, args, input objects, fragments) trả về suggestions từ `GraphQLSchemaSummary` (parse introspection payload). Provider gắn vào query editor qua `lsp_mut().completion_provider` (pattern gRPC).
6. **Không thêm SendEvent variant mới**: SSE dùng Frame + Progress + HttpSuccess; GraphQL dùng HttpSuccess.
7. UI tabs: `RequestTabKind::Query` (dual-pane Query+Variables), GraphQL response = HTTP tabs; SSE composer = HTTP tabs, response = `[Messages, Response, Headers, Cookies, Console]`. `is_realtime` (request_bar + inspector) thu hẹp thành `matches!(WebSocket|Grpc)`.
8. `protocol_color(GraphQL|Sse)` → token neutral (không tạo màu mới — monochrome).

## Step-by-Step Implementation Tasks

### Phase A — Core
1. `models/request.rs`: `Protocol::{GraphQL, Sse}` + `as_str` ("GraphQL"/"SSE"); `GraphQLPayload`; `RequestItem.graphql` + `graphql_mut()` + `to_graphql_http_item() -> Result<RequestItem, String>` (validate variables JSON).
2. `storage/models.rs`: `RequestFile.graphql` + 4 conversion fns.
3. `transport/frame.rs`: `label: Option<String>` (SSE event name).
4. `sse/` mới: `SseEvent`, `SseParser` (data/event/id/retry, comment→Ping, multi-line data, CRLF/BOM) + unit tests.
5. `graphql/` mới: `GraphQLSchemaSummary` + parse introspection; `INTROSPECTION_QUERY`; lexer + context resolver (`completions_at(text, caret) -> Vec<Completion>`); tests.
6. `http/executor.rs`: extract `build_request()`; thêm nhánh SSE stream (event callback) chia sẻ builder.

### Phase B — Engines
7. `http_engine.rs`: nhánh SSE (Frame events + Progress counts + sse frame log Mutex map cap 1000 + snapshot); `fetch_graphql_schema` (tokio task + oneshot, mirror grpc).
8. `runner.rs`: dispatch arm GraphQL (validate trước begin_send → toast lỗi); pump Frame routing SSE (register + push_frame nếu active); `Failed` arm.

### Phase C — UI
9. Tabs: `RequestTabKind::Query` + `tabs_for` (GraphQL → `[Query, Params, Headers, Auth, PreRequestScript, Tests]`; Sse → HTTP list); `ResponseTabKind::tabs_for` (Sse → `[Messages, Response, Headers, Cookies, Console]`); `workspace_crud::protocol_method` (GraphQL→Post, Sse→Get); `theme.rs protocol_color` neutral arms.
10. Composer: `components/request_composer/graphql_query_tab.rs` (dual-pane query/variables, toolbar: operation name input + Fetch Schema); `composer_inputs.rs` `GraphQLComposerFields` + sync; `message_tab.rs` arm; `mod.rs` match arm.
11. `subscriptions.rs`: Change subs query/variables/operation_name (mirror grpc pattern).
12. Completion provider (components/completion/): deep autocomplete dùng `session.graphql_schema`.
13. Inspector: `is_realtime` → WebSocket|Grpc; SSE pill (spinner+progress khi stream, HTTP pills khi xong); messages_tab label event name + SSE empty state.
14. `request_bar.rs`: `is_realtime` thu hẹp; GraphQL render static POST badge.
15. Sidebar (`mod.rs` + `folder_node.rs` đồng bộ): "New GraphQL" (ChartNetwork), "New SSE" (Zap) — không tint.
16. `workspace_event_handler.rs`: SSE snapshot restore arm; `app/mod.rs` wrapper `fetch_graphql_schema`.

### Phase D — Docs & Verification
17. Spec §2/§4 progress + roadmap.md + docs/README.md + docs/plans/README.md.
18. `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`.

## Risk Mitigation

- **deny_unknown_fields**: `RequestFile` phải thêm `graphql` key; YAML cũ không có key → default None.
- **Infinite streams**: không cache .loc khi cancel; cap body 2 MB / frames 1000.
- **Parser edge cases**: block strings `"""`, comments `#`, char-boundary (không byte-slice user text — TD Windows FFI).
- **Không block UI thread**: mọi parse/decode chạy trên tokio runtime.
- **650 LOC/module**: graphql/ tách lexer | schema | complete; complete.rs có thể tách context.rs.
- Import Postman graphql mode: out of scope (defer).

## Verification Steps

- Unit tests: SSE parser (CRLF/BOM/multi-line/named events/retry), graphql context resolver (root/nested/args/input-object/fragments), introspection parse, YAML roundtrip payload, to_http_item validation.
- E2E manual: GraphQL qua `https://countries.trevorblades.com/graphql` (introspection + query + variables); SSE qua `https://sse.dev/test` (event stream live, cancel giữa stream).
- Pipeline: `cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings`.
