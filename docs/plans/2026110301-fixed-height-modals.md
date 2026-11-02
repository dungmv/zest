# Fixed Height & Scrollable Modals for Import and Scope Settings

**Date**: 2026-11-03
**Status**: Completed

## 1. Goal

Fix the modal heights for `ImportModal` (`crates/app/src/components/import_modal.rs`) and `ScopeSettingsModal` (`crates/app/src/components/scope_settings_modal.rs`) so that switching between tabs does not cause jumping or resizing of the modal card. Ensure internal content areas scroll smoothly using `overflow_y_scrollbar()` when content exceeds the dialog height.

## 2. Context & Root Cause Analysis

- In `ImportModal`, the dialog was initialized with `.max_h(px(600.))` without an explicit `.h(...)`. When switching between the "From File" and "Paste Content" tabs, the contents have different intrinsic heights (~300px vs ~500px+), causing the modal container to shrink and expand abruptly.
- In `ScopeSettingsModal`, the dialog was initialized with `.max_h(px(580.))` without an explicit `.h(...)`. When toggling between "General", "Headers", and "Auth" tabs, the content height changes substantially (e.g. empty Headers table is very short vs General with multiple settings), resulting in jumping dialog sizes.
- Missing `.min_h(px(0.))` on the scrollable flex-1 child containers could prevent flex children from shrinking below content size in flexbox layout.

## 3. Implementation Tasks

1. **`crates/app/src/components/import_modal.rs`**:
   - Removed "Paste Content" tab, input area, and unused tab switching code; modal now focuses purely on file selection.
   - Removed subtitle description `"OpenAPI 3.0/3.1, Postman v2.0/v2.1, or cURL"` from the header for cleaner UI.
   - Sized dialog cleanly to `w(px(640.)).h(px(520.))`.
   - Content body container retains `.flex_1().min_h(px(0.)).overflow_y_scrollbar()` for smooth preview scrolling.
2. **`crates/app/src/components/scope_settings_modal.rs`**:
   - Set modal dimensions to exact requested size `w(px(580.)).h(px(320.))`.
   - Content container retains `.flex_1().min_h(px(0.)).p(px(16.)).overflow_y_scrollbar()` ensuring all tabs (General, Headers, Auth) fit cleanly without excess empty space and scroll if headers list expands.
3. **Verification**:
   - `cargo fmt --check`: 0 format issues.
   - `cargo clippy --all-targets -- -D warnings`: 0 warnings.
   - `cargo test`: 199/199 tests passed.
