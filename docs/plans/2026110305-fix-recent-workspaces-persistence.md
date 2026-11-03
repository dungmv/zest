# Plan: Fix Recent Workspaces Persistence and Isolation

## 1. Goal
Ensure that workspaces opened by the user are properly persisted and retained in the "Recent Workspaces" list across app restarts, eliminate destructive test-suite pollution of real user `settings.json`, and normalize workspace directory paths to prevent duplicate or missing recent entries.

## 2. Root Cause Analysis
1. **Test-Suite Pollution of Real User Config**:
   - `AppSettings::config_file_path()` always pointed directly to `~/Library/Application Support/zest/settings.json` even in unit tests.
   - `test_workspace_rename_and_history` in `workspace_data_tests.rs` invoked `ws_state.clean_history_internal()`, executing `settings.recent_workspaces.clear(); settings.save();` against the user's real configuration file.
   - Every execution of `cargo test` (required before committing by Rule 6) wiped all recent workspaces from the user's active installation.
   - `test_modal_state_theme_preference_persistence` modified the user's actual theme preference on disk.
2. **Path Non-Canonicalization & Formatting Inconsistency**:
   - `open_workspace` and `WorkspaceDataState::new()` did not canonicalize paths before storing them in `recent_workspaces`. Symlinks, relative paths, or trailing slashes caused identity comparisons (`r_path == ws_path` or `retain(|p| p != path)`) to fail or create fragmented entries.
3. **Ambiguous UI Action Labeling**:
   - In the titlebar workspace switcher dropdown, the action was labeled "Clean History" without explanation or confirmation, which can mislead users into thinking it cleans request logs or curl history rather than wiping workspace recents.

## 3. Implementation Tasks
1. **Isolate `AppSettings` During Testing**:
   - Update `AppSettings::config_file_path()`:
     - Check `ZEST_CONFIG_DIR` environment variable first if set.
     - Under `#[cfg(test)]`, route to a thread/process-safe temporary directory path (`std::env::temp_dir().join(format!("zest_test_settings_{}.json", std::process::id()))`).
     - Under `#[cfg(not(test))]`, use `dirs::config_dir().join("zest/settings.json")`.
2. **Normalize Workspace Paths**:
   - Add path normalization in `AppSettings::add_recent_workspace` so canonical disk paths are recorded consistently.
   - In `WorkspaceDataState::open_workspace` and `WorkspaceDataState::new()`, resolve canonicalized path (`path.canonicalize().unwrap_or(path)`) before passing to `add_recent_workspace`.
3. **Update UI Label & Dropdown Formatting**:
   - In `titlebar.rs`, rename "Clean History" to "Clear Recents" to accurately describe the action.
4. **Restore User's Real Config**:
   - Restore existing known workspaces (`/Users/dungmv/Documents/autotool/AFK-Mode-Web/api`, `/Users/dungmv/Documents/api`, `/Users/dungmv/Documents/emddi/apidocs`, `/Users/dungmv/Documents/zest`) into the user's `settings.json` with theme preference restored to `"light"`.
5. **Add Comprehensive Unit Tests**:
   - Add unit tests verifying `add_recent_workspace` maintains ordering, limits to 10 entries, normalizes paths, and prevents duplicates.
   - Verify `clean_history_internal` operates on isolated test configuration without touching user config.

## 4. Verification Steps
- Run `cargo test` across all targets.
- Verify `~/Library/Application Support/zest/settings.json` is completely untouched by `cargo test`.
- Run `cargo clippy --all-targets -- -D warnings`.
