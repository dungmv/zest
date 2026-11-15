---
name: tech-debt-auditor
description: >-
  Use this skill to audit, diagnose, track, and resolve technical debt, anti-patterns,
  performance bottlenecks, and memory leaks across zest-core and zest.
---

# Technical Debt Auditor & Resolution Runbook

This skill provides diagnostic checklists and validation procedures to maintain code health, prevent architectural regressions, and manage [`docs/technical-debt.md`](docs/technical-debt.md).

---

## 1. Cycle 1 & 2 Debt Archives (100% Resolved)

All 31 Cycle 1 technical debt items (`SL-001..007`, `PERF-001..013`, `CC-001..004`, `TD-01..019`) and all 6 Cycle 2 items (`TD-201..206`) have been 100% resolved (2026-11-13). When auditing, verify the resolved structures stay intact rather than re-deriving them:

| ID | Issue | Resolved As |
| :--- | :--- | :--- |
| **TD-201** | Module-wide `#![allow(dead_code)]` / `#![allow(unused_imports)]` | ✅ Removed; `#[allow]` only at narrowly-scoped, commented items |
| **TD-202** | `scope_settings_modal.rs` ~1,515 LOC | ✅ `components/workspace_setting_modal/` (`mod`, `general_tab`, `headers_tab`, `auth_tab`, `auth_inputs`) — module later renamed from scope_settings (2026-11-15; "scope" is reserved for variable scopes) |
| **TD-203** | `script_editor.rs` ~1,443 LOC | ✅ `components/script_editor/` (`mod`, `highlighter`, `completion`, `editor`, `tests`) |
| **TD-204** | `response_inspector.rs` ~1,334 LOC | ✅ `components/response_inspector/` (`mod`, `body_tab`, `headers_tab`, `cookies_tab`, `console_tab`, `request_sent_tab`, `timing_popover`, `json_highlight`) |
| **TD-205** | 650–875 LOC components (`titlebar`, `import_modal`, `body_editor/multipart`, `git/diff_view`) | ✅ `titlebar/` (mod+menus+breadcrumb), `import_modal/` (mod+preview), `body_editor/multipart/` (mod+state+row), `git/diff_view.rs` + `git/diff_parser.rs` |
| **TD-206** | `#[allow(clippy::too_many_arguments)]` in sidebar | ✅ `FlatTreeInput` + `FlatTreeCtx` methods; direct `SidebarTree` struct literal; 0 allows left. State structs grouped: `GitState` 13 fields (`GitModalUiState`), `ModalState` 12 (`NetworkPrefs`, `EnvModalUi`, `ImportModalUi`), `SidebarState` 14 (`SidebarRenaming`) |

**Regression guards when touching these areas:** keep `#[cfg(test)]` blocks in dedicated `*_tests.rs` files (or `tests.rs` inside module folders) instead of growing the production file past 650 LOC; keep sub-state grouping (`GitModalUiState`, `NetworkPrefs`, `EnvModalUi`, `ImportModalUi`, `SidebarRenaming`) and access nested fields instead of flattening fields back onto the parent struct; never reintroduce module-level `#![allow(...)]`.

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

---

## 5. Refactor & Test Hygiene Playbook (Cycle 2 lessons)

Rules forged during the 2026-11-13 audit remediation and subsequent module splits — apply them to every future refactor:

### 5.1 Rust File-Splitting Rules
- **Inherent `impl` blocks may be split across files** (same crate); **trait `impl` blocks cannot** (E0119 conflicting implementations) — e.g. `fs_store.rs` (inherent) + `fs_store_ops.rs` (`impl WorkspaceStore`).
- A submodule of a **non-`mod.rs`** parent file needs `#[path = "..."]` attribute declaration.
- In `use` paths the first segment only resolves within the current module's scope — cross-tree paths need the `crate::` prefix (`crate::components::...`), the shorthand only works for items of the current module.
- Prefer umbrella modules over minimal in-place splits when extracting a feature family (e.g. `completion/`, `common/`); re-export selectively from `mod.rs`, never glob re-export internals.
- **Move code verbatim, never restyle**: a structural refactor's `git diff` should show only imports/module docs changing. Compare style calls against `git show <commit>^` when in doubt.
- Slim orchestrator pattern: convert one giant `render()` into a thin `mod.rs` + per-tab/per-section free functions that take the owning view (`&AppView` / `&ResponseInspector`) and re-derive state snapshots locally — no borrows across function boundaries, no extra structs.

### 5.2 Test Extraction Rules
- Extract `#[cfg(test)] mod tests` blocks to sibling `*_tests.rs` files to keep production files < 650 LOC.
- **Dedent must be raw-string-aware**: blindly dedenting extracted test blocks corrupts indentation inside `raw_string!`/YAML/diff literals — re-extract from git history if corrupted.
- Tests that `use super::*` require `mod.rs` to retain the imports/re-exports they rely on (e.g. `MessageDescriptor`, `json_field_candidates`).

### 5.3 Test Isolation From Real User Config
- `AppSettings::config_file_path()` honors `ZEST_CONFIG_DIR` and, under `#[cfg(test)]`, routes to a process-unique temp file — **unit tests must never read or write the real user `settings.json`** (a past regression let `cargo test` wipe the user's recent workspaces and theme preference).
- New settings-persistence tests must assert on the isolated path; workspace recents are canonicalized (`path.canonicalize()`) before being stored to avoid duplicate entries.

