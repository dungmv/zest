---
name: gpui-architecture
description: >-
  Use this skill when developing, refactoring, or reviewing GPUI components, state management,
  event-driven subscriptions, input handle synchronization, view layouts, gpui-kit unified
  dependency patterns, theme token synchronization, and platform input pitfalls in crates/app.
---

# GPUI State & View Architecture Guide

This skill guides the design, implementation, and maintenance of the GPUI-based UI layer in `crates/app`. It encapsulates the architecture forged through refactoring legacy God Objects (`WorkspaceState`, `AppView`), the `gpui-kit` unified dependency + component migration (Cycle 2), centralized theme synchronization, and the Windows/platform input fixes.

---

## 1. Unified GPUI Ecosystem: `gpui-kit` (Single Dependency Rule)

`crates/app` relies strictly on **`gpui-kit`** as the single unified crate for GPUI primitives, UI components, and bundled assets. Direct dependencies on `gpui`, `gpui-component`, or `gpui-kit-assets` in `crates/app/Cargo.toml` or application source code are **strictly forbidden**.

### Why `gpui-kit`?
- **Prevents Version Split & Resolution Conflicts**: Avoids duplicate or conflicting versions of layout engines (`taffy`), text shaping (`rustybuzz`), and GPUI runtime internals across separate `gpui` and `gpui-component` crates.
- **Unified Feature Flags**: Bundles all necessary features (`component`, `assets`, `test-support`) under a single verified package version.

### Cargo Dependency Setup:
```toml
# crates/app/Cargo.toml
[dependencies]
gpui-kit = { version = "0.9.1", features = ["component", "assets"] }

[dev-dependencies]
gpui-kit = { version = "0.9.1", features = ["test-support"] }
```

### Import Conventions & Namespace Mapping:
Always import through `gpui_kit` namespaces:

| Purpose | Replaces Legacy Import | ✅ Modern `gpui-kit` Import |
| :--- | :--- | :--- |
| **Core GPUI Primitives & Macro** | `use gpui::{div, px, App, Div, Entity, ...};` | `use gpui_kit::{div, px, App, Div, Entity, ParentElement, Styled, Window, ...};` |
| **Actions Macro** | `use gpui::actions;` | `use gpui_kit::actions;` |
| **Traits & Builders** | `use gpui::prelude::FluentBuilder;` | `use gpui_kit::prelude::FluentBuilder as _;`<br>`use gpui_kit::AppContext as _;` |
| **UI Components** | `use gpui_component::button::Button;`<br>`use gpui_component::input::InputState;` | `use gpui_kit::component::button::{Button, ButtonVariants as _};`<br>`use gpui_kit::component::input::{EditorState, InputState, TextareaState};`<br>`use gpui_kit::component::resizable::{h_resizable, resizable_panel, v_resizable};` |
| **Icons & Assets** | `use gpui_component::Icon;`<br>`use gpui_assets::IconName;` | `use gpui_kit::component::Icon;`<br>`use gpui_kit::assets::IconName;` |

```rust
// ❌ WRONG: Direct imports from gpui or gpui-component
use gpui::{div, px, App, Entity, Window};
use gpui_component::button::Button;

// ✅ CORRECT: Unified gpui-kit imports
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{div, px, App, Div, Entity, ParentElement, Styled, Window};
use gpui_kit::component::button::{Button, ButtonVariants as _};
```

---

## 2. Multi-Entity State Pattern

`crates/app/src/state/` divides application state into 9 isolated GPUI `Entity<T>` states owned by `AppState` (`state/app_state.rs`), plus supporting event and persistence coordinators:

