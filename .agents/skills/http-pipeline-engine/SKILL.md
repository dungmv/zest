---
name: http-pipeline-engine
description: >-
  Use this skill when developing, refactoring, or optimizing HTTP request execution,
  connection pooling, response parsing, hierarchy resolution, or multi-protocol features in core and app.
---

# HTTP Pipeline & Execution Engine Guide

This skill governs the networking pipeline in `zest-core::http`, the protocol-neutral machinery in `zest-core::transport`, and the application execution services in `crates/app/src/services/`.

---

## 1. Architectural Separation: Unified Send Pipeline (`Runner` + engines)

All three transports (HTTP, WebSocket, gRPC) share ONE send pipeline. Protocol dispatch lives in `Runner::send_current_request` (never in the View layer); the shared preamble is split into `gather_send_context` (pure reads) and `begin_send` (flips the sending state) so protocol-specific guards can run between the two; every engine emits the same `SendEvent` enum (`services/send_event.rs`) consumed by one pump (`Runner::spawn_send_pump`). Tokio-side shared stages live in `services/pipeline.rs` (`run_pre_request_stage`, `merge_env_mutations`).

```
UI Interaction (AppView::send_current_request delegates to the Runner)
      │
      ▼
Runner::send_current_request(cx)  [crates/app/src/services/runner.rs]
      │  ├── gather_send_context: resolve hierarchy headers/auth, apply global
      │  │   network settings, snapshot effective_environment
      │  ├── protocol guard (WS: reject if already connected)
      │  └── begin_send: cancel oneshot + start_sending
      │
      ▼ (Dispatch by protocol onto the shared tokio runtime)
HttpEngine::execute / WsService::connect / GrpcEngine::invoke
      │
      ├── 1. pipeline::run_pre_request_stage (fake vars + pre-request script)
      ├── 2. pipeline::merge_env_mutations & ResolvedRequest::from_item (zest-core transport)
      ├── 3. Transport-specific work (ReqwestExecutor / WebSocketClient / GrpcClient)
      └── 4. Emit SendEvent (Started { env_mutations, .. } / Progress / Frame /
             HttpSuccess / HandshakeConnected / CallCompleted / Failed / Cancelled)
      │
      ▼ (Unbounded mpsc channel consumed by GPUI async task)
Runner::spawn_send_pump updates RequestSessionState, EnvironmentState,
ResponseState, and saves the .loc cache (HTTP only)
```

### Key Rules:
1. **Engines are 100% UI-independent**: They depend only on `zest-core` and `tokio`, never on GPUI or `Window`.
2. **Zero-Copy Payload Transfer (SL-006, TD-009)**: Wrap `HttpResponseRecord` in `Arc<HttpResponseRecord>` and box `PostResponseOutput` inside `SendEvent::HttpSuccess` to keep channel messages small and avoid deep-copying body buffers.
3. **One Event Vocabulary**: Never reintroduce per-protocol event enums (`ExecEvent` / `WsUiEvent` / `GrpcUiEvent` are gone); extend `SendEvent` instead.
4. **Protocol-neutral machinery lives in `zest-core::transport`** (ResolvedRequest, hierarchy merge, RequestError, TransportFrame frame log) — WS/gRPC must not import from `zest_core::http` or borrow `WsMessage`-typed logs.
3. **Cached Formatted Body (PERF-007)**: `ResponseState` caches `CachedResponseFormat` (pretty and raw text + line slices) keyed by the `Arc<HttpResponseRecord>` pointer so toggling `Pretty` / `Raw` in the inspector is $O(1)$ without re-parsing JSON/XML.

---

## 2. Tokio Runtime Bridge (`services/runtime.rs`)

GPUI `cx.spawn` threads run on the GPUI background executor **without a Tokio reactor**, but `reqwest` (and any tokio-dependent future) requires one. Rule:
- All async network futures (engines, `zest_core::updater::check_github_release`) are dispatched to the centralized `global_tokio_runtime()`.
- Results return to GPUI via `tokio::sync::oneshot`, with the UI-state mutation happening in the GPUI-side callback — never share a `MutexGuard` or entity borrow across the boundary.
- Never `block_on` on the UI thread and never spawn tokio futures directly from `cx.spawn` closures.

