---
name: gpui-architecture
description: >-
  Use this skill when developing, refactoring, or reviewing GPUI components, state management,
  event-driven subscriptions, input handle synchronization, and view layouts in crates/app.
---

# GPUI State & View Architecture Guide

This skill guides the design, implementation, and maintenance of the GPUI-based UI layer in `crates/app`. It encapsulates the architecture forged through refactoring legacy God Objects (`WorkspaceState`, `AppView`) and eliminating observer polling anti-patterns.

---

## 1. Multi-Entity State Pattern

`crates/app/src/state/` divides application state into 9 isolated GPUI `Entity<T>` states owned by `AppState` (`state/app_state.rs`), plus supporting event and persistence coordinators:

| Entity / Module | Primary Responsibility | File Location |
| :--- | :--- | :--- |
| `WorkspaceDataState` | Pure domain entities: requests, folders, pins, workspace metadata, in-memory tree | `state/workspace_data_state.rs`, `state/workspace_crud.rs` |
| `EnvironmentState` | Environments, `globals`, active env ID, variable caching (`effective_environment`) | `state/environment_state.rs` |
| `PersistenceState` | Write-behind persistence (`PersistenceCoordinator`), file watcher (`WorkspaceWatcher`), dirty/conflict tracking | `state/persistence_state.rs`, `state/persistence_coordinator.rs` |
| `SidebarState` | Sidebar layout, collapse states, back/forward history navigation, generic `RenamingState<T>` | `state/sidebar_state.rs` |
| `ModalState` | Modal overlay visibility (`env`, `git`, `settings`, `import`, `scope_settings`), theme preference | `state/modal_state.rs` |
| `GitState` | Git VCS status cache, staging/diff/branches/remotes data, modal & tab state; CLI commands dispatched via `GitService` (`services/git_service.rs`) | `state/git_state.rs` |
| `RequestSessionState` | Pointer to active request (`active_request_id`), active composer tabs, in-flight send status | `state/request_session_state.rs` |
| `ResponseState` | Active HTTP response, bounded LRU cache (`ResponseCache`), cached formatted bodies, test results, console logs | `state/response_state.rs` |
| `ToastState` | Transient notification banners and error toasts | `state/toast_state.rs` |
| `workspace_events` | Typed `EventEmitter` payloads (`WorkspaceEvent`, `SessionEvent`, `ResponseEvent`, `EnvironmentEvent`, `ComposerSyncScope`) | `state/workspace_events.rs` |

### Rules for State Management:
1. **Never create a monolithic state facade**: Each entity notifies independently (`cx.notify()`). Views observe only the entities they render.
2. **No Prop-Drilling `AppState`**: Child views must accept only the specific `Entity<*State>` handles they read or mutate (or a dedicated context struct like `AuthTabContext<'a>` / `ModalOverlayEntities<'a>` when grouping handles).
3. **Single Source of Truth for Requests (SL-003)**: `RequestSessionState` only stores `active_request_id: Option<String>`. Never clone a full `HttpRequestItem` into session state. Query `WorkspaceDataState::find_request(&id)` on demand.
4. **Use Generic Renaming Pattern (SL-004)**: Always use `RenamingState<T>` (`target: T`, `input: Entity<InputState>`, `_subscription: Subscription`) rather than scattered ad-hoc input handles.

---

## 2. Dual-Channel Event & Redraw Architecture

Zest strictly separates **view redraw notifications** from **cross-entity data synchronization**:

1. **Channel 1 — Typed Command / Event Dispatch (`cx.emit` + `cx.subscribe` / `cx.subscribe_in`)**:
   - Used for all cross-entity coordination, auto-save scheduling, and synchronizing domain models into UI `InputState` handles (`ComposerInputs`, `InspectorDisplays`).
   - State entities emit typed events (`WorkspaceEvent`, `SessionEvent`, `ResponseEvent`, `EnvironmentEvent`) defined in `crates/app/src/state/workspace_events.rs`.
   - Subscribed in `crates/app/src/app/subscriptions.rs` (`setup_app_subscriptions`) and routed through `crates/app/src/app/workspace_event_handler.rs` (`handle_workspace_event`, `handle_session_event`, `handle_response_event`, `handle_environment_event`).
2. **Channel 2 — Pure Redraw Observers (`cx.notify` + `cx.observe`)**:
   - `cx.observe` closures in `subscriptions.rs` must ONLY trigger `cx.notify()` to schedule a frame redraw (`cx.observe(entity, |_, _, cx| cx.notify())`).
   - **Strictly Forbidden**: Never store or diff intermediate polling flags (`last_synced_*`, `*_version`, `is_dirty`) inside `cx.observe` closures or inside `AppView::render()`.

```rust
// ✅ CORRECT: Typed event subscription + pure redraw observer in subscriptions.rs
subscriptions.push(cx.subscribe_in(
    editor_state,
    window,
    |this, _ed, event: &SessionEvent, window, cx| {
        handle_session_event(this, event, window, cx);
    },
));

// Pure redraw observer — no state reads, no side effects
subscriptions.push(cx.observe(editor_state, |_, _, cx| cx.notify()));
```

---

## 3. UI Component Patterns & Modularity

1. **Max File Size**: Target **< 650 LOC** per source file. When a component grows beyond this threshold, split it into a directory module (e.g., `components/sidebar/`, `components/git/`, `components/body_editor/`, `components/settings_modal/`, `components/env_autocomplete/`).
2. **Environment Variable Input & Autocomplete**:
   - Single-line inputs supporting `{{variable}}` interpolation use `EnvInput` (`components/env_input.rs`) paired with `EnvAutocomplete` (`components/env_autocomplete/`), providing inline token highlighting, compact variable hover tooltips, and keyboard-navigable autocomplete popups.
3. **Index-Based Key-Value Tables**:
   - Params, Headers, Form, and Multipart tables (`components/kv_table.rs`, `components/body_editor/multipart.rs`) synchronize `InputState` pools by row index and request revision without recreating inputs on every keystroke.
4. **Read-Only Ancestor Header & Auth Projection**:
   - `components/inherited_headers.rs` and `components/scope_settings_modal.rs` display inherited Workspace/Folder headers and auth using `zest_core::http::hierarchy::resolve_inherited_headers`.
5. **Virtualized Sidebar Tree**:
   - `components/sidebar/flat_tree.rs` flattens visible nodes into a precomputed slice so large collections render at 120 FPS.

---

## 4. Verification & Testing

Whenever modifying GPUI components, state entities, or event handlers:
```bash
# 1. Run all unit tests for app components, states, and event flows
cargo test --package zest --bin zest

# 2. Verify with strict clippy
cargo clippy --package zest --bin zest -- -D warnings
```