| Entity / Module | Primary Responsibility | File Location |
| :--- | :--- | :--- |
| `WorkspaceDataState` | Pure domain entities: requests, folders, pins, workspace metadata, in-memory tree | `state/workspace_data_state.rs`, `state/workspace_crud.rs`, `state/workspace_folder_crud.rs` |
| `EnvironmentState` | Environments (sorted alphabetically on load), `globals`, active env ID, variable caching (`effective_environment`) | `state/environment_state.rs` |
| `PersistenceState` | Write-behind persistence (`PersistenceCoordinator`), file watcher (`WorkspaceWatcher`), dirty/conflict tracking | `state/persistence_state.rs`, `state/persistence_coordinator.rs`, `state/watcher_reconciler.rs` |
| `SidebarState` | Sidebar layout, collapse states, pinned-folder collapse, multi-selection sets, history navigation, generic `RenamingState<T>` (grouped in `SidebarRenaming`) | `state/sidebar_state.rs` |
| `ModalState` | Modal overlay visibility (env, git, settings, import, workspace settings, about), theme preference, update-check status; fields grouped into `NetworkPrefs`, `EnvModalUi`, `ImportModalUi` | `state/modal_state.rs` |
| `GitState` | Git VCS status cache, staging/diff/branches/remotes data, modal & tab state (grouped in `GitModalUiState`); CLI commands dispatched via `GitService` | `state/git_state.rs` |
| `RequestSessionState` | Pointer to active request (`active_request_id`), active composer tabs, in-flight send status | `state/request_session_state.rs` |
| `ResponseState` | Active HTTP response, realtime frames (`Vec<TransportFrame>`), bounded LRU cache (`ResponseCache`), cached formatted bodies, test results, console logs | `state/response_state.rs` |
| `ToastState` | Transient notification banners and error toasts | `state/toast_state.rs` |
| `workspace_events` | Typed `EventEmitter` payloads (`WorkspaceEvent`, `SessionEvent`, `ResponseEvent`, `EnvironmentEvent`, `ModalEvent`) | `state/workspace_events.rs` |

### Rules for State Management:
1. **Never create a monolithic state facade**: Each entity notifies independently (`cx.notify()`). Views observe only the entities they render. State structs stay ≤ 15 fields by grouping related fields into sub-structs (see `tech-debt-auditor` skill).
2. **No Prop-Drilling `AppState`**: Child views must accept only the specific `Entity<*State>` handles they read or mutate (or a dedicated context struct like `AuthTabContext<'a>` / `ModalOverlayEntities<'a>` when grouping handles).
3. **Single Source of Truth for Requests (SL-003)**: `RequestSessionState` only stores `active_request_id: Option<String>`. Never clone a full `HttpRequestItem` into session state. Query `WorkspaceDataState::find_request(&id)` on demand.
4. **Use Generic Renaming Pattern (SL-004)**: Always use `RenamingState<T>` rather than scattered ad-hoc input handles.
5. **Sort/derive presentation data once at the state layer** (e.g. environments sorted by `name.to_lowercase()` in `EnvironmentState::reload_environments`), never per-frame in `render()`.
6. **Shared row/table mutations go through `state::edit_active_request`** (free fn in `state/workspace_crud.rs`) instead of duplicating borrow-update-emit boilerplate per call site.

---

## 3. Dual-Channel Event & Redraw Architecture

Zest strictly separates **view redraw notifications** from **cross-entity data synchronization**:

1. **Channel 1 — Typed Command / Event Dispatch (`cx.emit` + `cx.subscribe` / `cx.subscribe_in`)**:
   - Used for all cross-entity coordination, auto-save scheduling, and synchronizing domain models into UI `InputState` handles (`ComposerInputs`, `InspectorDisplays`).
   - State entities emit typed events (`WorkspaceEvent`, `SessionEvent`, `ResponseEvent`, `EnvironmentEvent`, `ModalEvent`) defined in `crates/app/src/state/workspace_events.rs`.
   - Subscribed in `crates/app/src/app/subscriptions.rs` (`setup_app_subscriptions`) and routed through `crates/app/src/app/workspace_event_handler.rs`.
