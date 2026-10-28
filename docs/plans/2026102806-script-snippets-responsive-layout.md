# Plan: Responsive Snippets Layout for Script Editor (Pre-request & Post-response)

## 1. Goal
Dynamically adjust the layout of the snippets panel inside the Pre-request and Post-response (Tests) script editors based on the application's split layout:
- **Layout Dọc (Left / Right columns - `split_layout_vertical == false`)**:
  - The composer pane is narrow in width.
  - Snippets are placed on **TOP** (`đưa snippets lên trên`) as a single horizontal row with horizontal scroll (`1 hàng ngang kèm theo scroll`).
  - Below it, the code editor gets the full width of the column and maximum vertical working space.
- **Layout Ngang (Top / Bottom rows - `split_layout_vertical == true`)**:
  - The composer pane is wide across the screen, but limited in vertical height.
  - Snippets are placed on the **RIGHT** as a vertical column (`w(px(200.))`), keeping the full vertical height intact for the code editor to work comfortably.

## 2. Context & Architectural Alignment
- In `ModalState`, `split_layout_vertical == false` is the default 2-column layout (Left: Composer, Right: Response).
- In `ScriptEditor` (`crates/app/src/components/script_editor.rs`):
  - `split_layout_vertical == false`: Top horizontal scrollable bar for snippets, Editor below it.
  - `split_layout_vertical == true`: Right vertical column for snippets, Editor on left.

## 3. Implementation Steps
1. Swap the `if self.split_layout_vertical` branches in `ScriptEditor::render` to match the user's layout definition.
2. Verify with `cargo test`, `cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings`.
