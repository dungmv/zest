---
name: gpui-architecture
description: >-
  Use this skill when developing, refactoring, or reviewing GPUI components, state management,
  event-driven subscriptions, input handle synchronization, and view layouts in crates/app.
---

# GPUI State & View Architecture Guide

This skill guides the design, implementation, and maintenance of the GPUI-based UI layer in `crates/app`. It encapsulates the learnings from refactoring the legacy God Objects (`WorkspaceState`, `AppView`) into modular entities and event-driven pipelines.

---

## 1. Multi-Entity State Pattern

`crates/app/src/state/` divides application state into 9 isolated GPUI `Entity<T>` states managed by `AppState`:

| Entity State | Primary Responsibility | File Location |
| :--- | :--- | :--- |
| `WorkspaceDataState` | Pure domain entities: requests, folders, pins, workspace metadata, in-memory tree | `state/workspace_data_state.rs`, `state/workspace_crud.rs` |
| `EnvironmentState` | Environments, global vars, active env ID, variable caching, disk save debounce | `state/environment_state.rs` |
| `PersistenceState` | Disk persistence, file watcher (`FsWatcher`), dirty flags, external conflict detection | `state/persistence_state.rs` |
| `SidebarState` | Sidebar layout, collapse states, back/forward history navigation, inline renaming | `state/sidebar_state.rs` |
| `ModalState` | Modal overlay visibility (`env`, `git`, `settings`, `import`, `cookies`), theme preference | `state/modal_state.rs` |
| `GitState` | Git VCS status cache, staging/diff/branches/remotes data, modal & tab state; CLI commands dispatched via `GitService` (`services/git_service.rs`, Runner-style) | `state/git_state.rs` |
| `RequestSessionState` | Pointer to active request (`active_request_id`), UI tab selections | `state/request_session_state.rs` |
| `ResponseState` | Active HTTP response, LRU cache (`ResponseCache`), test results, logs | `state/response_state.rs` |
| `ToastState` | Transient notification banners and error toasts | `state/toast_state.rs` |

### Rules for State Management:
1. **Never create a monolithic state facade**: Each entity must notify independently (`cx.notify()`). Components should observe only the entities they care about to avoid unnecessary re-renders.
2. **No Prop-Drilling `AppState`**: Views should take only the exact `Entity<*State>` they need as arguments.
3. **Single Source of Truth for Requests**: `RequestSessionState` only stores `active_request_id: Option<String>`. Never clone a full request struct into session state (SL-003). Query `WorkspaceDataState::find_request(&id)` on demand.
4. **Use Generic Renaming Pattern**: Always use `RenamingState<T>` (`id: T`, `input: Entity<InputState>`, `subscription: Subscription`) rather than scattered ad-hoc input handles (SL-004).

---

## 2. Event-Driven Synchronization (PERF-006)

### Anti-Pattern: Syncing inside `render()`
```rust
// ❌ STRICTLY FORBIDDEN: NEVER DO THIS IN GPUI
impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_active_request_editors(window, cx); // ❌ Causes frame drops and echo loops!
        ...
    }
}
```

### Best Practice: Dedicated Subscriptions in `subscriptions.rs`
All editor inputs (`ComposerInputs`) and display panels (`InspectorDisplays`) are updated exclusively through `cx.observe_in()` subscriptions registered in `crates/app/src/app/subscriptions.rs`:

```rust
// ✅ CORRECT: Event-driven observer pattern
pub fn register_subscriptions(
    state: &AppState,
    composer_inputs: &mut ComposerInputs,
    inspector_displays: &mut InspectorDisplays,
    cx: &mut Context<AppView>,
) -> Vec<Subscription> {
    let mut subs = Vec::new();

    // Observe active request pointer changes
    subs.push(cx.observe_in(&state.session, |view, session, window, cx| {
        let active_id = session.read(cx).active_request_id.clone();
        view.composer_inputs.sync_for_request(active_id.as_deref(), &view.state, window, cx);
    }));

    // Observe response arrival
    subs.push(cx.observe_in(&state.response, |view, resp, window, cx| {
        let response_record = resp.read(cx).current_response.clone();
        view.inspector_displays.sync_response(response_record.as_deref(), window, cx);
    }));

    subs
}
```

---

## 3. UI Component Structure & Limits

1. **Max File Size**: Keep all component files under **~650 LOC**. When a component grows, modularize it into a directory package (e.g., `components/sidebar/`, `components/git/`, `app/`).
2. **Highlighter Factory**: Code editor syntax highlighters (JSON, XML, JavaScript) must be passed via shared `HighlighterFactory` (`Rc<dyn Fn(&str) -> Option<Box<dyn InputHighlighter>>>`).
3. **Resizable Panels**: Use `gpui_component::resizable::{h_resizable, v_resizable, resizable_panel}` for customizable layouts (sidebar, response inspector split).

---

## 4. Verification & Testing

Whenever touching GPUI components or states:
```bash
# 1. Run all unit tests for app components & states
cargo test --package zest --bin zest

# 2. Verify with clippy
cargo clippy --package zest --bin zest -- -D warnings
```
