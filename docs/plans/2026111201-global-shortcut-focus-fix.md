# Implementation Plan: Global Keyboard Shortcuts Focus Resolution

- Date: 2026-11-12
- Plan File: `docs/plans/2026111201-global-shortcut-focus-fix.md`

## 1. Goal
Fix keyboard shortcuts (such as `Ctrl+K` on Windows/Linux or `Cmd+K` on macOS, `Ctrl+Enter` / `Cmd+Enter`, `Ctrl+B` / `Cmd+B`, etc.) not triggering when the mouse cursor/focus is not positioned inside any text input or editor.

## 2. Root Cause Analysis
In GPUI, key events (`KeyDownEvent`) and actions registered via `cx.bind_keys` are dispatched along the active dispatch path:
```rust
let node_id = self.focus_node_id_in_rendered_frame(self.focus);
let dispatch_path = self.rendered_frame.dispatch_tree.dispatch_path(node_id);
```
1. `AppView` currently does not create or own a `FocusHandle`.
2. The root container `div()` in `AppView::render` does not track focus (`.track_focus(...)`).
3. When the application launches, or when a user clicks non-input elements (sidebar, response inspector, status badges, background), or when a modal/palette closes, `self.focus` is `None` (or points to an unmounted element ID).
4. When `self.focus` is `None`, `focus_node_id_in_rendered_frame` defaults to `self.rendered_frame.dispatch_tree.root_node_id()` (`DispatchNodeId(0)`).
5. The `dispatch_path` from `DispatchNodeId(0)` contains only `[DispatchNodeId(0)]` and never traverses downward to `root` or child elements.
6. Since all `.on_action(...)` handlers and `root.on_key_down` are attached to `root` (`div()`), GPUI finds no action handlers or key listeners on `dispatch_path`.
7. As a result, all window-level shortcuts fail to trigger unless an input element holds focus.

## 3. Implementation Steps
1. **ModalState Helper**:
   - In `crates/app/src/state/modal_state.rs`, add `pub fn has_any_modal_open(&self) -> bool` to check if any modal or dialog is currently presented.
2. **AppView FocusHandle & Track Focus**:
   - In `crates/app/src/app/mod.rs`:
     - Add `pub(crate) focus_handle: FocusHandle` to `AppView`.
     - In `AppView::new(window, cx)`:
       - Allocate `let focus_handle = cx.focus_handle()`.
       - Immediately focus it with `window.focus(&focus_handle, cx)`.
       - Subscribe to `cx.on_focus_lost(window, ...)` to restore focus to `this.focus_handle` whenever a focused child (such as `CommandPalette`'s search input) is dismissed or unmounted.
     - Implement `Focusable` for `AppView`.
     - In `AppView::render`:
       - Add `.id("app-root").track_focus(&self.focus_handle)` to `root`.
       - In `root.on_key_down`, support `Ctrl+Enter` / `Cmd+Enter` to reliably execute `send_current_request` regardless of whether an input is focused or not.
3. **Verification**:
   - Run `cargo fmt --check`.
   - Run `cargo clippy --all-targets -- -D warnings`.
   - Run `cargo test`.
