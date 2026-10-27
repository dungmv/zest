---
name: local-first-storage-vcs
description: >-
  Use this skill when developing, refactoring, or testing the local-first YAML filesystem storage,
  write-behind persistence coordinator, file watcher events, secret splitting, conflict handling,
  and Git VCS operations in core and app.
---

# Local-First Storage, Write-Behind Persistence & Git VCS Guide

This skill governs the local-first filesystem storage layer (`zest-core::storage`), unified write-behind persistence (`PersistenceState` + `PersistenceCoordinator`), and asynchronous Git version control operations (`zest-core::git`, `GitService`, `GitState`).

---

## 1. Local-First Directory Hierarchy

Zest stores workspaces as human-readable, Git-friendly YAML files on disk (`FsWorkspaceStore` in `crates/core/src/storage/fs_store.rs`):

```
my-workspace/
├── .local/                        # Strictly gitignored local state & secrets
│   ├── state.yaml                 # Local UI state (active_request, active_environment)
│   ├── pins.yaml                  # Pinned request relative paths
│   ├── cookies.json               # Persisted HTTP cookie jar
│   └── secrets/
│       ├── local.yaml             # Private secret values for "local" environment
│       └── globals.yaml           # Private secret values for global variables
├── .gitignore                     # Automatically ensures .local/ and *.loc are ignored
├── workspace.yaml                 # Workspace metadata, default_environment, workspace headers & auth
├── environments/
│   ├── local.yaml                 # Committed environment variables (secret values stripped to None)
│   └── globals.yaml               # Committed global variables (optional, created on save)
└── collections/
    ├── health-check.yaml          # Root-level request definition
    ├── health-check.loc           # Sidecar JSON response cache (gitignored via *.loc)
    └── users/
        ├── _folder.yaml           # Folder metadata, request order, folder headers & auth
        ├── get-profile.yaml       # Request definition (name, method, url, params, headers, auth, body, settings, script, examples)
        └── get-profile.loc        # Sidecar JSON response cache (truncated at 512KB)
```

---

## 2. Strict Schema Validation & Move Semantics (SL-005, PERF-008)

All storage model structs in `crates/core/src/storage/models.rs` and `crates/core/src/models.rs` enforce:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)] // MANDATORY: Rejects unexpected fields at parse time
pub struct RequestFile { ... }
```
- **No Dynamic Catch-All**: Never introduce `extra: BTreeMap<String, Value>`.
- **Zero-Clone Conversions**: Prefer `RequestFile::into_item(self, ...)` and `RequestFile::from_item_owned(item)` when moving between storage and domain layers.

---

## 3. Secret Splitting & Globals Architecture

Variables marked with `secret: true` in `EnvVarFile` / `EnvironmentVariable` are split automatically on save:
- **Public YAML** (`environments/<name>.yaml` and `environments/globals.yaml`): Committed to Git with `secret: true` and `value: None`.
- **Private Secrets** (`.local/secrets/<name>.yaml` and `.local/secrets/globals.yaml`): Gitignored `HashMap<String, String>` storing the sensitive values. Corrupt secret files are backed up to `<name>.yaml.bak` before returning an error.
- When loading (`load_environment` / `load_global_environment`), `FsWorkspaceStore` merges the secret values back into memory seamlessly.

---

## 4. Unified Write-Behind Persistence (`PersistenceCoordinator`)

All UI edits to YAML-backed data flow through `PersistenceCoordinator` (`crates/app/src/state/persistence_coordinator.rs`) owned by `PersistenceState`:
1. **Immediate In-Memory Update**: UI callbacks mutate the canonical entity (`WorkspaceDataState` or `EnvironmentState`) immediately and emit a modification event (`SessionEvent::RequestModified`, `WorkspaceEvent::WorkspaceMetaModified`, `WorkspaceEvent::FolderMetaModified`, `WorkspaceEvent::PinsModified`, `EnvironmentEvent::EnvironmentModified`, `EnvironmentEvent::GlobalsModified`).
2. **Per-Target Coalescing**: Edits are queued by `PersistTarget` (`Request`, `Environment`, `Globals`, `WorkspaceMeta`, `FolderMeta`, `Pins`, `LocalState`). Rapid edits within the debounce window coalesce into a single latest `PendingWrite` snapshot per target.
3. **Ordered Revisions & Workspace Generation**: Monotonic `revision: u64` and `workspace_gen: u64` ensure at most one in-flight writer per target, prevent stale snapshots from overwriting newer edits, and discard completions from a closed workspace.
4. **Barriers & Remapping**: Rename/move operations remap queued targets via `PersistenceState::remap_request_ids`. Delete and Git discard operations cancel pending writes via `PersistenceState::cancel_request` / `cancel_environment`.

---

## 5. File Watcher, Suppression & External Change Reconciliation

External disk changes are monitored by `WorkspaceWatcher` (`crates/core/src/storage/watcher.rs`) emitting `FsEvent`:
- **Single Cleaner Thread (PERF-005)**: `FsWorkspaceStore::suppress_path` registers app-written paths and schedules expiration (1500ms) on the dedicated `workspace-suppress-cleaner` background thread.
- **Defense-in-Depth Atomic Replace Handling**: Tools like `git restore` and external editors replace files via remove + recreate. Both `downgrade_existing_deletions` (in `watcher.rs`) and `PersistenceState::handle_fs_events_batch` check whether a "deleted" request file still exists on disk; if it does, the event is downgraded to a change/tree rescan so live requests never vanish from the sidebar.
- **Selective Git Metadata Watching**: The watcher emits `FsEvent::GitChanged` for `.git/index`, `HEAD`, and `refs/` while ignoring `.lock`, `.git/objects/`, and `.git/logs/`, keeping Git status badges in sync with external CLI actions.
- **Conflict Detection**: If a file changes externally while `PersistenceState::is_dirty(rel_path)` is true, the request is marked in `conflicts` rather than silently overwriting in-memory edits.

---

## 6. Non-Blocking Git VCS via `GitService`

Git state and command execution are split across two layers:
- **`GitState` (`crates/app/src/state/git_state.rs`)**: Pure GPUI entity holding repository status cache, branches, remotes, commit history, diff selection, and `loading` / `is_syncing` guards.
- **`GitService` (`crates/app/src/services/git_service.rs`)**: Stateless command router holding `Entity<GitState>` and `Entity<ToastState>`. Dispatches all blocking CLI calls (`init_repo`, `commit`, `checkout`, `create_branch`, `add_remote`, `remove_remote`, `discard_file`, `fetch`, `pull`, `push`) to `cx.background_executor()` via `run_command` and `run_sync_op`.
- **Subprocess Timeout & Discard Safety**:
  - All `zest_core::git` commands execute with a 30-second timeout (`run_git_cmd_timeout`) and path validation.
  - `GitService::discard_file(path, delete_untracked, ...)` enforces caller intent: sidebar discard passes `delete_untracked = false` (preserving untracked request files) and cancels pending write-behind tasks, while the Git Changes modal passes `delete_untracked = true`.

---

## 7. Verification & Testing

```bash
# 1. Run core storage and watcher unit tests
cargo test --package zest-core --lib storage::

# 2. Run core Git unit tests
cargo test --package zest-core --lib git::

# 3. Run app persistence coordinator and state tests
cargo test --package zest --bin zest state::persistence_tests

# 4. Verify clippy compliance
cargo clippy --workspace --all-targets -- -D warnings
```
