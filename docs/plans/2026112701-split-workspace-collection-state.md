# Plan: Split `WorkspaceDataState` into `WorkspaceState` + `CollectionState` (Option B)

## Goal

Eliminate the `WorkspaceDataState` logical God Object (~1,793 LOC across 4 files, 7 mixed responsibilities) by renaming and splitting it into two decoupled GPUI entities (`WorkspaceState` and `CollectionState`) plus a stateless OpenAPI export helper in `services/`.

## Context

`WorkspaceDataState` currently combines:
1. Workspace lifecycle & metadata (`workspace_path`, `workspace_meta`, `active_workspace`, `recent_workspaces`, `open/new/rename_workspace`).
2. Workspace-scoped HTTP executor & cookie jar (`executor`, `cookies`).
3. Collection tree data & indexing (`folders`, `requests`, `scratch_request`, `pins`, `parse_errors`, `tree_version`, `url_version`, `request_index`).
4. Request & folder CRUD (`workspace_crud.rs`, `workspace_folder_crud.rs`).
5. Background filesystem scan & watcher reload coalescing (`scan`).
6. Hierarchy/inheritance resolution (`inherited_headers_for_request`, `inherited_auth_source`, `resolve_request_hierarchy`).
7. Import/OpenAPI export orchestration.

This forces collection-only UI components (`SidebarTree`, `BodyEditor`, `RequestComposer`, `ResponseInspector`, `RequestBar`) to depend on workspace lifecycle and cookie/executor state.

## Architectural Decisions

1. **Two GPUI Entities in `AppState` (9 entities total):**
   - `WorkspaceState` (`crates/app/src/state/workspace_state.rs`): owns `store`, `workspace_path`, `workspace_meta`, `active_workspace`, `recent_workspaces`, `executor`, `cookies` (7 fields). Emits `WorkspaceEvent` (`WorkspaceOpened`, `WorkspaceMetaModified`).
   - `CollectionState` (`crates/app/src/state/collection/`): owns `store`, `folders`, `requests`, `scratch_request`, `parse_errors`, `pins`, `tree_version`, `url_version`, `workspace_scope` (lightweight `(name, headers, auth)` mirror), and `scan` (10 fields). Emits `CollectionEvent` (`IdsRemapped`, `RequestsReloaded`, `ExampleRenamed`, `CrudFailed`, `FolderMetaModified`, `PinsModified`, `WorkspaceScanLoaded`).
2. **Least-Privilege Component Dependencies:**
   - `SidebarTree` (including `Copy as cURL`), `BodyEditor`, `RequestComposer`, `ResponseInspector`, `RequestBar`, `GitPanel`, `PersistenceState`, and `SidebarState` depend **only** on `Entity<CollectionState>`.
   - `SettingsModal` (cookies), `Titlebar` (workspace switcher / recent workspaces), and `WorkspaceSettingModal` general tab depend on `Entity<WorkspaceState>`.
3. **Stateless Export Helpers:**
   - Move OpenAPI export stats & builder functions to `crates/app/src/services/openapi_export.rs` operating on `&CollectionState` + workspace name.
4. **Synchronization via Typed Event Channel:**
   - `WorkspaceEvent::WorkspaceOpened` updates `CollectionState::set_workspace(...)` and triggers `load_workspace_async`.
   - `WorkspaceEvent::WorkspaceMetaModified` updates `CollectionState::set_workspace_scope(...)` and schedules `WorkspaceMeta` persistence.
   - `CollectionEvent::WorkspaceScanLoaded` hydrates `WorkspaceState` (`workspace_meta`, `active_workspace`, `cookies`) from the single background scan without duplicating disk I/O.

## Step-by-Step Implementation Tasks

1. Create `crates/app/src/services/openapi_export.rs` with stateless OpenAPI export stats & builder functions; wire into `services/mod.rs`.
2. Create `crates/app/src/state/workspace_state.rs` (`WorkspaceState`) and `crates/app/src/state/collection/` (`mod.rs`, `scanner.rs`, `inheritance.rs`, `request_crud.rs`, `folder_crud.rs`).
3. Split `WorkspaceEvent` and `CollectionEvent` in `crates/app/src/state/workspace_events.rs`.
4. Update `AppState`, `PersistenceState`, `watcher_reconciler.rs`, `SidebarState`, `app/subscriptions.rs`, and `app/workspace_event_handler.rs`.
5. Update UI components, modals, `Runner`, and unit tests (`workspace_data_tests.rs`, `workspace_collision_tests.rs`, `workspace_export_tests.rs`, `persistence_tests.rs`) to use `WorkspaceState` or `CollectionState` by least privilege.
6. Remove legacy `workspace_data_state.rs`, `workspace_data_ops.rs`, `workspace_crud.rs`, `workspace_folder_crud.rs` and update `AGENTS.md` / `docs/technical-debt.md` if needed.

## Risk Mitigation

- Keep all `*_internal` test helper methods on `WorkspaceState` and `CollectionState` so existing unit tests migrate cleanly without losing coverage.
- Keep `WorkspaceScan` producing both tree and workspace metadata/cookies in one background pass so startup I/O does not double.
- Enforce `cargo fmt --check`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings`.

## Verification Steps

```bash
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
find crates/ -type f -name "*.rs" -exec wc -l {} + | awk '$1 > 650 && $2 != "total"' | sort -rn
```
