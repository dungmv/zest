---
name: tech-debt-auditor
description: >-
  Use this skill to audit, diagnose, track, and resolve technical debt, anti-patterns,
  performance bottlenecks, and memory leaks across zest-core and zest.
---

# Technical Debt Auditor & Resolution Runbook

This skill provides diagnostic checklists and validation procedures to maintain code health, prevent architectural regressions, and manage [`docs/technical-debt.md`](docs/technical-debt.md).

---

## 1. Cycle 2 Modularization & Hygiene Watchlist

All 31 Cycle 1 technical debt items (`SL-001..007`, `PERF-001..013`, `CC-001..004`, `TD-01..019`) have been 100% resolved. When auditing or touching the following areas in Cycle 2, prioritize these modularization and hygiene items:

| ID | Issue | Impact | Target Location | Recommended Remedy |
| :--- | :--- | :---: | :--- | :--- |
| **TD-201** | Crate/module-wide `#![allow(dead_code)]` & `#![allow(unused_imports)]` | 🟡 Med | `crates/app/src/state/mod.rs`, `crates/app/src/components/mod.rs` | Remove module-level `#![allow(...)]` and prune unused methods/re-exports or scope `#[allow]` to specific items. |
| **TD-202** | `scope_settings_modal.rs` exceeds 650 LOC (~1,515 LOC) | 🔵 Low | `crates/app/src/components/scope_settings_modal.rs` | Split into `components/scope_settings_modal/` (`mod.rs`, `headers_tab.rs`, `auth_tab.rs`). |
| **TD-203** | `script_editor.rs` exceeds 650 LOC (~1,443 LOC) | 🔵 Low | `crates/app/src/components/script_editor.rs` | Split JS highlighter/snippets from the `ScriptEditor` GPUI view into `components/script_editor/`. |
| **TD-204** | `response_inspector.rs` exceeds 650 LOC (~1,334 LOC) | 🔵 Low | `crates/app/src/components/response_inspector.rs` | Extract tab sub-renderers (`body_tab.rs`, `headers_cookies_tab.rs`, `timing_popover.rs`) into `components/response_inspector/`. |
| **TD-205** | `env_modal.rs` & `import_modal.rs` exceed 650 LOC (~1,050 / ~934 LOC) | 🔵 Low | `crates/app/src/components/env_modal.rs`, `import_modal.rs` | Modularize into submodules when adding new import formats or environment features. |
| **TD-206** | `#[allow(clippy::too_many_arguments)]` in sidebar & command palette | 🔵 Low | `crates/app/src/components/sidebar/`, `command_palette.rs` | Group entity/callback parameters into dedicated context structs (following `AuthTabContext` pattern). |

---

## 2. Anti-Pattern Detection Matrix

When reviewing or writing code, flag and eliminate these 7 architectural anti-patterns immediately:

### 1. God Objects & File Bloat (SL-001, TD-02, TD-04)
- **Limit**: No struct should exceed **15 fields** without modular grouping.
- **Limit**: Keep new and refactored source files under **~650 LOC**.

### 2. Synchronization or I/O Inside `render()` (PERF-006)
- **Rule**: `Render::render()` must be 100% pure element tree construction. Never call `sync_*`, mutate entities, or read from disk inside `render()`.

### 3. Polling / Diffing Inside `cx.observe` (Event-Emitter Principle)
- **Rule**: `cx.observe` closures in `subscriptions.rs` must ONLY call `cx.notify()`.
- Never store intermediate tracking fields (`last_synced_*`, `*_version`, `is_dirty`) to diff state inside `cx.observe`. Always emit a typed event (`WorkspaceEvent`, `SessionEvent`, `ResponseEvent`, `EnvironmentEvent`) via `cx.emit(...)` and handle it synchronously in `workspace_event_handler.rs` via `cx.subscribe` / `cx.subscribe_in`.

### 4. Uncoordinated Direct Disk Writes (Write-Behind Principle)
- **Rule**: Never spawn ad-hoc per-keystroke disk write tasks or call `WorkspaceStore::write_*` directly from UI input callbacks.
- Always mutate the canonical in-memory state first and enqueue snapshots through `PersistenceState` / `PersistenceCoordinator` (`PersistTarget`).

### 5. Unbounded Memory Growth (SL-002, SL-006)
- **Rule**: Never store response payloads in unbounded `HashMap<K, V>`.
- Use `zest_core::cache::LruMemoryCache` (dual-bounded by entry count and byte size) and wrap responses in `Arc<HttpResponseRecord>`.

### 6. Blocking the UI Thread (TD-003, PERF-010)
- **Rule**: Never run `std::process::Command`, synchronous file scans, or QuickJS script evaluation on the UI thread.
- Use `GitService` (`cx.background_executor()`) for Git operations and `HttpEngine` (`tokio_rt.spawn`) for scripts and network requests.

### 7. Catch-All Deserialization & Unchecked Lock Unwraps (SL-005, TD-005)
- **Rule**: Every YAML storage struct must declare `#[serde(deny_unknown_fields)]` with no `extra: BTreeMap`.
- **Rule**: Never call `.unwrap()` on `Mutex::lock()` or `RwLock`; always recover via `.unwrap_or_else(|poisoned| poisoned.into_inner())`.

---

## 3. Auditing & Verification Commands

Execute this validation pipeline on every audit:

```bash
# Stage 1: Format check
cargo fmt --check

# Stage 2: Compile and run all workspace unit tests
cargo test --workspace

# Stage 3: Strict Clippy linter with zero warnings allowed
cargo clippy --workspace --all-targets -- -D warnings

# Stage 4: Inspect files exceeding the 650 LOC modularity threshold
find crates/ -type f -name "*.rs" -exec wc -l {} + | awk '$1 > 650 && $2 != "total"' | sort -rn
```

---

## 4. Updating the Technical Debt Registry

When resolving or recording any technical debt item:
1. Update [`docs/technical-debt.md`](docs/technical-debt.md) directly (never create secondary files like `technical-debt-02.md`).
2. Accompany every resolution with a regression unit test.
