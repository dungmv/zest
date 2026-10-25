# Duplicate Name/File Collision Fix — Rename & Drag-Drop Move

- **Date**: 2026-10-25
- **Status**: Implemented (policy revised same day, see bottom)
- **Scope**: `zest-core::storage` (slug/fs_ops/fs_store), `zest` app state layer (workspace_crud, persistence_state, subscriptions), sidebar drag-drop wiring

## Goal

Eliminate the "two requests share the same name" state produced by renaming or
drag-dropping requests/folders, and make folder drag-drop actually persist to
disk. ~~When a target name collides with an existing sibling, the operation is
**blocked with an error toast** (policy confirmed by the user) instead of
silently suffixing the on-disk file (`login-2.yaml`) while the UI shows two
identical display names.~~ *(Superseded — see "Policy Revision" below.)*

## Context — Root Causes

1. **Rename collision (requests)**: `fs_ops::rename_request` resolved a taken
   slug via `unique_slug` (`-2`/`-3` suffix) with zero user feedback; the UI
   displayed two identically named siblings. Worse, renaming `login-2.yaml` to
   "Login" while `login.yaml` existed produced `login-3.yaml` (the item's own
   stem counts as occupied during the probe).
2. **Cross-folder move**: `fs_ops::move_request` silently suffixed the stem in
   the destination and left a stale stem in the source `_folder.yaml` order
   list. Dropping a request onto its **own** folder renamed it to `login-2.yaml`
   (`unique_slug` in the same directory finds the file itself).
3. **Folder drag-drop never persisted**: `WorkspaceDataState::move_folder` was
   an in-memory Vec shuffle — no `parent_id` update, no `store.move_group`
   call. The move reverted on the next watcher `reload_tree()`.
4. **Id changes not propagated**: only request rename had a propagation
   mechanism (`last_renamed_request`). Moving requests, renaming folders, and
   (the new) folder moves left `RequestSessionState.active_request_id`,
   `ResponseState` cache keys, and `PersistenceState` dirty-map/persisted
   selection pointing at dead ids.
5. **Rename race**: `fs_ops::rename_request` copies the on-disk yaml; edits
   still waiting in the 500 ms save debounce were lost on rename.
6. **Related, same class**: `fs_store::rename_environment` used a bare
   `fs::rename` that silently **overwrote** an existing sibling environment
   (real data loss). `unique_slug` also ignored `.yml` twins which the scanner
   loads, allowing same-stem collapse on the next scan.

## Architectural Decisions

- **Policy at the app layer, mechanics in core**: `zest-core` stays permissive
  (suffixing remains as a last-resort defense), while `WorkspaceDataState`
  enforces the "no duplicate slug within a folder" invariant and reports
  rejections through a new `last_crud_error` field consumed by the workspace
  observer subscription and surfaced as an error toast (mirrors the
  `last_renamed_request` pattern; `SidebarRenderContext` has no toast entity,
  so state → subscription is the only channel).
- **Guards compare slugs, not display names**: file identity is the slugified
  stem, so "Login" vs "login" is a collision. Grouping uses the sidebar's
  *effective folder* (a dangling `folder_id` renders at root) so guards and
  rendering can never disagree.
- **Id propagation generalizes** via `last_moved_items: Vec<(old, new)>`
  recorded by folder rename/move and request moves, consumed in
  `subscriptions.rs` to remap response-cache keys, the session pointer, and
  `PersistenceState` (`remap_request_ids`). Folder rename now also remaps
  `parse_errors` (previously missed).
- **Flush-before-rename/move**: in-memory item state is written to the old path
  before any id-changing store call, closing the 500 ms debounce race.
- **Folder move semantics**: drop onto the current parent = cosmetic reorder
  (legacy behavior, folder sibling order remains in-memory only); drop into a
  descendant or itself = rejected; duplicate dir name in the destination =
  rejected; otherwise `store.move_group` persists and
  `repath_folder_descendants` remaps the subtree. Root dropzone now accepts
  folders (`move_folder(src, None)`).

## Step-by-Step Implementation

1. `slug.rs` — `unique_slug` treats `.yml` twins as occupied (`stem_taken`).
2. `fs_ops.rs` — shared `update_folder_order` helper; `rename_request` order
   rewrite refactored onto it; `move_request` drops the stem from the source
   order and appends it to the destination order; `move_group` dest-dedupe
   loop self-excludes (same-parent move keeps the directory name).