2. **Channel 2 — Pure Redraw Observers (`cx.notify` + `cx.observe`)**:
   - `cx.observe` closures in `subscriptions.rs` must ONLY trigger `cx.notify()` to schedule a frame redraw.
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
subscriptions.push(cx.observe(editor_state, |_, _, cx| cx.notify()));
```

---

## 4. UI Component Patterns & Modularity

1. **Max File Size — Target < 650 LOC per source file**. Split grown components into directory modules. Current umbrella modules (keep this layout when adding code):
   - `components/request_composer/` — composer shell + per-tab free render fns (`body_tab`, `message_tab`, `proto_tab`, `params_tab`, `headers_tab`, `auth_tab`, `scripts_tab`, `ws/grpc_message_tab`, `grpc_proto_tab`, `banners`, `tab_strip`, `composer_inputs`). Per-tab fns take `&AppView` and re-derive state snapshots locally (never hold borrows across fn boundaries).
   - `components/response_inspector/`, `components/script_editor/`, `components/workspace_setting_modal/` (ex-scope_settings_modal), `components/env_modal/`, `components/titlebar/`, `components/import_modal/`, `components/sidebar/`, `components/git/`, `components/body_editor/`, `components/settings_modal/`.
   - `components/completion/` — umbrella for all `CompletionProvider`s: `env_autocomplete/` (tokens + popup + detail card), `proto_json/` (gRPC JSON keys), `validation.rs` (gRPC diagnostics + gutter marks).
   - `components/common/` — shared composer helpers (`toolbar.rs`, `format_json.rs`, `status_pill.rs`); reuse these instead of duplicating toolbar/format/status builders.
2. **Environment Variable Input & Token Attachment** (`components/env_input.rs`, `components/completion/env_autocomplete/`):
   - `gpui-kit` never auto-tokenizes typed text. Retokenization runs in the `InputEvent::Change` subscription (`attach_missing_var_tokens`) and **must** be gated by a `RetokenizeGuard` (value-diff guard): skip value-identical `Change` events or undo is poisoned (each undo replays a conversion and re-attaches it forever).
   - `attach_missing_var_tokens` restores the caret itself (capture `cursor()` before, re-pin after) and uses the lean core scanner `zest_core::interpolation::scan_variable_spans` (regex only — no env map, no fake-var RNG) on the hot path. Full `scan_variables` is only for discrete actions (request switch, autocomplete pick, import) via `build_tokenized_content`.
   - Token hover shows a **native `Tooltip`** (`variable_value_tooltip`) attached inside the `.token(...)` closure — never a custom deferred absolute `div` with `use_keyed_state` hover tracking.
   - Only `TextareaMode` inputs get real chips; editor modes get tag-like highlighter styling (see `EditorMode`).
3. **Index-Based Key-Value Tables**: Params, Headers, Form, and Multipart tables (`components/kv_table.rs`, `components/body_editor/multipart/`) synchronize `InputState` pools by row index and request revision without recreating inputs on every keystroke. Flex rows that mix fixed + flexible columns must mark fixed columns `.flex_shrink_0()` and flexible containers `.min_w(px(0.))` to prevent flexbox squishing.
4. **Content-Type Sync & Body Actions**: Switching body type updates the `Content-Type` header through the domain helper `HttpRequestItem::update_content_type_header(Some(mime))` / `(None)` (case-insensitive replace or insert/remove). Row-add actions ("Add Part", "Add Row") live in the top toolbar as `Button`s (KvTable `show_add_button(false)`), not bottom buttons.
5. **Read-Only Ancestor Header & Auth Projection**: `components/inherited_headers.rs` and `components/workspace_setting_modal/` display inherited Workspace/Folder headers/auth using `zest_core::transport::hierarchy::resolve_inherited_headers`; overridden rows are hidden in favor of an info popover on the disabled inherited row.
6. **Virtualized Sidebar Tree & Interactions** (`components/sidebar/`):
   - `flat_tree.rs` flattens visible nodes into a precomputed slice (`FlatTreeInput` + `FlatTreeCtx`) so large collections render at 120 FPS. The Pinned section is a collapsible pseudo-folder (`FlatSidebarRow::PinnedHeader { is_collapsed }`) rendered only when pins resolve.
   - Multi-select (Ctrl/Cmd-click) stores `selected_requests`/`selected_folders` sets in `SidebarState` (seeded from the active request on first toggle); a multi-selection drag dispatches a single `DragMultiItem { request_ids, folder_ids }` payload, and context menus on selected items show only "Delete".
7. **Fixed-Height Modals**: `ModalDialog`s that switch tabs must pin an explicit `.w(...).h(...)` (e.g. import 640×520, workspace settings 580×320) and wrap scrollable content in `.flex_1().min_h(px(0.)).overflow_y_scrollbar()` — otherwise the card jumps/resizes between tabs. Remember `overflow_y_scroll` needs an `.id()`-ed element, and `vertical_scrollbar()` inserts a child that shifts `scroll_to_item` indices.
8. **State-Toggle Buttons Stay Ghost**: Boolean toggles (e.g. Line Wrap) render `.ghost()` in both states and signal the active state via the **icon tint** (`Icon::new(..).text_color(t.primary)` when on) — never by swapping the button to `.primary()` variant.

---

## 5. gpui-kit Component Catalog (Migration Standard)

All 24 UI component groups were migrated to `gpui-kit` standard components (Cycle 2). New UI **must** reuse these instead of hand-rolled `div()` constructions:

| Group | Use | Notes |
| :--- | :--- | :--- |
| Form controls | `Checkbox` (3-state), `Switch`, `Button`, `Radio/RadioGroup` | Selects/dropdowns = `Button` + `.dropdown_menu()` (theme mode, timeout, method, env, auth type, body format). Popover self-dismisses. |
| Navigation | `Tab/TabBar` (`.underline()`), `Breadcrumb`, `Collapsible`, `h_resizable`/`v_resizable` | Sidebar + composer/inspector splits use gpui-kit resizable controllers. |
| Feedback | `Alert`, `Empty`, `Tag`/`Badge`, `Toast` (`ToastContainer`), `Progress` (indeterminate in-flight), `Spinner`, `Skeleton` | Request in-flight = `Progress` bar + `Skeleton` shimmer rows; sidebar stays spinner-free. |
| Overlays | `ModalDialog` + `DialogHeader/Title/Description/Content/Footer` | All system modals standardized; backdrop dim, ESC + backdrop-click dismiss, mouse occlusion for free. |
| Menus | `menu::{ContextMenuExt, PopupMenuItem}` | 100% sidebar context menus with icons (`PopupMenuItem::icon` accepts a colored `Icon`). |
| Data display | `Kbd`, `Separator::horizontal()/vertical()`, `Diff` (split + unified), `Table` | Kbd auto-resolves macOS ⌘ / Windows Ctrl glyphs. |

**Button rules (hard-won)**:
- **Never call `.hover(...)` on a `gpui-kit` `Button`** — `Button::render` registers its own hover style and a second one panics ("hover style already set"). Delete-row buttons: ghost 24×24px, icon `t.text_muted`, hover red tint via variant/icon color, not custom hover.
- Don't restyle around kit defaults (e.g. `.ghost().bg(t.bg_card).border_1()...`); if a variant looks wrong, fix the **theme sync** (§6), not the call site.

---

## 6. Theme Token Centralization (`crates/app/src/theme.rs`)

Every color in the app flows through `AppTheme` tokens — **hardcoded `hex_to_hsla`/`rgb(...)` in components are forbidden**:

- Token families: surfaces (`bg_*`, `border*`, `text_*`), action variants (`primary/secondary/danger/warning/success/info` + hover/active/foreground), git status (`git_added/modified/deleted/untracked/ignored`), diff (`diff_added_bg/text`, `diff_deleted_bg/text`), timing waterfall (`timing_dns/tcp/tls/ttfb/download`), console logs (`log_*`), autocomplete tags (`tag_variable`, `tag_dynamic`), and method/status colors (`method_*`, `status_*` via `method_color()`/`protocol_color()`).
- `dark()` and `light()` palettes must both be tuned for contrast: colors designed for dark backgrounds (neon green/yellow) are not reused in light mode — use dedicated high-contrast light variants.
- `sync_component_theme()` maps the full `AppTheme` onto `gpui_kit::component::Theme::update(cx, ...)` (50+ `ThemeColor` properties: buttons, inputs, popover, menus, sidebar, title bar, tab bar, table). Kit components that look "wrong" are a **sync gap**, not a reason for local overrides. Always call `theme::sync_component_theme()` instead of raw `Theme::change`; popover bg comes from `ActiveTheme.colors.popover`.
- Direction for new UI is **monochrome**: no new colored accents; prefer icon shape/tint-from-existing-tokens over introducing new hues (method/protocol colors are legacy-restricted to existing badges/menus).

---

## 7. Global Shortcuts, Platform Input & API Pitfalls

### 7.1 Global Shortcut Dispatch (`app/mod.rs`)
- `AppView` owns `focus_handle: FocusHandle`, allocates and focuses it in `new()`, implements `Focusable`, and renders the root with `.id("app-root").track_focus(&self.focus_handle)`. Restore focus via `cx.on_focus_lost` when a child editor/palette unmounts.
- Without root focus tracking, GPUI's dispatch path defaults to `DispatchNodeId(0)` and **no** `.on_action` handler on root ever fires when no input holds focus — every window-level shortcut appears broken.
- Global combos that must fire regardless of input focus (Ctrl/Cmd+K palette, Ctrl/Cmd+Enter send) are intercepted in `root.on_key_down` with `cx.stop_propagation()`, because on Windows `Ctrl+K` yields `ToUnicode` control char `0x0B`, GPUI sets `prefer_character_input`, and `KeyBindings` are skipped while an input is focused. Local ESC handling must use `.on_action::<DismissModal>` on the wrapper (deepest bubble listener wins; a global escape binding consumes raw key events).

### 7.2 Windows Titlebar Drag Region (`components/titlebar/`)
- Under `WM_NCHITTEST`, any element inside a `.window_control_area(WindowControlArea::Drag)` parent without `HitboxBehavior::BlockMouse` resolves to `HTCAPTION` — Windows swallows the click as a window drag and never dispatches client mouse messages. Interactive elements need `.occlude()` + `on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())` (containers: `left_section`, `right_toolbar`, and child buttons).
- **Caveat**: do NOT occlude a `Button` wrapped by `dropdown_menu()` — the outer `Popup` wrapper must itself be hit-tested to register hover/toggle; block at the parent container level instead.

### 7.3 gpui-kit API Quirks (0.9.x)
- `on_hover` closure takes `&bool`; `overflow_y_scroll` requires `.id()`; `ScrollHandle::child_bounds` is private (no row-aligned tooltips — use the hovered-row + pane-hover gating pattern with diagnostics).
- `use_keyed_state` auto-observes the owning view (a render-path state change re-renders the whole view — avoid for hover-only state).
- `App::spawn` closures take a single `cx` param; a bare `cx.update(...)` is fine.
- `MutexGuard` held across `.await` makes the future `!Send` — clone out of the guard before awaiting.
- `render` match arms unify to the first arm's element type; wrap odd arms in a `div().child(...)`.
- `rfd` async file picker has no `Window` inside `cx.update` — re-sync editors via state instead.
- Never byte-slice user text in per-keystroke handlers (multi-byte UTF-8 panic across the window-procedure FFI boundary → `0xC0000409`); use `.get(..n)` and verify prefixes with `str` methods.
- Editor completions: `ShowCompletions` action (no default keybinding — bind ctrl-space yourself); providers attach via `lsp_mut().completion_provider`; `set_value` suppresses `Change` + clears undo (use `replace_all` for programmatic edits that must persist); diagnostics latch on hover — only push while the pane is hovered.

---

## 8. Verification & Testing

Whenever modifying GPUI components, state entities, or event handlers:
```bash
# 1. Run all unit tests for app components, states, and event flows
cargo test --package zest --bin zest

# 2. Verify with strict clippy
cargo clippy --package zest --bin zest -- -D warnings
```
