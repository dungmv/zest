# AGENTS.md — Zest Workspace Instructions

This document governs all AI agent interactions and development sessions in the `zest` repository. Every agent must strictly adhere to the architecture, standards, patterns, and constraints documented herein to ensure consistency and prevent regressions.

---

## 1. Project Overview & Architecture

`zest` is a high-performance, macOS-native, local-first REST & API client built with **GPUI** (GPU-accelerated UI framework from Zed) and **Rust**.

### 1.1 Workspace Layout

The project is structured as a Cargo workspace with two core crates:

```
zest/
├── crates/
│   ├── core/                  # [zest-core] Pure domain, storage, and networking engine
│   │   ├── src/
│   │   │   ├── models.rs      # Canonical domain & storage data models
│   │   │   ├── cache/         # Bounded LRU memory cache infrastructure
│   │   │   ├── http/          # Reqwest-based HTTP executor, connection pool, hierarchy merge
│   │   │   ├── storage/       # Local-first YAML storage, watcher, slugification
│   │   │   ├── scripting/     # QuickJS (rquickjs) sandbox, z.* API, fake vars catalog
│   │   │   ├── git/           # Git VCS wrapper (with subprocess timeouts & safe discard)
│   │   │   ├── curl/          # Cross-shell cURL command parser & exporter
│   │   │   ├── importers/     # OpenAPI 3.0/3.1 & Postman v2.0/v2.1 collection importers
│   │   │   └── interpolation.rs # Variable resolution and substitution engine
│   └── app/                   # [zest] GPUI native application
│       ├── src/
│       │   ├── app/           # AppView, ComposerInputs, InspectorDisplays, subscriptions, workspace_event_handler
│       │   ├── components/    # Modular GPUI UI components (< 650 LOC per module)
│       │   ├── state/         # Multi-entity state (9 entities + PersistenceCoordinator + workspace_events)
│       │   ├── services/      # HttpEngine, background Runner, GitService command router
│       │   ├── platform/      # Platform-specific window & titlebar integration
│       │   ├── settings.rs    # User preferences persistence
│       │   ├── theme.rs       # Theme definitions and dark/light/system switching
│       │   └── main.rs        # Application entrypoint & GPUI bootstrap
├── docs/                      # Technical specifications, roadmap, plans, and debt registry
└── .agents/                   # Agent custom skills, rules, and runbooks
```

---

## 2. Fundamental Architectural Principles

These principles were forged through major refactoring milestones (SL-001..007, PERF-001..013, CC-001..004, TD-001..019). Violations of these principles are considered regressions.

### 2.1 Multi-Entity State Architecture (No God Objects)
- **Zero God Objects**: Monolithic structs like the legacy `WorkspaceState` (>3,300 LOC) or monolithic `AppView` (32 fields) are strictly forbidden.
- **`AppState` Container**: `AppState` (`crates/app/src/state/app_state.rs`) is solely an initialization container that owns 9 decoupled GPUI entities:
  1. `workspace: Entity<WorkspaceDataState>`: Pure domain data (collections, requests, folders, pins, workspace metadata).
  2. `env: Entity<EnvironmentState>`: Environments, `globals`, active environment, cached variable merging.
  3. `persistence: Entity<PersistenceState>`: Coalesced write-behind persistence (`PersistenceCoordinator`), file watcher, conflict & dirty tracking.
  4. `sidebar: Entity<SidebarState>`: Sidebar width, collapse states, history navigation, generic `RenamingState<T>`.
  5. `modal: Entity<ModalState>`: Modal visibility flags, network settings, theme preference, split layout.
  6. `git: Entity<GitState>`: VCS status cache, staging, diff, branches, remotes (commands dispatched via `GitService`).
  7. `session: Entity<RequestSessionState>`: Active request pointer, active composer tabs, in-flight send status.
  8. `response: Entity<ResponseState>`: Active response, cached formatted bodies, test results, console logs, LRU cache.
  9. `toast: Entity<ToastState>`: Notification toasts.
