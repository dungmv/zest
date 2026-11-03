# Plan: GitHub Release Update Checker (2026110304-github-release-update-checker)

## 1. Goal
Integrate an in-app update checker that queries the GitHub Releases API (`dungmv/zest`) to compare the running version (`CARGO_PKG_VERSION`) against the latest release, displaying interactive status (Checking, Up to date, Update Available, Error) and a one-click link to download or view release notes.

## 2. Context & User Request
- User request: `"có thể kiểm tra update thông qua link github release không?"` -> `"vậy hãy tích hợp vào đi"`
- Users want to check for new versions directly from within the app (in the About popup and Settings About tab) via GitHub Releases.

## 3. Architecture & Design Decisions
1. **Domain Engine (`crates/core/src/updater.rs`)**:
   - `GithubRelease` deserializer for GitHub REST API (`/repos/{owner}/{repo}/releases/latest`).
   - SemVer comparison engine `parse_semver` and `is_newer_version(current, latest)`.
   - `check_github_release(owner, repo, current)` async query with 10s timeout, custom User-Agent, and graceful 404 handling (no releases published yet treated as up-to-date).
   - Comprehensive unit tests in `zest_core`.
2. **State Management (`crates/app/src/state/modal_state.rs`)**:
   - `UpdateStatus` enum (`Idle`, `Checking`, `UpToDate`, `UpdateAvailable`, `Error`).
   - `check_for_updates(&mut self, cx: &mut Context<Self>)` that spawns the async query on the background executor and mutates state on the main UI thread.
   - `open_browser_url(url)` cross-platform helper (`open` on macOS, `rundll32` on Windows, `xdg-open` on Linux).
3. **UI Integration**:
   - **About Popup (`crates/app/src/components/about_modal.rs`)**: Interactive update card with state-specific buttons (Check for Updates, Checking spinner, UpToDate checkmark, UpdateAvailable banner with "View Release on GitHub" button).
   - **Settings About Tab (`crates/app/src/components/settings_modal/about_tab.rs`)**: Matching update checking interface within the Preferences window.
4. **Tokio Runtime Reactor Safety**:
   - GPUI `cx.spawn` threads run on GPUI's executor without a Tokio reactor. `reqwest` requires a running Tokio reactor to perform async network calls.
   - Centralized `global_tokio_runtime()` in `crates/app/src/services/runtime.rs`.
   - `check_for_updates` dispatches the network future directly to `global_tokio_runtime()`, communicating back to GPUI's `cx.spawn` via `tokio::sync::oneshot`.

## 4. Step-by-Step Implementation Tasks
1. Create `crates/core/src/updater.rs` with domain logic, HTTP client, SemVer comparison, and unit tests.
2. Export `pub mod updater;` in `crates/core/src/lib.rs`.
3. Add `UpdateStatus` and `check_for_updates` in `crates/app/src/state/modal_state.rs`.
4. Update `crates/app/src/components/about_modal.rs` with update checker UI controls.
5. Update `crates/app/src/components/settings_modal/about_tab.rs` with update checker UI controls.
6. Verify with `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check`.

## 5. Verification Checklist
- [ ] Unit tests for `parse_semver` and `is_newer_version` pass.
- [ ] Compiles cleanly with 0 clippy warnings.
- [ ] Formatting satisfies `cargo fmt`.
