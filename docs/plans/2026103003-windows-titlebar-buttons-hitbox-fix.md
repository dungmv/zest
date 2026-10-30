# Implementation Plan: Windows Titlebar Buttons Hitbox & Click Fix

## 1. Goal Description

On Windows, the Settings button (`btn-settings`) and Split Layout button (`btn-split-toggle`) on the titlebar do not respond to clicks.

### Root Cause
1. `AppTitleBar` wraps the entire middle titlebar area (`left_section`, `center_section`, `right_toolbar`) inside a parent `drag_region` div configured with `.window_control_area(WindowControlArea::Drag)`.
2. On Windows, during `WM_NCHITTEST`, GPUI traverses the hitbox stack under the cursor. If an element does not have `HitboxBehavior::BlockMouse` (set via `.occlude()`), the hit test continues through to the parent `drag_region` hitbox, causing the OS hit-test to return `HTCAPTION`.
3. When Windows receives `HTCAPTION`, the Windows operating system intercepts the mouse click as a window drag action and never dispatches client-area mouse messages (`WM_LBUTTONDOWN`, `WM_LBUTTONUP`).
4. While `btn-toggle-sidebar` and `btn-titlebar-search` were explicitly marked with `.occlude()` and `.on_mouse_down(..., cx.stop_propagation())`, `right_toolbar` and its child buttons (`btn-split-toggle` and `btn-settings`) lacked `.occlude()` and `stop_propagation()`.
5. Additionally, `btn-workspace-switcher` in `left_section` also lacked `.occlude()` and `stop_propagation()`, leaving it vulnerable to the same Windows titlebar drag interception.

---

## 2. Proposed Changes

### `crates/app/src/components/titlebar.rs`
1. Update `right_toolbar`:
   - Add `.id("titlebar-right-toolbar")`
   - Add `.occlude()`
   - Add `.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())`
2. Update `btn-split-toggle`:
   - Add `.occlude()`
   - Add `.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())`
3. Update `btn-settings`:
   - Add `.occlude()`
   - Add `.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())`
   - Update shortcut tooltip to be cross-platform: `if is_macos { "Settings (⌘,)" } else { "Settings (Ctrl+,)" }`
4. Update `left_section` and `btn-workspace-switcher`:
   - Add `.id("titlebar-left-section")`, `.occlude()`, and `on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())` on `left_section`.
   - Do NOT add `.occlude()` or `stop_propagation()` directly on `Button::new("btn-workspace-switcher")`: `dropdown_menu` creates an outer `Popup` wrapper around `Button` that listens to `MouseDown` and requires `Popup` itself to be marked hovered in `hit_test.ids`. Placing `.occlude()` on the child `Button` causes hit-testing to break before reaching `Popup`, which prevented `Popup.is_hovered()` from being true and blocked `toggle_open`. Leaving `Button` unoccluded lets `left_section` perform the window drag blocking, matching `btn-env-breadcrumb`.

---

## 3. Verification Plan

1. Verify cargo check and cargo clippy with zero warnings:
   ```bash
   cargo clippy --all-targets -- -D warnings
   ```
2. Run test suite:
   ```bash
   cargo test
   ```
3. Add a unit test in `titlebar.rs` verifying titlebar state / button construction and attributes.