- **No Prop-Drilling**: Do not pass `AppState` into child components. Child views must receive only the specific `Entity<*State>` handles they observe or mutate.

### 2.2 Single-Active Request & On-Demand Borrowing (SL-003)
- `RequestSessionState` only stores `active_request_id: Option<String>`.
- **Never duplicate request data**: Never store a full `HttpRequestItem` clone in `RequestSessionState`.
- All composer tabs, header tables, and pipelines query and borrow `&HttpRequestItem` directly from `WorkspaceDataState`. Scratch requests are managed via `WorkspaceDataState::scratch_request`.

### 2.3 Pure GPUI Rendering & Dual-Channel Event Architecture (PERF-006)
- **`AppView::render()` must be 100% pure element tree construction**:
  - NEVER call data synchronization routines (`sync_*`), disk reads, or state mutations inside `render()`.
  - Rendering must run at 60/120 FPS with zero overhead.
- **Dual-Channel Event & Redraw Pattern**:
  1. **Typed Command / Event Channel (`cx.emit` + `cx.subscribe` / `cx.subscribe_in`)**: Cross-entity synchronization and UI input updates (`ComposerInputs`, `InspectorDisplays`) flow through typed events (`WorkspaceEvent`, `SessionEvent`, `ResponseEvent`, `EnvironmentEvent` in `state/workspace_events.rs`) handled centrally in `crates/app/src/app/workspace_event_handler.rs`.
  2. **Pure Redraw Channel (`cx.notify` + `cx.observe`)**: `cx.observe` closures in `crates/app/src/app/subscriptions.rs` must ONLY trigger `cx.notify()`. Never store or diff intermediate flags (`last_synced_*`, `*_version`, `is_dirty`) inside `cx.observe`.

### 2.4 Non-Blocking Asynchronous Concurrency
- **UI Thread Safety**: Never execute blocking I/O, `std::process::Command`, filesystem directory scans, or JavaScript evaluation on the main UI thread.
- HTTP execution and QuickJS pre/post scripts run on the Tokio runtime via `HttpEngine::execute` (`services/http_engine.rs`).
- Git CLI operations run via `GitService` (`services/git_service.rs`) on `cx.background_executor()`, setting `loading` or `is_syncing` guards on `GitState`.

### 2.5 Bounded Memory Caching (SL-002, SL-006, PERF-007)
- Session response caches must use `zest_core::cache::LruMemoryCache` with dual bounds (max 100 entries, max 50 MB byte capacity).
- Responses are wrapped in `Arc<HttpResponseRecord>` for zero-copy $O(1)$ sharing across threads and UI states, with formatted `Pretty`/`Raw` text cached per `Arc` pointer (`CachedResponseFormat`).

### 2.6 Local-First Storage & Unified Write-Behind Persistence (SL-005)
- All storage models in `zest_core::storage::models` must specify `#[serde(deny_unknown_fields)]`. Never add catch-all fields like `extra: BTreeMap`.
- All UI mutations to YAML-backed state (`Request`, `Environment`, `Globals`, `WorkspaceMeta`, `FolderMeta`, `Pins`, `LocalState`) update in-memory state immediately and enqueue coalesced, revision-ordered snapshots via `PersistenceCoordinator` (`state/persistence_coordinator.rs`).
- File writes initiated by the application register paths with `FsWorkspaceStore::suppress_path` (managed by the `workspace-suppress-cleaner` thread) to prevent watcher echo loops.

---

## 3. Code Quality & Contribution Standards

### 3.1 Clippy & Static Analysis
- Every change must pass `cargo clippy --all-targets -- -D warnings` with **0 warnings**.
- Never add `#![allow(dead_code)]` or `#![allow(clippy::...)]` at the crate or module level. Address the root cause or scope `#[allow(...)]` to the exact minimal function/item with an explanatory comment.

