# Plan: Extract GitService from GitState (Runner-style command service)

- **Date**: 2026-10-25
- **Status**: Approved (in implementation)
- **Scope**: `crates/app` — state layer + git components + services

## Goal

Move all Git CLI command orchestration (`commit`, `checkout`, `pull`, `push`, `fetch`,
`init_repo`, `create_branch`, `add_remote`, `remove_remote`, `discard_file`) out of
`GitState` into a new stateless `GitService` in `crates/app/src/services/git_service.rs`,
following the existing `Runner` pattern. `GitState` remains the single GPUI entity for
git data and UI state.

## Context

- `GitState` is currently implemented across two files (`git_state.rs` 591 LOC,
  `git_ops.rs` 398 LOC). Both mix pure state transitions with async CLI orchestration.
- Five command methods thread `Option<Entity<ToastState>>` through every call site
  (e.g. `git.commit(&msg, Some(toast_s.clone()), cx)` in `commit_modal.rs`).
- ~11 near-identical async blocks: guard → set flag → `cx.spawn` → background run →
  clear flag → `match res` → error/toast/refresh cascade.
- Architecture already defines the target pattern: `HttpEngine` (pure engine) +
  `Runner` (entity-holding orchestrator, `services/runner.rs`).

## Architectural Decisions

1. **Runner-style, not engine-style.** Git ops are one-shot commands with no progress
   streaming or mid-flight cancellation, so `HttpEngine`'s event-channel design does not
   apply. `GitService` is a `Clone` struct holding `Entity<GitState>` +
   `Entity<ToastState>` — exactly the two handles git commands need.
2. **Stateless service, constructed at point of use.** No `AppView` field. Only two
   construction sites: `app/modals.rs` (for `GitModal`) and `render_sidebar_footer`
   (from the entities it already receives). Cheap to construct; no lifecycle to manage.
3. **Separation of concerns:**
   - `GitState` keeps: data fields, modal/tab transitions (`open_modal`, `set_tab`, …),
     refreshes (`refresh_status/history/branches/remotes/all`, which contain
     modal-aware UI coherence logic), diff/selection mutations (`select_file`,
     `select_commit`, `toggle_file_selection`, …).
   - `GitService` owns: guard checks, background dispatch, result routing, toasts,
     post-success refresh triggers.
4. **Guards stay in `GitState`.** `loading` / `is_syncing` drive spinner rendering;
     the service checks/sets them through `git.update(...)` (single-threaded UI context,
     no race).
5. **Two private helpers collapse the boilerplate:**
   - `run_command` — `loading`-guarded ops, errors → `GitState::error`.
   - `run_sync_op` — `is_syncing`-guarded ops (fetch/pull/push), errors → state +
     error toast with per-op title.
   - `commit`, `checkout`, `discard_file` stay bespoke (chained push, branch update,
     selection cleanup).
6. **Dead code dropped, not migrated:** `fetch_in_background` and `delete_branch`
   have zero callers (masked by `#![allow(dead_code)]` in `state/mod.rs`).
7. **`git_ops.rs` is dissolved.** Its three remaining selection methods merge into
   `git_state.rs`; `state/mod.rs` drops the module. Result: `git_state.rs` ≈ 470 LOC.
8. **Behavior preserved**, with one deliberate improvement: the `run_command` success
   path now calls `cx.notify()` so loading spinners clear immediately instead of
   waiting for the follow-up refresh to land.
9. **No changes** to `app/mod.rs`, `SidebarTree` constructor, `subscriptions.rs`,
   `command_palette.rs` — those only use state-side methods (`set_repo_path`,
   `open_modal`, `set_tab`, `refresh_status`).

## Step-by-Step Implementation Tasks

1. Create `crates/app/src/services/git_service.rs`:
   `GitService { git, toast }`, `new()`, helpers `run_command` / `run_sync_op`, and
   public commands: `commit`, `checkout`, `create_branch`, `add_remote`,
   `remove_remote`, `init_repo`, `discard_file`, `fetch`, `pull`, `push`.
2. Register module in `services/mod.rs`.
3. Remove migrated methods from `git_state.rs` (`add_remote`, `remove_remote`,
   `delete_branch`, `fetch`) and prune now-unused imports (`ToastState`, `Entity`).
4. Merge `toggle_file_selection`, `toggle_all_files_selection`,
   `set_commit_and_push` from `git_ops.rs` into `git_state.rs`; delete `git_ops.rs`;
   update `state/mod.rs`.
5. `components/git/common.rs`: `render_not_a_repo_view(git_service, is_loading)` —
   init button calls `service.init_repo(cx)`.
6. `components/git/commit_modal.rs`: `GitCommitModal::new(git, ws, service)`;
   `ChangesViewProps.service` replaces `.toast`; commit/discard via service.
7. `components/git/branch_modal.rs`, `remotes_modal.rs`, `history_modal.rs`:
   add service param; commands via service.
8. `components/git/mod.rs`: `GitModal::new(git, ws, service)` passes service down.
9. `app/modals.rs`: construct `GitService::new(git_entity, toast_entity)` for
   `GitModal` (replaces passing raw toast).
10. `components/sidebar/footer.rs`: construct service internally; menu commands
    (`checkout`, `fetch`, `pull`, `push`) via service; toast clones removed.
11. Update `.agents/skills/gpui-architecture/SKILL.md` entity table
    (`GitState` → `state/git_state.rs`; commands via `services/git_service.rs`).

## Risk Mitigation

- **Async API drift**: mirror the exact `cx.spawn(async move |cx| …)` +
  `cx.update(...)` + `cx.background_executor().spawn(…).await` shapes already
  compiling in `runner.rs` / `git_state.rs`.
- **Guard semantics**: service reads guards via `git.read(cx)` before updating —
  same single-threaded sequencing as the current `&mut self` methods.
- **Chained commit+push ordering** preserved: push only after successful commit;
  commit-failure still surfaces only in `GitState::error` (no toast), push-failure
  still toasts "Git Push Failed".
- **Discard + workspace reload**: `ws.reload_all()` stays at the call site
  (commit modal); service keeps git-side cleanup only.

## Verification Steps

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Manual smoke (optional): open Git modal → commit with "and push", create branch,
add/remove remote, sidebar Fetch/Pull/Push, discard a file, "Not a repo" → init.
