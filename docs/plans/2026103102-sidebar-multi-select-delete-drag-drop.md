# Plan: Sidebar Multi-Select for Requests & Folders (Ctrl/Cmd), Context Menu Delete Only, & Drag-and-Drop Move

- **Date**: 2026-10-31
- **Sequence**: 02
- **File**: `docs/plans/2026103102-sidebar-multi-select-delete-drag-drop.md`

## 1. Goal & Requirements
Allow users to multi-select requests and folders in the sidebar by holding `Ctrl` (Windows/Linux) or `Cmd` (macOS). When multiple items are selected:
1. The right-click context menu on any selected item must show **only** "Delete" (deleting all selected items at once).
2. Users can drag and drop the multi-selection to move the items (both requests and folders) into a target folder, onto a request (moving to its parent folder), or to the root level.
3. Clean visual indication for multi-selected requests and folders using existing theme selection styles.

## 2. Architecture & Design Decisions

### 2.1 State Management in `SidebarState`
- Add `selected_requests: HashSet<String>` and `selected_folders: HashSet<String>`.
- Selection helpers:
  - `is_request_selected(&self, id: &str) -> bool`
  - `is_folder_selected(&self, id: &str) -> bool`
  - `selected_count(&self) -> usize`
  - `is_multi_selected(&self) -> bool`: returns `self.selected_count() > 1`.
  - `toggle_select_request(&mut self, req_id: &str, current_active_id: Option<&str>, cx: &mut Context<Self>)`
  - `toggle_select_folder(&mut self, folder_id: &str, current_active_id: Option<&str>, cx: &mut Context<Self>)`
  - `select_single_request(&mut self, req_id: &str, cx: &mut Context<Self>)`
  - `select_single_folder(&mut self, folder_id: &str, cx: &mut Context<Self>)`
  - `clear_selection(&mut self, cx: &mut Context<Self>)`
- Seeding logic: When starting a multi-selection (selection sets are empty) and the user Ctrl/Cmd-clicks an item, seed the currently active request (`active_request_id`) if applicable so that the initial item remains selected alongside the new selection.
- All selection updates increment `self.ui_version` and trigger `cx.notify()`.

### 2.2 Drag-and-Drop Multi-Payload
- Define `DragMultiItem` in `crates/app/src/components/sidebar/drag_drop.rs`:
  ```rust
  #[derive(Clone, Debug)]
  pub struct DragMultiItem {
      pub request_ids: Vec<String>,
      pub folder_ids: Vec<String>,
  }
  ```
- Render representation: compact badge showing an icon and count (e.g. "N items").
- Supported drop targets for `DragMultiItem`:
  - `folder_node.rs`: drops into `target_folder_id`.
  - `mod.rs` (RootDropzone): drops to root (`None`).
  - `request_node.rs`: drops into target request's folder (`target_req.folder_id`).
- When dragging an item that is part of a multi-selection, the drag payload dispatched is `DragMultiItem`. Single items without multi-select continue to dispatch `DragRequestItem` or `DragFolderItem`.

### 2.3 Context Menu & Bulk Delete
- Put `delete_multi_selected_items` in `crates/app/src/components/sidebar/item_actions.rs`.
- When right-clicking an item:
  - If the item is part of a multi-selection (`is_multi_selected` is true and item is selected):
    - Context menu contains ONLY "Delete" with `IconName::Trash`.
    - Clicking "Delete" calls `delete_multi_selected_items`, which deletes all selected folders and requests from `WorkspaceDataState`, updates `active_request_id` gracefully if deleted, and clears the selection.
  - If the item is NOT part of the selection:
    - Clear multi-selection and select this single item, presenting its full single-item context menu.

### 2.4 File Size & Modularity Guardrails
- `crates/app/src/components/sidebar/drag_drop.rs`: ~98 LOC -> ~135 LOC.
- `crates/app/src/components/sidebar/item_actions.rs`: ~39 LOC -> ~90 LOC.
- `crates/app/src/state/sidebar_state.rs`: ~403 LOC -> ~480 LOC.
- `crates/app/src/components/sidebar/folder_node.rs`: ~318 LOC -> ~380 LOC.
- `crates/app/src/components/sidebar/request_node.rs`: ~488 LOC -> ~550 LOC.
- `crates/app/src/components/sidebar/mod.rs`: ~571 LOC -> ~610 LOC.
- All files remain strictly below the 650 LOC ceiling.

## 3. Step-by-Step Implementation Tasks
1. **Extend `SidebarState`**:
   - Add `selected_requests` and `selected_folders`.
   - Add selection management and query helper methods.
2. **Define `DragMultiItem`**:
   - Add `DragMultiItem` struct and `Render` implementation in `drag_drop.rs`.
3. **Implement `delete_multi_selected_items`**:
   - Add bulk delete function in `item_actions.rs`.
4. **Update `SidebarRenderContext` in `mod.rs`**:
   - Pass `is_multi_select: bool`, `selected_requests: &'a HashSet<String>`, `selected_folders: &'a HashSet<String>` down to row renderers.
   - Add `DragMultiItem` drop handler to `RootDropzone`.
5. **Update `request_node.rs`**:
   - Ctrl/Cmd click toggles request in selection.
   - Click without modifier clears multi-select and selects single request.
   - Right-click preserves selection if item is selected; else selects single.
   - Multi-select context menu displays only "Delete".
   - Drag initiation dispatches `DragMultiItem` if item is in multi-selection.
   - Drop handler accepts `DragMultiItem`.
   - Visual highlighting for selected state.
6. **Update `folder_node.rs`**:
   - Ctrl/Cmd click toggles folder in selection.
   - Click without modifier clears multi-select and collapses/expands.
   - Right-click preserves selection if item is selected; else selects single folder.
   - Multi-select context menu displays only "Delete".
   - Drag initiation dispatches `DragMultiItem` if folder is in multi-selection.
   - Drop handler accepts `DragMultiItem`.
   - Visual highlighting for selected state.
7. **Verification**:
   - Run `cargo fmt --check`.
   - Run `cargo test`.
   - Run `cargo clippy --all-targets -- -D warnings`.