### 3.2 Error Handling & Panic Prevention (TD-07 / TD-005)
- Never use `.unwrap()` on Mutex or RwLock guards:
  ```rust
  // CORRECT:
  let store = self.cookie_store.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
  ```
- Use `Result` and `?` propagation in `zest-core`. In UI callbacks, present user-friendly errors via `ToastState` or error badges.

### 3.3 Documentation & Language
- **Source Code**: 100% English for all code comments, docstrings, struct field descriptions, error messages, and git commit messages.
- **Documentation**: Vietnamese or bilingual is preserved in `docs/` (`docs/specs/`, `docs/technical-debt.md`, `docs/roadmap.md`).

### 3.4 Modularity & File Size Limit
- Keep source files under **~650 LOC**.
- When a component or state grows beyond ~650 LOC, modularize it into a dedicated folder (e.g., `components/git/`, `components/sidebar/`, `components/body_editor/`, `components/settings_modal/`, `app/`, `state/`).

---

## 4. Planning & Plan Artifacts Protocol

To ensure all architectural and implementation decisions are preserved across agent sessions and tracked in version control:
- **Mandatory Plan Storage**: Whenever creating or updating an implementation plan (via `/plan`, brain artifacts, or design proposals), the agent **MUST** write/sync a persistent copy directly into the repository at:
  ```
  docs/plans/YYYYMMDDSeq-<plan-name>.md
  ```
  *(Example: `docs/plans/2026102701-collection-runner-phase6.md`)*
- **Format Requirements**:
  - Date prefix `YYYYMMDD` using current local date (e.g. `20261027`).
  - Sequence number `Seq`: 2-digit zero-padded incremental sequence per day (`01`, `02`, ..., `17`). Always inspect existing files in `docs/plans/` for that day to determine the next sequential number.
  - Kebab-case descriptive artifact name `<plan-name>`.
  - Standard markdown structure: Goal, Context, Architectural Decisions, Step-by-Step Implementation Tasks, Risk Mitigation, and Verification Steps.
- **Git Tracking**: All plans stored in `docs/plans/` are part of the repository history and must be committed alongside the corresponding implementation.

---

## 5. Skills Directory & Runbooks

Specialized runbooks and checklists are located in `.agents/skills/`:

| Skill | Focus Area |
| :--- | :--- |
| [`gpui-architecture`](.agents/skills/gpui-architecture/SKILL.md) | Multi-entity state, typed `EventEmitter` + `cx.subscribe`, pure redraw `cx.observe`, component modularity |
| [`http-pipeline-engine`](.agents/skills/http-pipeline-engine/SKILL.md) | `HttpEngine` vs `Runner`, connection pooling, hierarchy merge (`HeaderMerger`), variable interpolation |
| [`scripting-automation`](.agents/skills/scripting-automation/SKILL.md) | QuickJS (`rquickjs`) engine, `z.*` & `console.*` API, pre/post scripts, assertions, `FAKE_VAR_CATALOG` |
| [`local-first-storage-vcs`](.agents/skills/local-first-storage-vcs/SKILL.md) | `.local/` & `collections/` YAML layout, `PersistenceCoordinator`, watcher reconciliation, `GitService` |
| [`interop-spec-converters`](.agents/skills/interop-spec-converters/SKILL.md) | OpenAPI 3.0/3.1, Postman v2.0/v2.1, cross-shell cURL parser/exporter, cycle-safe `$ref` resolution |
| [`tech-debt-auditor`](.agents/skills/tech-debt-auditor/SKILL.md) | Debt registry auditing, 7-point anti-pattern matrix, Cycle 2 modularization watchlist |

---

## 6. Verification Checklist

Before finishing any task, execute the full validation pipeline:

```bash
# 1. Format check (optional but recommended)
cargo fmt --check

# 2. Compile and run all tests (core + app)
cargo test

# 3. Strict clippy analysis
cargo clippy --all-targets -- -D warnings
```
