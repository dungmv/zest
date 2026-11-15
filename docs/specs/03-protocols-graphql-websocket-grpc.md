# Đặc tả Kỹ thuật: Mở rộng Đa Giao thức — GraphQL, WebSocket, SSE & gRPC (Protocols Specification)

## 1. Bối cảnh
API hiện đại không còn chỉ là các endpoint REST tĩnh. Các kiến trúc phần mềm ngày nay phổ biến các giao thức:
- **GraphQL:** Cung cấp khả năng truy vấn trường linh hoạt theo yêu cầu client.
- **WebSocket:** Giao tiếp hai chiều thời gian thực (chat, notifications, sàn tài chính, dashboard).
- **Server-Sent Events (SSE):** Truyền dữ liệu dạng dòng (streaming tokens) đặc biệt phổ biến trong kỷ nguyên AI / Large Language Models (OpenAI, Gemini, Anthropic API).
- **gRPC:** Giao tiếp microservice nội bộ với hiệu năng cao dựa trên HTTP/2 và Protobuf.

Tài liệu này xác định cách thiết kế và mở rộng kiến trúc Zest để hỗ trợ đầy đủ các giao thức trên.

> **Tiến độ (2026-11-15):** WebSocket ✅, gRPC ✅, GraphQL ✅ và SSE ✅ — cả 4 giao thức đã chạy trên send pipeline hợp nhất (`Runner` + `SendEvent`, máy móc trung lập giao thức tại `zest_core::transport`). Gap còn lại: WS binary message, gRPC client-streaming/bidi.

---

## 2. GraphQL Client — ✅ Đã hoàn thành (POST JSON + introspection + deep autocomplete)

### 2.1 Cấu trúc Dữ liệu Model
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLPayload {
    pub query: String,
    pub variables: String, // Raw JSON string
    pub operation_name: Option<String>,
}
```

### 2.2 Tính năng Cốt lõi:
1. **Chia 2 Pane Soạn Thảo (Dual-pane editor):**
   - **Query Pane (trên/trái):** Soạn thảo `query` hoặc `mutation`.
   - **Variables Pane (dưới/phải):** Soạn thảo tham số JSON cho query.
2. **Schema Introspection:**
   - Khi người dùng bấm nút **"Fetch Schema"**, gửi truy vấn introspection chuẩn GraphQL:
     ```graphql
     query IntrospectionQuery {
       __schema {
         queryType { name }
         mutationType { name }
         types { ...FullType }
       }
     }
     ```
   - Lưu trữ schema AST vào bộ nhớ đệm (cache) của request.
3. **Gợi ý Trường & Tự Động Hoàn Thành (Autocomplete) — ✅:**
   - `GraphQLQueryCompletionProvider` (`components/completion/graphql_query.rs`) gọi resolver `zest_core::graphql::completions_at(text, caret, schema)`: lexer tự viết (block strings/comments/UTF-8 safe) + walker giữ stack type-context qua selection sets, argument lists, input object literals, inline fragments và `fragment ... on` definitions.
   - Gợi ý theo vị trí: field (Object/Interface), argument name, enum value, input field, type condition (`... on`), fragment spread, directive (`@skip`/`@include`...). `{{env}}` span vẫn do EnvCompletionProvider xử lý.
   - Schema introspect vào session-only `RequestSessionState.graphql_schema` (mirror gRPC); nút **Fetch Schema** nằm trên toolbar Query tab.

---

## 3. WebSocket Client — ✅ Đã hoàn thành (ws/wss + frame log; Binary message chưa hỗ trợ)

### 3.1 Kiến trúc đã triển khai
- Kết nối `ws://` / `wss://` bất đồng bộ qua `WsService` (`services/ws_service.rs`) trên Tokio runtime; sự kiện frame về UI qua kênh `SendEvent` hợp nhất.
- Frame log trung lập giao thức `zest_core::transport::TransportFrame` (inbound/outbound, timestamp, kind) — dùng chung với gRPC; frame log được persist theo request và **giữ nguyên khi switch request**.
- Message tab trong composer: gửi Text/JSON, lịch sử tin nhắn real-time, filter, clear log. Status: Disconnected/Connecting/Connected (handshake headers hiển thị ở response state).

### 3.2 Gap còn lại
- Binary message chưa hỗ trợ (chỉ Text/JSON); auto-reconnect/heartbeat chưa có.

---

## 4. Server-Sent Events (SSE) & Streaming Responses

### 4.1 Ứng dụng Thực Tế
Khi kiểm thử các endpoint AI streaming (như `POST https://api.openai.com/v1/chat/completions` với `{"stream": true}`), response trả về theo từng dòng `data: {"choices": [{"delta": {"content": "Hello"}}]}\n\n`.

### 4.2 Kiến trúc đã triển khai
- Parser SSE tự viết trong `zest_core::sse::SseParser` (đúng thuật toán WHATWG: `data`/`event`/`id`/`retry`, comment keep-alive, CRLF/CR/LF, BOM, multi-line data) — không thêm crate.
- `ReqwestExecutor::send_streaming` (core) drive `bytes_stream()`, mỗi event bắn `SseOutput` → `HttpEngine` chuyển thành `SendEvent::Frame` (`TransportFrame` Text, event name trong `label`, comment → `FrameKind::Ping`).
- Frame log engine-side bound 1000/request (restore khi switch request, mirror gRPC); body tích lũy cap 2 MB vào record → Response tab + `.loc` cache hoạt động như HTTP.
- `timeout_ms` đóng vai trò **idle timeout giữa các chunk** (stream dài không bị chết ở 30s); response không phải `text/event-stream` fallback về HTTP thường. Request bar giữ Send/Cancel như HTTP (không Connect/Disconnect); response tabs: `[Messages, Response, Headers, Cookies, Console]`.

---

## 5. gRPC Client — ✅ Đã hoàn thành (unary & server-streaming; client-streaming/bidi chưa hỗ trợ)

### 5.1 Kiến trúc đã triển khai
- Engine `GrpcEngine` + `GrpcClient` (HTTP/2, `prost-reflect`) chạy trên Tokio runtime, tham gia chung send pipeline (`Runner::send_current_request` → `SendEvent::CallCompleted`).
- Schema nguồn: nạp file `.proto` hoặc **Server Reflection** (`grpc.reflection`) — tự động fetch khi gửi; filter service nội bộ theo `grpc.reflection` ONLY (không dùng bare `grpc.` — tránh ẩn health service).
- Message tab soạn JSON thân thiện với autocomplete key từ `GrpcSchemaSummary.pool`; `{{var}}` interpolation + fake vars chạy trên `message_json` như HTTP body; validation JSON/protobuf hiển thị inline editor diagnostics + red gutter mark (không toolbar check).
- Proto tab chỉ chọn schema source; frame log dùng chung `TransportFrame` và persist qua engine-side session log.

### 5.2 Gap còn lại
- Client-streaming & bidi streaming chưa hỗ trợ.
