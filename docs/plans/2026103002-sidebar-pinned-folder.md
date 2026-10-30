# Implementation Plan: Sidebar Pinned Folder

## 1. Goal Description

Currently, the "Pinned" section in the sidebar renders as a static text header (`📌 PINNED`) that cannot be collapsed or expanded by the user.

The requirements are:
1. Transform "Pinned" in the sidebar into an interactive folder that the user can actively open and close.
2. Remove the pin icon (`📌`) and replace it with a folder disclosure arrow (`ChevronRight` when collapsed, `ChevronDown` when open).
3. Preserve the clean behavior: only display the pinned section/folder if there are pinned items (`pins` is not empty and resolves to requests in the workspace). When there are no pinned items, do not render anything in the sidebar.
4. When expanded, show the pinned items indented appropriately as folder children. When collapsed, hide the pinned request items.

---

## 2. User Review Required

> [!NOTE]
> The pinned section will default to expanded (`pinned_collapsed: false`) when pins exist, preserving the immediate visibility of user-pinned requests, but allowing one-click collapse/expand just like any normal folder in the sidebar.

---

## 3. Proposed Changes

### State Management (`crates/app/src/state/sidebar_state.rs`)
- Add `pub pinned_collapsed: bool` to `SidebarState` (default: `false`).
- Add `pub fn toggle_pinned_collapsed(&mut self, cx: &mut Context<Self>)` that toggles `pinned_collapsed`, increments `ui_version`, and calls `cx.notify()`.
- Pass `self.pinned_collapsed` from `SidebarState::cached_flat_rows` into `flatten_sidebar_tree`.

### Flat Tree Projection (`crates/app/src/components/sidebar/flat_tree.rs`)
- Update enum variant `FlatSidebarRow::PinnedHeader` to `FlatSidebarRow::PinnedHeader { is_collapsed: bool }`.
- Update `flatten_sidebar_tree` parameter list to take `pinned_collapsed: bool`.
- When `!pinned_indices.is_empty()`:
  - Compute `let is_collapsed = pinned_collapsed && !has_query;`.
  - Push `FlatSidebarRow::PinnedHeader { is_collapsed }`.
  - Only push `FlatSidebarRow::PinnedRequest` entries if `!is_collapsed`.
  - If `pins` is empty or no pinned requests exist, neither `PinnedHeader` nor `PinnedRequest` is added.
- Update and add unit tests covering expanded and collapsed states.

### Sidebar Row Rendering (`crates/app/src/components/sidebar/mod.rs`)
- Replace the static `📌 PINNED` `div()` with an interactive folder header row:
  - Click handler calling `sidebar.toggle_pinned_collapsed`.
  - Remove "📌" emoji/icon.
  - Insert 14x14 disclosure chevron (`IconName::ChevronRight` when collapsed, `IconName::ChevronDown` when expanded).
  - Title "Pinned" using `.SystemMonoFont`, `SEMIBOLD`, and `text_secondary`.
  - Hover background highlighting (`t.bg_hover`) and cursor pointer.
- For `FlatSidebarRow::PinnedRequest`, indent using `.pl(px(20.))` (level 1 indentation) to visually match folder child items.

---

## 4. Verification Plan

### Automated Tests
- Run `cargo test crates/app/src/components/sidebar/flat_tree.rs` to verify flat tree row generation and collapse logic.
- Run `cargo test` across the workspace.
- Run `cargo clippy --all-targets -- -D warnings` to verify zero warnings.
- Run `cargo fmt --check` to verify code formatting.

### Manual Verification
- Launch application, pin requests -> "Pinned" folder appears with chevron down.
- Click "Pinned" header -> chevron changes to right, pinned items collapse.
- Click again -> expands, showing items.
- Unpin all requests -> "Pinned" folder disappears entirely.