3. `fs_store.rs` — `rename_environment` bails when the target env yaml or
   secrets file already exists; `environment_state.rs` adds an in-memory
   duplicate guard.
4. `workspace_data_state.rs` — new `last_moved_items` / `last_crud_error`.
5. `workspace_crud.rs` — helpers (`effective_folder_id`, `request_stem`,
   `folder_dir_name`, `request_slug_taken`, `folder_dir_taken`,
   `folder_is_in_subtree`, `flush_request_to_disk`,
   `repath_folder_descendants`, `move_request_across_folders`); rename guards
   for requests and folders; `move_folder_internal` rewrite; cross-folder
   request moves funneled through the guarded shared helper (own-folder drop
   is a no-op).
6. `persistence_state.rs` — `remap_request_ids` (dirty map + persisted
   active-request selection).
7. `subscriptions.rs` — workspace observer consumes `last_moved_items`
   (response cache, session pointer, persistence) and `last_crud_error`
   (error toast).
8. `folder_node.rs` / `sidebar/mod.rs` — drop handlers pass the new
   `Option<String>` target; root dropzone accepts `DragFolderItem`.

## Risk Mitigation

- Watcher echo after directory moves (`TreeChanged` → `reload_tree`) is
  benign: disk is already correct, reload rebuilds identical state — same as
  the pre-existing folder rename behavior; group ops keep working without
  per-path suppression.
- Windows `fs::rename` of a directory onto an existing directory fails; the
  dedupe loop plus the app-level duplicate guard prevent ever reaching it.
- Pending debounced save tasks keyed by old ids simply no-op (content already
  flushed); their cleanup path already removes stale dirty flags.
- The blocking fs-store calls on the UI thread are the pre-existing pattern;
  not widened by this change.

## Verification

- New core tests (`storage::tests`): `unique_slug` skips `.yml` twins;
  `move_request` updates both order lists; `move_group` same-parent keeps the
  name and cross-parent clash suffixes; environment rename collision is
  rejected with both files intact.
- New app tests (`state::workspace_collision_tests`, 7 tests): duplicate
  request/folder rename blocked with `last_crud_error`; move onto duplicate
  stem blocked; own-folder move is a no-op (no `login-2.yaml`); folder move
  persists to disk, remaps descendant ids/parents/pins, survives reload;
  move-into-descendant rejected; flush-before-rename preserves pending edits.
- Full pipeline: `cargo fmt --check`, `cargo test` (155 passed),
  `cargo clippy --all-targets -- -D warnings` (0 warnings).

## Policy Revision (same day, user decision)

After trying the blocking policy, the user revised it: **duplicate display
names are accepted; the slug suffixes automatically** (`Login` + `Login` →
`login.yaml` + `login-2.yaml`, both display "Login"). Rationale: blocking
made day-to-day use harder than the ambiguity of duplicate names.

Changes applied on top of the original implementation:

- Removed the app-layer duplicate guards from `rename_request_internal`,
  `rename_folder_internal`, `move_request_across_folders`, and
  `move_folder_internal` (helpers `request_slug_taken`, `folder_dir_taken`,
  `request_stem`, `folder_dir_name` deleted). The store's suffixing is now the
  intended behavior, not a fallback.
- **Self-exclusion fix in `fs_ops::rename_request`**: the item's own stem no
  longer counts as occupied during the probe (`unique_slug_excluding`), so
  renaming `login-2.yaml` to "Login" while a sibling `login.yaml` exists keeps
  `login-2` (in-place display-name change) instead of jumping to `login-3`.
  The old `base_slug == old_stem` special case was replaced by this.
- **`move_request` collision semantics**: a cross-directory move must respect
  stems already present in the destination (plain `unique_slug`); only a
  same-directory move keeps its stem (never suffixes itself). Excluding the
  own stem in the *destination* probe would overwrite the sibling file —
  caught by the test suite.
- Kept unchanged: cycle guard ("Cannot move a folder into itself"), own-folder
  drop as no-op, `last_crud_error` toast channel (still reports genuine
  failures), flush-before-rename/move, `last_moved_items` propagation,
  environment rename overwrite protection, folder move persistence and id
  remapping, `.yml` twin awareness.
- Tests updated: duplicate renames/moves now assert suffixed files with
  unchanged display names, self-exclusion, and the own-folder no-op; core adds
  `test_rename_request_self_exclusion_keeps_own_suffix`. Full pipeline:
  `cargo fmt --check`, `cargo test` (156 passed),
  `cargo clippy --all-targets -- -D warnings` (0 warnings).
