# Implementation Plan: Command Palette Refinement & Windows Ctrl+K Shortcut Fix

- Date: 2026-10-30
- Plan File: `docs/plans/2026103004-command-palette-refinement-and-shortcut-fix.md`

## 1. Goal
1. Simplify Command Palette to strictly keep only:
   - "SWITCH REQUEST"
   - "SWITCH ENVIRONMENT"
   Remove all extraneous actions (Send, Settings, Cookies, Git, Import, History).
2. Display the full hierarchical path for each request (e.g., `Folder / Subfolder / Request Name`) instead of only the request name.
3. Fix the `Ctrl+K` shortcut on Windows so that Command Palette opens reliably anywhere in the application, including when an input or editor has focus.

## 2. Technical Context & Root Cause Analysis
- **Command Palette Overcrowding**: `crates/app/src/components/command_palette.rs` currently includes 7 hardcoded command items that clutter the list.
- **Request Path Display**: Currently requests only display `req.name`. The folder hierarchy can be traced upward from `req.folder_id` through `ws.folders`.
- **Windows Ctrl+K Failure**: In GPUI on Windows (`gpui-fast-windows`), when `Ctrl` is pressed together with `K`, `ToUnicode` generates ASCII `0x0B` (control character). `events.rs:process_key` detects `c != c_no_modifiers` and sets `prefer_character_input = true`. As a consequence, `window.input_preference` returns `InputPreference::CharacterInput` whenever any text input or editor has focus, causing GPUI to skip `KeyBindings` in `match_result.bindings`. Additionally, `root.on_action` depends on element focus traversal. By intercepting `Ctrl+K` / `Cmd+K` directly in `root.on_key_down` (mirroring how `Escape` was handled), the shortcut will reliably trigger regardless of which input or widget is focused.

## 3. Implementation Steps
1. **Command Palette Cleanup & Full Path**:
   - In `crates/app/src/components/command_palette.rs`:
     - Compute full folder path for each request: traverse `folder_id` -> `parent_id` up the tree, format as `[Folder1, Folder2, ..., RequestName].join(" / ")`.
     - Filter by `full_path.to_lowercase().contains(&q) || r.url.to_lowercase().contains(&q)`.
     - Remove the 7 extraneous action blocks (Send, Settings, Cookies, Git changes, Git checkout, Import, Git history).
     - Display `full_path` for each request row with clear visual separation.
     - Simplify `CommandPalette` struct and constructor to remove unused `git_state` and `on_send`.
     - Add auto-focus to `search_input` on render.
     - Support `Ctrl+K` in CommandPalette's own `on_key_down` to dismiss the palette.
   - In `crates/app/src/app/modals.rs`:
     - Update `attach_modal_overlays` to pass the updated parameters to `CommandPalette::new`.
2. **Windows Ctrl+K Shortcut Fix**:
   - In `crates/app/src/app/mod.rs`:
     - In `root.on_key_down`, detect `(event.keystroke.modifiers.control || event.keystroke.modifiers.platform) && event.keystroke.key.eq_ignore_ascii_case("k") && !event.keystroke.modifiers.alt`.
     - Toggle Command Palette and stop propagation.
3. **Verification**:
   - Run `cargo fmt --check`.
   - Run `cargo clippy --all-targets -- -D warnings`.
   - Run `cargo test`.