---

## 3. Connection Pooling & Socket Reuse (PERF-001, PERF-002)

In `crates/core/src/http/executor.rs`, `ReqwestExecutor` manages persistent HTTP connection pools:

- **Client Caching**: `reqwest::Client` instances are cached in an `RwLock<HashMap<(bool, bool), reqwest::Client>>` keyed by `(follow_redirects, validate_tls)`.
- **Pre-warming**: The default `(true, true)` client is pre-warmed during `ReqwestExecutor::new()`.
- **No TCP Probing**: Redundant pre-flight TCP connection probes are prohibited.
- **Connection Reuse Tracking**: Keep-alive socket reuse is tracked via `active_hosts`, setting `connection_reused = true` in `HttpResponseRecord`.
- **Poison-Safe Locks (TD-005)**: Always acquire `cookie_store` and `active_hosts` locks with `.unwrap_or_else(|poisoned| poisoned.into_inner())`.

---

## 4. Hierarchy & Variable Resolution

### 3.1 Shared Header & Auth Inheritance (`crates/core/src/transport/hierarchy.rs`)
Both send-time resolution (`resolve_hierarchical_headers_and_auth`) and the read-only composer projection (`resolve_inherited_headers` -> `Vec<InheritedHeader>`) use the same `HeaderMerger`:
- **Precedence Chain**: `Workspace` -> `Parent Folder Chain` (outermost to innermost) -> `Request`.
- **Headers**: Case-insensitive key matching where child overrides ancestor while preserving first-seen order. In the UI (`components/inherited_headers.rs`), ancestor headers overridden by the request are flagged `overridden_by_request: true`.
- **Auth**: If a scope specifies `Auth::Inherit`, resolution walks upward from the nearest parent folder to the workspace, defaulting to `Auth::None`.

### 3.2 Variable Resolution Precedence & Interpolation (`zest_core::interpolation`)
- **Environment Merging (`EnvironmentState::effective_environment`)**: Merges `global_environment` (`environments/globals.yaml` + `.local/secrets/globals.yaml`) with the active environment (active environment overrides globals), cached in an `Arc<Environment>`.
- **Dynamic Fake Variables**: `{{$uuid}}`, `{{$guid}}`, `{{$timestamp}}`, `{{$isoTimestamp}}`, `{{$randomInt}}`, `{{$randomEmail}}`, `{{$randomName}}`, `{{$randomPhoneNumber}}` are resolved per execution via `resolve_fake_vars_in_request` before standard `{{var}}` interpolation.
- **Single Interpolator Reuse (PERF-009)**: Construct `VariableInterpolator::new(env)` once per request resolution or cURL export and call `interpolator.interpolate_cow(text)` across all fields to avoid recompiling regex/maps and to return `Cow::Borrowed` when no `{{` is present.

---

## 5. Multi-Protocol Extensions (Spec 03)

Refer to [`docs/specs/03-protocols-graphql-websocket-grpc.md`](docs/specs/03-protocols-graphql-websocket-grpc.md) when implementing protocol extensions:
- **GraphQL**: Dedicated query + variables split editor and schema introspection via `__schema`.
- **WebSocket**: Asynchronous duplex communication, frame log, and connection lifecycle management.
- **Server-Sent Events (SSE)**: Incremental streaming (`text/event-stream`) into `InspectorDisplays`.
- **gRPC**: HTTP/2 unary and streaming calls with Protobuf reflection.

---

## 6. Verification & Testing

```bash
# 1. Run core HTTP executor and hierarchy unit tests
cargo test --package zest-core --lib http::

# 2. Run variable interpolation unit tests
cargo test --package zest-core --lib interpolation::

# 3. Verify with strict clippy
cargo clippy --workspace --all-targets -- -D warnings
```
