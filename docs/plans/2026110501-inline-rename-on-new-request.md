# Plan: Unify New Request Behavior with New Folder (Inline Rename on Creation)

**Date**: 2026-11-05
**Status**: Completed

## 1. Goal
When creating a new folder in Zest, an inline rename input is immediately activated so the user can name the folder without having to manually trigger rename. In contrast, creating a new request currently leaves the request pre-named as "New Request" without opening the rename input. This plan unifies the creation UX so that creating a new request immediately focuses an inline rename input with the default text selected, matching the behavior of "New Folder".

## 2. Context & Root Cause Analysis
- In `crates/app/src/components/sidebar/mod.rs`:
  - `PopupMenuItem::new("New Folder")` calls `ws.create_folder` and immediately activates `sb.start_renaming_folder(...)` with window context.
  - `PopupMenuItem::new("New Request")` only calls `ws.create_request` and `ed.set_active_request_id`, leaving the request in static read-only label mode.
  - Empty state "+ New Request" button also only calls `create_request` and `set_active_request_id`.
- In `crates/app/src/components/sidebar/folder_node.rs`:
  - Context menu "New Folder" calls `ws.create_folder` and activates `sb.start_renaming_folder(...)`.
  - Context menu "New Request" only calls `create_request` and `set_active_request_id`.
  - Additionally, creating an item inside a collapsed folder should expand the folder so the new inline rename item is visible in the tree.

## 3. Implementation Steps
1. **Sidebar Header Menu** (`crates/app/src/components/sidebar/mod.rs`):
   - Update `PopupMenuItem::new("New Request")` click handler to accept `window`.
   - Clone `sb_menu` as `sb_req`.
   - Call `sb_req.update(cx, |state, cx| state.start_renaming_request(req.id, req.name, ws_c, window, cx))`.

2. **Sidebar Empty State Button** (`crates/app/src/components/sidebar/mod.rs`):
   - Clone `sb_clone` for the empty state closure.
   - Update on_click to receive `window`.
   - Call `sb.start_renaming_request(req.id, req.name, ws_c, window, cx)`.

3. **Folder Context Menu** (`crates/app/src/components/sidebar/folder_node.rs`):
   - Pass `sb_menu` into "New Request" item handler.
   - Expand the parent folder if collapsed (`state.collapsed_folders.remove(&f_id_n)`).
   - Call `sb_n.update(cx, |state, cx| state.start_renaming_request(req.id, req.name, ws_c, window, cx))`.
   - Ensure "New Folder" in context menu also expands the parent folder (`state.collapsed_folders.remove(&f_id_ns)`).

## 4. Verification
- Run `cargo fmt --check`.
- Run `cargo test`.
- Run `cargo clippy --all-targets -- -D warnings`.
