---
name: scripting-automation
description: >-
  Use this skill when developing, refactoring, or testing the JavaScript scripting engine (rquickjs),
  `z.*` API, pre/post-request scripts, assertions, and dynamic fake variables.
---

# Scripting Engine & Automation Guide

This skill governs the JavaScript automation engine implemented in `zest-core::scripting`, providing sandboxed pre-request scripts, post-response test assertions, request chaining, and dynamic fake variables.

---

## 1. Engine Architecture & Runtime

- **Runtime**: Embedded QuickJS engine via `rquickjs` (v0.8), executed off the UI thread inside `HttpEngine::execute` (`tokio_rt.spawn`).
- **Core Modules** in `crates/core/src/scripting/`:
  - `sandbox.rs`: Isolated QuickJS `Runtime` and `Context` per script execution (`ScriptSandbox::run_pre_request`, `ScriptSandbox::run_post_response`), `z.*` object builders, and structured `console` capture.
  - `api.rs`: Context and output payloads (`PreRequestContext`, `PreRequestOutput`, `PostResponseContext`, `PostResponseOutput`).
  - `fake_vars.rs`: `FAKE_VAR_CATALOG` and zero-allocation `Cow<str>` dynamic variable resolver (`resolve_fake_vars`, `resolve_fake_vars_in_request`).

---

## 2. Implemented `z.*` & `console.*` API Surface

Scripts executed in Zest interact with the global `z` object and `console` object:

### 2.1 Environment & Variable Management
- `z.environment.get(key: string): string | undefined` (also aliased as `z.global` and `z.globals`)
- `z.environment.set(key: string, value: any): void` — Records mutation in `env_mutations` for request chaining.
- `z.environment.unset(key: string): void` — Records `(key, None)` removal in `env_mutations`.
- `z.variables.get(key: string): string | undefined`
- `z.variables.set(key: string, value: any): void` — Ephemeral in-script map update.
- `z.variables.has(key: string): boolean`

### 2.2 Pre-Request Mutations (`z.request`)
Available in pre-request scripts and applied to `ResolvedRequest` via `apply_pre_script_mutations`:
- `z.request.headers.add(name: string, value: string): void`
- `z.request.body.json(): any | null` — Parses the current JSON request body.
- `z.request.body.setJson(obj: any): void` — Serializes `obj` via `JSON.stringify` and overrides the outgoing body.

### 2.3 Post-Response Inspection (`z.response`)
Available in post-response / test scripts:
- `z.response.status: number` — HTTP status code (e.g. `200`).
- `z.response.time: number` — Total response duration in milliseconds.
- `z.response.headers: Record<string, string>` — Response headers keyed by lowercase header name.
- `z.response.json(): any` — Parses the response body via `JSON.parse`.
- `z.response.text(): string` — Raw UTF-8 response body (safety-bounded at 10 MB).

### 2.4 Assertions & Test Suite (`z.test` & `z.assert`)
```javascript
z.test("Status code is 200 OK", function () {
    z.assert(z.response.status === 200, "Status must be 200");
});

z.test("Response contains access token", function () {
    const jsonData = z.response.json();
    z.assert(Boolean(jsonData && jsonData.token), "Token must be present");
    // Request Chaining: Store token into active environment for downstream requests
    z.environment.set("access_token", jsonData.token);
});
```

### 2.5 Cryptographic Utilities (`z.crypto`)
- `z.crypto.hmacSha256(secret: string, data: string): string` (hex digest)
- `z.crypto.md5(data: string): string` (hex digest)
- `z.crypto.base64Encode(str: string): string`
- `z.crypto.base64Decode(str: string): string`

### 2.6 Structured Console Capture (`console`)
- `console.log(...args)`, `console.info(...args)`, `console.warn(...args)`, `console.error(...args)`, `console.debug(...args)`
- Captures `ConsoleLogEntry` items with `source` (`PreRequest` / `PostResponse`), `level`, `message`, and millisecond `timestamp` (`%H:%M:%S%.3f`). Uncaught script exceptions are also appended as `ConsoleLogLevel::Error`.

---

## 3. Dynamic Fake Variables (`FAKE_VAR_CATALOG`)

Defined in `crates/core/src/scripting/fake_vars.rs`. `FAKE_VAR_CATALOG` is the **single source of truth** shared between regex resolution (`resolve_fake_vars`) and the UI autocomplete popup (`EnvAutocomplete`):

| Variable | Description | Example Output |
| :--- | :--- | :--- |
| `{{$guid}}` | Generates random UUID v4 | `9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d` |
| `{{$uuid}}` | Generates random UUID v4 | `1b9d6bcd-bbfd-4b2d-9b5d-ab8dfbbd4bed` |
| `{{$timestamp}}` | Current UNIX timestamp in seconds | `1761123456` |
| `{{$isoTimestamp}}` | Current UTC timestamp in ISO 8601 | `2026-10-27T02:45:00.123Z` |
| `{{$randomInt}}` | Random integer between 1 and 1000 | `482` |
| `{{$randomEmail}}` | Random email address | `user4918@example.com` |
| `{{$randomName}}` | Random sample first name | `Olivia` |
| `{{$randomPhoneNumber}}` | Random phone number | `+84912345678` |

---

## 4. Sandboxing & Security Boundaries

1. **No Filesystem / OS Access**: The QuickJS context must never expose `fs`, `child_process`, or raw sockets.
2. **Off-UI-Thread Execution (PERF-010)**: Always execute `ScriptSandbox::run_pre_request` and `run_post_response` inside background tasks (`HttpEngine`), never on the GPUI main thread.
3. **English Comments (TD-007)**: Keep 100% English comments and docstrings across `crates/core/src/scripting/`.

---

## 5. Verification & Testing

```bash
# 1. Run all scripting unit tests (sandbox, crypto, assertions, fake vars)
cargo test --package zest-core --lib scripting::

# 2. Verify with strict clippy
cargo clippy --package zest-core --all-targets -- -D warnings
```
