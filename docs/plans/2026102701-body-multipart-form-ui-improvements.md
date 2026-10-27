# Implementation Plan: Body Multipart & Form UI/UX Improvements

**Date**: 2026-10-27  
**Artifact ID**: `2026102701-body-multipart-form-ui-improvements`  
**Status**: Completed  

## 1. Goal
Improve the UI/UX of the Body editor tabs (specifically Multipart and Form URL-Encoded), consolidate inputs, elevate action buttons to the top toolbar alongside format buttons, and synchronize the `Content-Type` request header when changing body types.

## 2. Context & Requirements
- **Multipart Value Type**: Change the value type selector from a toggle button to a dropdown selector ("Text" / "File"), defaulting to "Text".
- **Text Value Width**: Make the value input fill the remaining horizontal width (`flex_1`) instead of being truncated/short.
- **File Value Consolidation**: Consolidate the two separate file components (path display box and external "Browse..." button) into a single unified input element that fits the available width.
- **Top Bar Action Buttons**: Move "+ Add Part" (Multipart) and "+ Add Row" (Form URL-Encoded) to the top toolbar on the right side (same position and styling as "Format JSON" / "Format XML"), and remove the leading `+` sign ("Add Part", "Add Row"). Remove the duplicate bottom buttons.
- **Content-Type Header Sync**: When selecting a body type, automatically set or update the `Content-Type` header in the request's headers list accordingly (or remove it when body type is "None").

## 3. Architecture & Design Decisions
- `HttpRequestItem::update_content_type_header(Option<&str>)`: Add a domain helper in `zest_core::models::HttpRequestItem` to update the existing `Content-Type` header (case-insensitive) or insert a new one when `Some(...)`, and retain/remove when `None`.
- `BodyEditor::render`:
  - When switching body types in the dropdown, call `r.update_content_type_header(Some(...))` with appropriate MIME types (`application/json`, `application/xml`, `text/plain`, `multipart/form-data`, `application/x-www-form-urlencoded`, `application/octet-stream`, or `None`).
  - In the top action bar: render "Add Part" when `is_multipart` and "Add Row" when `is_form`, using identical styling as "Format JSON" / "Format XML".
- `KvTable`:
  - Add `show_add_button: bool` option (default `true`).
  - In `render_form_editor`, set `.show_add_button(false)` to hide the bottom button since the action is now in the top toolbar.
- `render_multipart_editor`:
  - Ensure rows have `.w_full()`.
  - Replace the toggle div with a `Button::new(...).dropdown_menu(...)` showing the current selection ("Text" or "File") with ChevronDown icon.
  - For file type: render a single button fitting the full remaining width (`flex_1().min_w(px(0.))` with `w_full()`, identical to text input width behavior), displaying "Choose file..." or only the filename with file icon and full path hover tooltip.
  - Apply `.flex_shrink_0()` on fixed-width columns (Checkbox: 20px, Key: 160px, Type dropdown: 76px, Delete: 24px) and `.min_w(px(0.))` on the Value flex-1 container and inner labels to prevent flexbox from squishing the Key column.
  - Remove the bottom "+ Add Part" button.

## 4. Verification Steps
- Unit test for `update_content_type_header` covering existing replacement, new addition, case-insensitivity, and removal on `None`.
- Run `cargo fmt --check`.
- Run `cargo test`.
- Run `cargo clippy --all-targets -- -D warnings`.
