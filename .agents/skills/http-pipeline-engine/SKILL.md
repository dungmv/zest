---
name: http-pipeline-engine
description: >-
  Use this skill when developing, refactoring, or optimizing HTTP request execution,
  connection pooling, response parsing, hierarchy resolution, or multi-protocol features in core and app.
---

# HTTP Pipeline & Execution Engine Guide

This skill governs the networking pipeline in `zest-core::http` and the application execution services in `crates/app/src/services/`.

---

## 1. Architectural Separation: `HttpEngine` vs `Runner`

The HTTP execution flow is decoupled from the GPUI main thread into two layers:

```
UI Interaction (AppView / RequestComposer / URL Enter)
      │
      ▼
Runner::send_current_request(cx)  [crates/app/src/services/runner.rs]
      │  ├── 1. Resolve hierarchical headers & auth (ws.resolve_request_hierarchy)
      │  ├── 2. Apply global network settings (timeout, redirects, validate_tls)
      │  └── 3. Snapshot cached effective_environment()
      │
      ▼ (Spawns onto Tokio runtime via HttpEngine::execute)
HttpEngine::execute(req_item, env, cancel_rx) [crates/app/src/services/http_engine.rs]
      │
      ├── 1. Resolve dynamic fake variables in-place (resolve_fake_vars_in_request)
      ├── 2. Run Pre-request Script (ScriptSandbox::run_pre_request) & emit ExecEvent::Started
      ├── 3. Merge pre-script env mutations & build ResolvedRequest + apply_pre_script_mutations
      ├── 4. Send over network via ReqwestExecutor with HttpProgress streaming & cancel_rx select
      ├── 5. Run Post-response / Test Script (ScriptSandbox::run_post_response)
      └── 6. Emit ExecEvent::Success { Arc<HttpResponseRecord>, Box<PreRequestOutput>, Box<PostResponseOutput> }
      │
      ▼ (Unbounded mpsc channel consumed by GPUI async task)
Runner updates RequestSessionState, EnvironmentState, ResponseState, and saves .loc cache
```

### Key Rules:
1. **`HttpEngine` is 100% UI-independent**: It depends only on `zest-core` and `tokio`, never on GPUI or `Window`.
2. **Zero-Copy Payload Transfer (SL-006, TD-009)**: Wrap `HttpResponseRecord` in `Arc<HttpResponseRecord>` and box `PreRequestOutput` / `PostResponseOutput` inside `ExecEvent::Success` to keep channel messages small and avoid deep-copying body buffers.
3. **Cached Formatted Body (PERF-007)**: `ResponseState` caches `CachedResponseFormat` (pretty and raw text + line slices) keyed by the `Arc<HttpResponseRecord>` pointer so toggling `Pretty` / `Raw` in the inspector is $O(1)$ without re-parsing JSON/XML.

---

## 2. Connection Pooling & Socket Reuse (PERF-001, PERF-002)

In `crates/core/src/http/executor.rs`, `ReqwestExecutor` manages persistent HTTP connection pools:

- **Client Caching**: `reqwest::Client` instances are cached in an `RwLock<HashMap<(bool, bool), reqwest::Client>>` keyed by `(follow_redirects, validate_tls)`.
- **Pre-warming**: The default `(true, true)` client is pre-warmed during `ReqwestExecutor::new()`.
- **No TCP Probing**: Redundant pre-flight TCP connection probes are prohibited.
- **Connection Reuse Tracking**: Keep-alive socket reuse is tracked via `active_hosts`, setting `connection_reused = true` in `HttpResponseRecord`.
- **Poison-Safe Locks (TD-005)**: Always acquire `cookie_store` and `active_hosts` locks with `.unwrap_or_else(|poisoned| poisoned.into_inner())`.

---

## 3. Hierarchy & Variable Resolution

### 3.1 Shared Header & Auth Inheritance (`crates/core/src/http/hierarchy.rs`)
Both send-time resolution (`resolve_hierarchical_headers_and_auth`) and the read-only composer projection (`resolve_inherited_headers` -> `Vec<InheritedHeader>`) use the same `HeaderMerger`:
- **Precedence Chain**: `Workspace` -> `Parent Folder Chain` (outermost to innermost) -> `Request`.
- **Headers**: Case-insensitive key matching where child overrides ancestor while preserving first-seen order. In the UI (`components/inherited_headers.rs`), ancestor headers overridden by the request are flagged `overridden_by_request: true`.
- **Auth**: If a scope specifies `Auth::Inherit`, resolution walks upward from the nearest parent folder to the workspace, defaulting to `Auth::None`.

### 3.2 Variable Resolution Precedence & Interpolation (`zest_core::interpolation`)
- **Environment Merging (`EnvironmentState::effective_environment`)**: Merges `global_environment` (`environments/globals.yaml` + `.local/secrets/globals.yaml`) with the active environment (active environment overrides globals), cached in an `Arc<Environment>`.
- **Dynamic Fake Variables**: `{{$uuid}}`, `{{$guid}}`, `{{$timestamp}}`, `{{$isoTimestamp}}`, `{{$randomInt}}`, `{{$randomEmail}}`, `{{$randomName}}`, `{{$randomPhoneNumber}}` are resolved per execution via `resolve_fake_vars_in_request` before standard `{{var}}` interpolation.
- **Single Interpolator Reuse (PERF-009)**: Construct `VariableInterpolator::new(env)` once per request resolution or cURL export and call `interpolator.interpolate_cow(text)` across all fields to avoid recompiling regex/maps and to return `Cow::Borrowed` when no `{{` is present.

---

## 4. Multi-Protocol Extensions (Spec 03)

Refer to [`docs/specs/03-protocols-graphql-websocket-grpc.md`](docs/specs/03-protocols-graphql-websocket-grpc.md) when implementing protocol extensions:
- **GraphQL**: Dedicated query + variables split editor and schema introspection via `__schema`.
- **WebSocket**: Asynchronous duplex communication, frame log, and connection lifecycle management.
- **Server-Sent Events (SSE)**: Incremental streaming (`text/event-stream`) into `InspectorDisplays`.
- **gRPC**: HTTP/2 unary and streaming calls with Protobuf reflection.

---

## 5. Verification & Testing

```bash
# 1. Run core HTTP executor and hierarchy unit tests
cargo test --package zest-core --lib http::

# 2. Run variable interpolation unit tests
cargo test --package zest-core --lib interpolation::

# 3. Verify with strict clippy
cargo clippy --workspace --all-targets -- -D warnings
```
