# Plan: Display App Version in About Popup and Settings (2026110303-about-modal-version-info)

## 1. Goal
Provide a dedicated About popup modal and an About tab within the Settings modal to display application information, notably the app version (`CARGO_PKG_VERSION`), target architecture, platform OS, UI engine, and repository/license details, along with a quick copy-to-clipboard action.

## 2. Context & User Request
- User request: `"trong popup about để hiển thị thông tin version app"` ("in popup about, display the app version info").
- Currently, Zest (`crates/app/Cargo.toml` v0.1.0) has no dedicated About popup dialog or About tab in Settings.
- Users need an intuitive, macOS-native About popup dialog displaying the app version, icon, and build information, accessible from the titlebar app menu, shortcuts/actions, and Settings.

## 3. Architecture & Design Decisions
1. **Modal State Integration (`crates/app/src/state/modal_state.rs`)**:
   - Add `show_about_modal: bool` to `ModalState`.
   - Add `open_about_modal()`, `close_about_modal()`, and `toggle_about_modal()`.
   - Update `dismiss_top_modal()` to dismiss `show_about_modal` when active.
   - Add `SettingsTab::About` to allow viewing app info directly inside the Settings modal.
2. **Dedicated About Popup Dialog (`crates/app/src/components/about_modal.rs`)**:
   - Use `ModalDialog::new("about-modal")` with ~440px width.
   - Display:
     - App icon (retina `app_icon_128.png`).
     - App title: **Zest**.
     - Version chip: **v0.1.0** (`env!("CARGO_PKG_VERSION")`) with copy button and clipboard toast feedback.
     - Tagline: "High-performance macOS-native REST & API client".
     - System information table (Version, OS, Architecture, UI Framework, Storage engine).
     - License note and Close button.
3. **Settings Modal About Tab (`crates/app/src/components/settings_modal/about_tab.rs`)**:
   - Add `SettingsTab::About` to `SettingsModal` navigation.
   - Render application version details within the settings interface.
4. **App Overlay Attachment (`crates/app/src/app/modals.rs`)**:
   - Conditionally render `AboutModal` when `modal_state.read(cx).show_about_modal` is true.
5. **Entrypoints**:
   - Titlebar workspace dropdown menu: Add `"About Zest"` item with `IconName::Info`.
   - Global action: `ToggleAboutModal`.

## 4. Step-by-Step Implementation Tasks
1. Update `crates/app/src/state/modal_state.rs` with `show_about_modal` and `SettingsTab::About`.
2. Implement `crates/app/src/components/about_modal.rs` with clean GPUI tokens and responsive layout.
3. Implement `crates/app/src/components/settings_modal/about_tab.rs` and register `SettingsTab::About` in `settings_modal/mod.rs`.
4. Export `AboutModal` in `crates/app/src/components/mod.rs`.
5. Attach `AboutModal` in `crates/app/src/app/modals.rs`.
6. Add `ToggleAboutModal` action in `crates/app/src/app/actions.rs` and wire in `crates/app/src/app/mod.rs`.
7. Add "About Zest" item in `crates/app/src/components/titlebar.rs` workspace switcher menu.
8. Verify with `cargo test` and `cargo clippy --all-targets -- -D warnings`.

## 5. Verification Steps
- `cargo check`
- `cargo test`
- `cargo clippy --all-targets -- -D warnings`
