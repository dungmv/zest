# Plan: 2026111401 — Hỗ trợ giao thức WebSocket & gRPC

> **Ngày tạo:** 2026-11-14 · **Trạng thái:** Đang triển khai · **Spec gốc:** `docs/specs/03-protocols-graphql-websocket-grpc.md`, Roadmap Phase 2

---

## 1. Goal (Mục tiêu)

Thêm hai giao thức **WebSocket** và **gRPC** song song với HTTP client hiện có:

- **WebSocket:** kết nối persistent (`ws://`/`wss://`), gửi/nhận message Text/JSON/Binary, message log thời gian thực có timestamp & hướng (inbound/outbound), Connect/Disconnect điều khiển độc lập với việc gửi message.
- **gRPC:** gọi RPC bằng schema từ **Server Reflection** hoặc **file `.proto`**, service/method picker, soạn payload JSON → mã hóa Protobuf tự động ở core, hỗ trợ **Unary + Server-streaming**, hiển thị grpc-status/trailers/metadata.

Phạm vi lần này: **cả hai giao thức, chia 2 phase triển khai (WebSocket trước, gRPC sau)**. Client-streaming/Bidi, SSE, GraphQL để phase sau.

## 2. Context (Bối cảnh)

- Hiện tại toàn bộ hệ thống HTTP-only: **không có khái niệm "protocol"** ở bất kỳ đâu — `HttpRequestItem` (`crates/core/src/models/request.rs:314`), `RequestFile` (`crates/core/src/storage/models.rs:82`), `ImportedRequest` (importers), sidebar, composer, engine.
- Executor seam `HttpExecutor::send(ResolvedRequest) -> HttpResponseRecord` là single-shot; WS cần kết nối bền vững, gRPC cần HTTP/2 + Protobuf.
- Postman importer hiện degrade WS/gRPC item thành `GET` (`postman.rs:187`).
- Spec 03 đã định hướng: `tokio-tungstenite` cho WS, `tonic` cho gRPC, JSON→Protobuf ở core, `mpsc::unbounded_channel()` đẩy event về GPUI.

## 3. Architectural Decisions (Quyết định kiến trúc)

| # | Quyết định | Lý do |
|---|---|---|
| AD-1 | **Một struct request duy nhất**: thêm `protocol: Protocol` (`Http` default, `WebSocket`, `Grpc`) + `grpc: Option<GrpcPayload>` vào `HttpRequestItem`/`RequestFile`, cả hai `#[serde(default, skip_serializing_if = ...)]` | Ít xâm lấn nhất: `find_request`, CRUD, persistence, sidebar đều dùng chung `HttpRequestItem`; tránh tách `WebSocketRequest`/`GrpcRequest` riêng |
| AD-2 | `Protocol` là enum riêng, **không** thêm variant vào `HttpMethod` | `HttpMethod` được serde UPPERCASE + `FromStr` strict, dùng rải rác; WS không có method, gRPC method là service path |
| AD-3 | **WsService mới** (`services/ws_service.rs`, Runner-style như `GitService`) giữ `HashMap<request_id, WsSessionHandle>` + message history (bounded) theo request | Kết nối sống độc lập với active request; không phá "9 entities" trong `AppState` (AGENTS 2.1); ResponseState chỉ giữ tham chiếu Arc của session đang active |
| AD-4 | gRPC đi qua luồng single-shot kiểu `execute` (mới: `GrpcExecEvent` loop trong Runner), unary → JSON response hiển thị ở Response tab; server-streaming append message | Tận dụng pattern oneshot-cancel + event loop sẵn có của `Runner::send_current_request` |
| AD-5 | Schema gRPC: `prost-reflect::DescriptorPool`; reflection qua `tonic-reflection` v1alpha; `.proto` compile bằng `protox` (pure-Rust protoc) | Không cần codegen build-time; JSON ↔ DynamicMessage có sẵn qua serde của prost-reflect |
| AD-6 | Message log WS/gRPC **session-only** (không ghi `.loc`); `last_status_code` tái dùng: WS = handshake status, gRPC = grpc-status (0..16) | Tránh đổi shape `LocResponseInfo`/`RequestExampleFile` vốn HTTP-typed |
| AD-7 | Protocol đặt khi tạo request (New Request menu); **không có** đổi protocol in-place ở v1 | Tránh ngữ nghĩa chuyển đổi dữ liệu body/message giữa các protocol |
| AD-8 | Tương thích ngược: file YAML cũ (không có `protocol`) đọc thành `Http`; file mới mở bằng app cũ sẽ lỗi do `deny_unknown_fields` — chấp nhận, ghi chú | Chuẩn serde additive của dự án |

**Dependencies thêm vào `crates/core/Cargo.toml`** (app không cần thêm): `tokio-tungstenite` (rustls), `tonic` + `tonic-reflection`, `prost`, `prost-reflect`, `protox`. Chốt phiên bản mới nhất tương thích khi cài (verify bằng `cargo update` + build).

## 4. Step-by-Step Implementation Tasks

