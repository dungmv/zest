# AGENTS.md — Zest Workspace Instructions

This document governs all AI agent interactions and development sessions in the `zest` repository. Every agent must strictly adhere to the architecture, standards, patterns, and constraints documented herein to ensure consistency and prevent regressions.

---

## 1. Project Overview & Architecture

`zest` is a high-performance, macOS-native, local-first REST & API client built with **GPUI** (GPU-accelerated UI framework from Zed) and **Rust** (2024 edition).

### 1.1 Workspace Layout

The project is structured as a Cargo workspace with two core crates:

```
zest/
├── crates/
│   ├── core/                  # [zest-core] Pure domain, storage, and networking engine
│   │   ├── src/
│   │   │   ├── models.rs      # Canonical domain & storage data models
│   │   │   ├── cache/         # Bounded LRU memory cache infrastructure
│   │   │   ├── http/          # Reqwest-based HTTP executor, connection pool, cookies
│   │   │   ├── storage/       # Local-first YAML storage, watcher, slugification
│   │   │   ├── scripting/     # QuickJS (rquickjs) sandbox, z.* API, fake vars
│   │   │   ├── git/           # Git VCS wrapper for local workspace versioning
│   │   │   ├── curl/          # cURL command parser & exporter
│   │   │   ├── importers/     # OpenAPI 3.0/3.1 & Postman collection importers
│   │   │   └── interpolation.rs # Variable resolution and substitution engine
│   └── app/                   # [zest] GPUI macOS native application
│       ├── src/
│       │   ├── app/           # AppView, composer inputs, inspector displays, subscriptions
│       │   ├── components/    # Modular GPUI UI components (< 650 LOC per module)
│       │   ├── state/         # Multi-entity state management (9 isolated entities)
│       │   ├── services/      # HttpEngine, background Runner, Git command service
│       │   ├── settings.rs    # User preferences persistence
│       │   ├── theme.rs       # Theme definitions and dark/light switching
│       │   └── main.rs        # Application entrypoint & GPUI bootstrap
├── docs/                      # Technical specifications, roadmap, and debt registries
└── .agents/                   # Agent custom skills and runbooks
```

---

## 2. Fundamental Architectural Principles

These principles were forged through major refactoring milestones (SL-001 through SL-006, TD-001 through TD-004). Violations of these principles are considered regressions.

### 2.1 Multi-Entity State Architecture (No God Objects)
- **Zero God Objects**: Monolithic structs like the legacy `WorkspaceState` (>3,300 LOC) or monolithic `AppView` (32 fields) are strictly forbidden.
- **`AppState` Container**: `AppState` (`crates/app/src/state/app_state.rs`) is solely an initialization container that owns 9 decoupled GPUI entities:
  1. `workspace: Entity<WorkspaceDataState>`: Pure domain data (collections, requests, folders, pins).
  2. `env: Entity<EnvironmentState>`: Environment variables, active environment, variable merging.
  3. `persistence: Entity<PersistenceState>`: File I/O, disk watcher, conflict detection, dirty tracking.
  4. `sidebar: Entity<SidebarState>`: Sidebar width, collapse states, history navigation, renaming controls.
  5. `modal: Entity<ModalState>`: Modal visibility flags, theme preference, split vertical layout.
  6. `git: Entity<GitState>`: VCS operations, staging, commit, diff, branches, remotes.
  7. `session: Entity<RequestSessionState>`: Active request pointer, editor layout modes.
  8. `response: Entity<ResponseState>`: Active response, test results, console logs, LRU cache.
  9. `toast: Entity<ToastState>`: Notification toasts.
- **No Prop-Drilling**: Do not pass `AppState` into child components. Child views must receive only the specific `Entity<*State>` they observe or mutate.

### 2.2 Single-Active Request & On-Demand Borrowing
- `RequestSessionState` only stores `active_request_id: Option<String>`.
- **Never duplicate request data**: Never store a full `HttpRequestItem` clone in `RequestSessionState`.
- All composer tabs, header tables, and pipelines query and borrow `&HttpRequestItem` directly from `WorkspaceDataState`. Scratch requests are managed via `WorkspaceDataState::scratch_request`.

### 2.3 Pure GPUI Rendering & Event-Driven Subscriptions (PERF-006)
- **`AppView::render()` must be 100% pure element tree construction**:
  - NEVER call data synchronization routines (`sync_*`), disk reads, or state mutations inside `render()`.
  - Rendering must run at 60/120 FPS with zero overhead.
- **Event-Driven Subscriptions**: All state synchronization between domain entities and UI input fields (`ComposerInputs`, `InspectorDisplays`) must be handled via `cx.observe_in()` in `crates/app/src/app/subscriptions.rs`.

