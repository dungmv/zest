---
name: local-first-storage-vcs
description: >-
  Use this skill when developing, refactoring, or testing the local-first YAML filesystem storage,
  file watcher events, secret splitting, conflict handling, and Git VCS operations in core and app.
---

# Local-First Storage & Git VCS Guide

This skill governs the local-first filesystem storage layer (`zest-core::storage`), disk persistence state (`PersistenceState`), and Git version control operations (`zest-core::git`, dispatched from `crates/app/src/services/git_service.rs` into `crates/app/src/state/git_state.rs`).

---

## 1. Local-First Directory Hierarchy

Zest stores workspaces purely as human-readable, Git-friendly YAML files on disk:

```
my-workspace/
├── .zest/
│   ├── local_state.yaml         # Local UI preferences (active environment, split ratio)
│   └── cache/                   # *.loc binary response cache files (LRU managed)
├── .gitignore                   # Automatically ignores .zest/ and *.secrets.local.yaml
├── workspace.yaml               # Workspace metadata, default headers, workspace auth
├── environments/
│   ├── dev.yaml                 # Public environment variables (checked into Git)
│   └── dev.secrets.local.yaml   # Private secret variables (strictly gitignored)
├── folders/
│   └── users/
│       └── folder.yaml          # Folder metadata, inherited auth & headers
└── requests/
    ├── get-user-profile.yaml    # Request definitions (URL, method, headers, params, body)
    └── login.yaml
```

---

## 2. Strict Schema Validation (SL-005)

All 9 storage model structs in `crates/core/src/storage/models.rs` and `crates/core/src/models.rs` must enforce:
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)] // ⚠ MANDATORY: Rejects unexpected fields at parse time
pub struct RequestFile { ... }
```
- **No Dynamic Catch-all**: Never introduce `extra: BTreeMap<String, Value>`. If new fields are added to the spec, add typed optional fields (`Option<T>`) with `#[serde(default, skip_serializing_if = "Option::is_none")]`.

---

## 3. Secret Splitting Architecture

Variables marked with `is_secret: true` are stored separately:
- **Public values**: Stored in `environments/<slug>.yaml`.
- **Secret values**: Stored in `environments/<slug>.secrets.local.yaml`.
- When loading an environment, `FsWorkspaceStore::load_environment()` reads both files and merges the secret values seamlessly in-memory.

---

## 4. File Watcher & Suppression Loop Prevention

External disk changes are monitored by `WorkspaceWatcher` emitting `FsEvent`.
- **Suppression Mechanism**: Whenever the application writes a file to disk, it registers the path in `suppressed_paths` (`FsWorkspaceStore::suppress_path`).
- When the watcher emits an event, it checks `suppressed_paths`. If matched, the event is silently ignored, preventing infinite render/reload cascade loops (CC-001, CC-002).
- **Conflict Handling**: If a file was modified externally while the user has unsaved in-memory changes, `PersistenceState` marks the item as conflicted (`conflicts` map) rather than silently overwriting it.

---

## 5. Non-Blocking Git VCS Operations (TD-003, CC-004)

All Git operations (`commit`, `pull`, `push`, `fetch`, `checkout`, `discard_file`, `create_branch`, `init_repo`) must be asynchronous:

```rust
// ✅ MANDATORY PATTERN: Dispatch Git CLI to background thread pool
pub fn commit(&mut self, message: String, cx: &mut Context<Self>) {
    self.loading = true;
    cx.notify();

    let git_dir = self.repo_path.clone();
    cx.background_executor().spawn(async move {
        let result = zest_core::git::git_commit(&git_dir, &message);
        // Dispatch result back to UI thread
    }).detach();
}
```

- **UI Status Indicators**:
  - `is_syncing = true`: Displays spinning indicator in status bar during network sync (`pull`, `push`, `fetch`).
  - `loading = true`: Disables double-clicks on modal action buttons.

---

## 6. Verification & Testing

```bash
# 1. Run storage roundtrip and lifecycle tests
cargo test --package zest_core --lib storage::tests

# 2. Run Git operations tests
cargo test --package zest_core --lib git::tests

# 3. Verify clippy compliance
cargo clippy --package zest_core --lib storage git -- -D warnings
```
