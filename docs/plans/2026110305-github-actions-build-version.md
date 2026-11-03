# Plan: GitHub Actions Build Version Integration (2026110305-github-actions-build-version)

## 1. Goal
Update the version resolution in `zest` so that the About popup, Settings About tab, and update checker consume the version tag and commit SHA generated during the GitHub Actions release build rather than relying solely on static `Cargo.toml`.

## 2. Context & User Request
- User request: `"phần about sửa lại để sử dụng thông tin version từ github action build. hiện tại có vẻ đang lấy từ cargo.toml"`
- Currently, `version` is hardcoded to `env!("CARGO_PKG_VERSION")`.
- In `.github/workflows/release.yml`, releases are triggered on tag pushes (`v*`), where GitHub provides `github.ref_name` (e.g. `v0.2.0`) and `github.sha`.
- We need the build process to inject the release version and commit hash at compile time, with clean fallback for local development.

## 3. Architecture & Design Decisions
1. **Build Script Resolution (`crates/app/build.rs`)**:
   - Check `ZEST_APP_VERSION` environment variable first.
   - Fallback to `GITHUB_REF_NAME` (automatically provided by GitHub Actions runner).
   - Fallback to `git describe --tags --exact-match` or `git describe --tags` (for local git repo tags).
   - Fallback to `CARGO_PKG_VERSION` from `Cargo.toml`.
   - Strip leading `v`/`V` so semver parsing remains canonical.
   - Resolve short git commit hash from `GITHUB_SHA` or `git rev-parse --short HEAD`.
   - Emit `cargo:rustc-env=ZEST_APP_VERSION=...` and `cargo:rustc-env=ZEST_COMMIT_HASH=...`.
   - Mark environment variables with `cargo:rerun-if-env-changed` to ensure accurate rebuilds when tags or hashes change.
2. **GitHub Actions Workflow (`.github/workflows/release.yml`)**:
   - Explicitly supply `ZEST_APP_VERSION: ${{ github.ref_name }}` and `GITHUB_SHA: ${{ github.sha }}` in the build step environment.
3. **App Integration**:
   - Introduce `crates/app/src/version.rs` (or centralize constants in app module) defining:
     - `APP_VERSION`: `env!("ZEST_APP_VERSION")`
     - `COMMIT_HASH`: `env!("ZEST_COMMIT_HASH")`
   - Update `about_modal.rs`, `about_tab.rs`, and `modal_state.rs` (`check_for_updates`) to use `APP_VERSION`.
   - Include Git Commit in the metadata table for enhanced diagnostic visibility.

## 4. Step-by-Step Implementation Tasks
1. Update `crates/app/build.rs` to compute and emit `ZEST_APP_VERSION` and `ZEST_COMMIT_HASH`.
2. Create `crates/app/src/version.rs` and export `APP_VERSION` and `COMMIT_HASH`.
3. Update `crates/app/src/components/about_modal.rs` to use `APP_VERSION` and display `COMMIT_HASH`.
4. Update `crates/app/src/components/settings_modal/about_tab.rs` to use `APP_VERSION` and display `COMMIT_HASH`.
5. Update `crates/app/src/state/modal_state.rs` to pass `APP_VERSION` into `check_github_release`.
6. Update `.github/workflows/release.yml` with explicit build environment variables.
7. Verify with `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check`.