### 2.4 Non-Blocking Asynchronous Concurrency
- **UI Thread Safety**: Never execute blocking I/O, `std::process::Command`, filesystem directory scans, or JavaScript evaluation on the main UI thread.
- Always dispatch background work using `cx.background_executor().spawn(...)`.
- Git operations in `services/git_service.rs` must set `loading` or `is_syncing` (via `GitState`) and execute asynchronously.

### 2.5 Bounded Memory Caching (SL-002, SL-006)
- All session caches storing responses or heavy payloads must use `zest_core::cache::LruMemoryCache` with dual bounds:
  - Max entry count (e.g., 100 entries).
  - Max memory capacity (e.g., 50 MB total byte size).
- Responses and payloads must be wrapped in `Arc<HttpResponseRecord>` to achieve zero-copy $O(1)$ clones between background executors and UI states.

### 2.6 Local-First Storage & Schema Integrity (SL-005)
- All storage models in `zest_core::storage::models` must specify `#[serde(deny_unknown_fields)]`. Never add dynamic catch-all fields like `extra: BTreeMap`.
- File writes initiated by the application must register paths with `suppressed_paths` in `FsWorkspaceStore` to avoid spurious file-watcher echo loops.

---

## 3. Code Quality & Contribution Standards

### 3.1 Clippy & Static Analysis
- Every change must pass `cargo clippy --all-targets -- -D warnings` with **0 warnings**.
- Never add `#![allow(dead_code)]` or `#![allow(clippy::...)]` at the crate level. Address the root cause or scope `#[allow(...)]` to the exact minimal function/block with an explanatory comment.

### 3.2 Error Handling & Panic Prevention (TD-07 / TD-005)
- Never use `.unwrap()` on Mutex locks:
  ```rust
  // CORRECT:
  let store = self.cookie_store.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
  ```
- Use `Result` and `?` propagation in `zest-core`. In UI callbacks, present user-friendly errors via `ToastState` or error badges.

### 3.3 Documentation & Language
- **Source Code**: 100% English for all code comments, docstrings, struct field descriptions, error messages, and git commit messages.
- **Documentation**: Vietnamese or bilingual is preserved in `docs/` (`docs/specs/`, `docs/technical-debt.md`).

### 3.4 Modularity & File Size Limit
- Keep source files under **~650 LOC**.
- When a component or state grows, modularize it into a dedicated folder (e.g., `components/git/`, `components/sidebar/`, `app/`, `state/`).

---

## 4. Planning & Plan Artifacts Protocol

To ensure all architectural and implementation decisions are preserved across agent sessions and tracked in version control:
- **Mandatory Plan Storage**: Whenever creating or updating an implementation plan (via `/plan`, brain artifacts, or design proposals), the agent **MUST** write/sync a persistent copy directly into the repository at:
  ```
  docs/plans/YYYYMMDD-<plan-name>.md
  ```
  *(Example: `docs/plans/20261022-collection-runner-phase6.md`)*
- **Format Requirements**:
  - Date prefix `YYYYMMDD` using current local date (e.g. `20261022`).
  - Kebab-case descriptive artifact name.
  - Standard markdown structure: Goal, Context, Architectural Decisions, Step-by-Step Implementation Tasks, Risk Mitigation, and Verification Steps.
- **Git Tracking**: All plans stored in `docs/plans/` are part of the repository history and must be committed alongside the corresponding implementation.

---

## 5. Skills Directory & Runbooks

Specialized runbooks and checklists are located in `.agents/skills/`:

| Skill | Focus Area |
| :--- | :--- |
| [`gpui-architecture`](.agents/skills/gpui-architecture/SKILL.md) | Multi-entity state, event-driven subscriptions, GPUI input handles, component modularization |
| [`http-pipeline-engine`](.agents/skills/http-pipeline-engine/SKILL.md) | Request executor, connection pooling, variable interpolation, auth resolution, multi-protocol specs |
| [`scripting-automation`](.agents/skills/scripting-automation/SKILL.md) | QuickJS (`rquickjs`) engine, `z.*` API, pre/post scripts, assertions, dynamic fake variables |
| [`local-first-storage-vcs`](.agents/skills/local-first-storage-vcs/SKILL.md) | Local-first YAML persistence, file watcher suppression, secrets splitting, async Git operations |
| [`interop-spec-converters`](.agents/skills/interop-spec-converters/SKILL.md) | OpenAPI 3.0/3.1, Postman v2.1, Insomnia v4, cURL parsers, cycle-safe resolvers, code generators |
| [`tech-debt-auditor`](.agents/skills/tech-debt-auditor/SKILL.md) | Debt registry auditing, anti-pattern checklists, memory leak prevention, async conversions |

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
