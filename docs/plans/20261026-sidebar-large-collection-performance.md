# Plan: Sidebar Large Collection Performance (~350+ Requests & Folders)

**Date**: 2026-10-26  
**Status**: Approved & In Progress

---

## 1. Goal & Context

When a workspace contains ~350 requests and ~50 folders, the application experiences severe UI lag during sidebar scrolling, request switching, and typing in composer editors (URL, JSON/XML/Raw body, Pre/Post scripts).

### Root Causes Identified:
1. **$O(F^2 + F \cdot R \cdot G)$ Git Status & Tree Filtering per Render Frame** (`crates/app/src/components/sidebar/git_status.rs`, `folder_node.rs`, `item_actions.rs`):
   - `calculate_folder_git_status` computes descendant folders in $O(F^2)$ and scans all requests $O(R)$ at every folder level, calling `calculate_request_git_status` ($O(G)$ with 3 heap `String` allocations per git status file). For 350 requests and 350 git files across nested folders, this causes >1,000,000 heap allocations per frame.
   - `folder_has_matches` and child lookups perform repeated linear scans and `.to_lowercase()` allocations across all folders and requests.
2. **No Virtualization in Sidebar Tree** (`crates/app/src/components/sidebar/mod.rs`):
   - `SidebarTree` renders all 400+ rows (~4,000+ GPUI elements, hitboxes, drag/drop handlers, and context menu closures) inside a non-virtualized `.overflow_y_scrollbar()` container on every frame, even though only ~25–30 rows fit in the viewport.
3. **Uncached Sidebar Model Reconstruction on Every `AppView::render()`**:
   - `SidebarTree` is a `RenderOnce` component constructed inside `AppView::render()`, cloning all folders, requests, pins, parse errors, and git status on every `AppView` frame.
4. **Redundant Per-Keystroke `cx.notify()` in Composer Editors** (`crates/app/src/state/request_session_state.rs`, `persistence_state.rs`, `app/subscriptions.rs`):
   - `RequestSessionState::mark_dirty` unconditionally calls `cx.notify()` on every keystroke even when `is_dirty` is already `true`, and `schedule_save_active_request` is invoked twice per keystroke.

---

## 2. Architectural Decisions

1. **Single-Pass $O(F + R + G)$ Git Status & Tree Indexing**:
   - Introduce `GitStatusIndex` in `git_status.rs` to normalize git file paths once in $O(G)$ into a `HashMap<String, SidebarGitStatus>`, making per-request status lookup $O(1)$ with zero allocations.
   - Compute folder git statuses and search match flags in a single bottom-up pass over an adjacency list of folders and requests in $O(F + R)$.
2. **Flattened Sidebar Model (`FlatSidebarRow`) & Versioned Cache in `SidebarState`**:
   - Track version counters across dependencies:
     - `WorkspaceDataState::tree_version`
     - `GitState::status_version`
     - `PersistenceState::unsaved_version`
     - `SidebarState::ui_version`
   - Cache `Arc<Vec<FlatSidebarRow>>` in `SidebarState` keyed by `(tree_version, status_version, unsaved_version, ui_version)`. When `AppView` re-renders for unrelated reasons, retrieving the flattened sidebar rows is $O(1)$.
3. **List Virtualization via `gpui::uniform_list`**:
   - Store `UniformListScrollHandle` in `SidebarState`.
   - Render visible rows using `gpui::uniform_list` + `.track_scroll(&scroll_handle)` + `.vertical_scrollbar(&scroll_handle)`.
   - Preserve all existing interactions: drag-and-drop reordering/moving (pinned, requests, folders, root dropzone), inline renaming (`Input` + `DismissModal` on Escape), context menus, status badges, and Git status colors.
4. **Eliminate Redundant Per-Keystroke Notifications & Optimize Command Palette**:
   - Only emit `cx.notify()` in `RequestSessionState::mark_dirty` when transitioning `is_dirty` from `false` to `true`.
   - Only emit `cx.notify()` and increment `unsaved_version` in `PersistenceState` when `unsaved_requests` membership actually changes.
   - Optimize `CommandPalette` sorting with a `HashMap` lookup for recent requests and cap displayed request rows to keep `Cmd+K` instantaneous.

---

## 3. Step-by-Step Implementation Tasks

1. **State Version Tracking & Notification Hygiene**:
   - Add `tree_version` to `WorkspaceDataState` and bump on tree/request metadata mutations (`workspace_data_state.rs`, `workspace_crud.rs`, `persistence_state.rs`, `runner.rs`, `subscriptions.rs`).
   - Add `status_version` to `GitState` (`git_state.rs`).
   - Add `unsaved_version` to `PersistenceState` and deduplicate `mark_dirty` / `schedule_save_active_request` notifications (`persistence_state.rs`, `request_session_state.rs`, `subscriptions.rs`).
2. **$O(F + R + G)$ Git Status Index & Flat Tree Builder**:
   - Refactor `crates/app/src/components/sidebar/git_status.rs` to provide `GitStatusIndex` and bottom-up folder status computation.
   - Create flat row model (`FlatSidebarRow`) and single-pass tree flattener with search filtering and caching in `SidebarState`.
3. **Virtualized Sidebar Rendering**:
   - Update `crates/app/src/components/sidebar/{mod.rs, folder_node.rs, request_node.rs, example_node.rs}` to render uniform-height rows via `gpui::uniform_list`.
4. **Command Palette Optimization**:
   - Optimize `crates/app/src/components/command_palette.rs` request sorting and cap rendered items.
5. **Unit & Performance Tests**:
   - Add unit tests for `GitStatusIndex`, bottom-up folder status, flat tree generation, and a 500-request performance regression test.

---

## 4. Verification Steps

```bash
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
