# Plan: Sort Environments Alphabetically and Modal Active Indicator Cleanup

- **Date**: 2026-10-28
- **Sequence**: 2026102805
- **Goal**:
  1. Sort environments alphabetically (case-insensitive) in the environment modal and environment list dropdown.
  2. In the environment modal, remove the circle status dot (`●`/`○`) before each environment name in the sidebar.
  3. In the environment modal detail header, remove the `● Active` badge from the title.
  4. Ensure the environment modal does not concern itself with which environment is currently active for request execution.

---

## 1. Context & Rationale

- Previously, environments were listed in the order loaded from filesystem or created.
- The environment modal displayed a green dot `●` or hollow circle `○` next to each environment item in the left sidebar indicating whether it was the workspace's currently active execution environment.
- The modal header also displayed a `● Active` badge next to the environment name if it matched the active environment.
- The environment modal is intended for configuring and editing environment variables across all environments, regardless of which one is currently active for sending requests.
- Users requested:
  - Alphabetical ordering in both the modal sidebar and the environment dropdown in the titlebar.
  - Removing the leading status dot in the modal sidebar items.
  - Removing the active status badge in the modal title/header.
  - Removing active environment tracking/concern from the modal.

---

## 2. Implementation Tasks

1. **`crates/app/src/state/environment_state.rs`**:
   - In `reload_environments()`, sort `envs` alphabetically by `name.to_lowercase()` (`envs.sort_by_key(|a| a.name.to_lowercase())`).
   - By sorting once at the state level in `EnvironmentState`, `self.environments` remains sorted everywhere across the application without requiring redundant re-sorting in `render()`.
   - Add a unit test to verify alphabetical sorting upon reload.

2. **`crates/app/src/components/titlebar.rs`**:
   - In the environment pill dropdown menu (`btn-env-breadcrumb`), read pre-sorted `envs` from state.

3. **`crates/app/src/components/env_modal.rs`**:
   - Use pre-sorted `filtered_envs` directly from state.
   - Remove `active_env_id` and `is_target_active` variables.
   - Rename `active_custom_env` to `selected_custom_env` to avoid semantic confusion.
   - Remove the leading circle dot (`●` / `○`) from sidebar environment rows.
   - Remove the `● Active` badge from the detail header next to the environment title.

---

## 3. Verification

1. Run `cargo test` across all crates to ensure existing tests and new sorting tests pass.
2. Run `cargo clippy --all-targets -- -D warnings` to verify zero warnings.