### Phase 1 — Nền tảng Protocol
1. Core `models/request.rs`: enum `Protocol` (+ `is_http`, `as_str`), fields `protocol`, `grpc: Option<GrpcPayload>` trên `HttpRequestItem` + `GrpcPayload { proto_path, use_reflection, service, method, message_json }`.
2. Core `storage/models.rs` + `fs_store_ops.rs`/`traits.rs`: `RequestFile.protocol/grpc`, thread qua mọi conversion (`from_item`, `into_item`, `update_from_item`, `From`, `Default`); `WorkspaceStore::create_request_typed(protocol, ...)`.
3. App: `Protocol` hiển thị — `SidebarReqItem.protocol` + badge WS/gRPC + màu mới `method_websocket`/`method_grpc` trong `theme.rs` (dark+light); menu New Request (sidebar `mod.rs`, `folder_node.rs`) thêm "New WebSocket/gRPC Request"; `RequestTabKind` + danh sách tab theo protocol (WS: Message/Params/Headers/Auth/Pre-request; gRPC: Message/Headers/Proto/Auth/Pre-request); tab `Message` với editor JSON.

### Phase 2 — WebSocket
4. Core `src/ws/` (mới): `WebSocketStatus`, `WsMessage`, `WsEvent`, `WebSocketClient::connect` (tái dùng auth/header expansion từ `ResolvedRequest`, chuẩn hóa scheme ws/wss), split read/write task, auto ping/pong.
5. App `services/ws_service.rs` (mới) + `Runner`: `connect_websocket` (chạy pre-request script, đẩy event loop), `send_ws_message`, `disconnect`; cancel bằng oneshot khi đang connecting.
6. UI: `RequestBar` — nút Connect/Disconnect khi protocol WS (thay Send); `ResponseInspector` — status pill Connected/Connecting/Disconnected/Failed, tab **Messages** (danh sách card message tái dùng pattern `ConsolePanel`, payload JSON/Text/Binary-hex), editor soạn message + Send.

### Phase 3 — gRPC
7. Core `src/grpc/` (mới): `schema.rs` (`resolve_schema`: reflection hoặc protox→FileDescriptorSet→DescriptorPool, cache theo request), `client.rs` (`invoke_unary` / `invoke_server_streaming` qua tonic generic client, JSON↔DynamicMessage, TLS theo scheme `grpc://` vs `grpcs://` + `validate_tls`, timeout).
8. App: `ExecEvent`-style `GrpcExecEvent` + engine flow trong `services/` + `Runner::invoke_grpc` (pre-request script, hierarchy merge → metadata, cancel oneshot).
9. UI: `RequestBar` — dropdown Service/Method từ schema (nút Fetch Schema / load .proto ở tab Proto); `ResponseInspector` — status `OK/ERROR(grpc-status)`, Response tab hiển thị JSON (unary) hoặc message list append (server-streaming), Metadata tab (headers/trailers).

### Phase 4 — Hoàn thiện
10. Postman importer: nhận diện `WS`/`WSS`/scheme ws → `Protocol::WebSocket`, gRPC call → `Protocol::Grpc` (body raw → `message_json`).
11. Pre-request script chạy cho WS connect & gRPC invoke (post-response bỏ qua v1).
12. Tests + `cargo fmt` + `cargo test` + `cargo clippy --all-targets -- -D warnings` = 0 cảnh báo.

## 5. Risk Mitigation

| Rủi ro | Giảm thiểu |
|---|---|
| Nặng compile/binary (tonic, prost, tungstenite) | Chỉ thêm vào `zest-core`; chọn feature tối thiểu (rustls); measure sau khi build |
| `deny_unknown_fields` — file mới lỗi trên app cũ | Chấp nhận (AD-8); giữ field mới `skip_serializing_if` để file HTTP không đổi shape |
| Regression UI HTTP (sync_response_editor, body_tab giả định `HttpResponseRecord`) | Nhánh protocol WS/gRPC tách riêng trong `ResponseInspector`; HTTP path không đổi |
| tonic/tungstenite version alignment (rustls) | Cài cùng lúc, `cargo tree` kiểm tra 1 bản rustls duy nhất |
| LOC > 650 | Module mới `ws/`, `grpc/`, `ws_service.rs` tách file; `response_inspector` thêm submodule `messages_tab.rs` |

## 6. Verification Steps

1. `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings` — sạch.
2. Unit test core: serde round-trip RequestFile (file cũ thiếu `protocol` → Http; file gRPC có payload); chuẩn hóa scheme ws/wss; JSON→DynamicMessage→JSON với sample `.proto` (protox).
3. Thủ công WebSocket: tạo New WebSocket Request → `wss://ws.postman-echo.com/raw` → Connect → gửi Text/JSON → nhận echo inbound trong Messages → Disconnect.
4. Thủ công gRPC: server reflection (vd local tonic example bật reflection) → Fetch Schema → chọn Service/Method → gửi JSON → thấy response OK; thử server-streaming append message; test file `.proto` với server không bật reflection.
5. Regression: tạo/gửi HTTP request thường, import Postman collection, kiểm tra sidebar/composer/response như cũ.
