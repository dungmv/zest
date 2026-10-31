# Implementation Plan: Line Wrap Toggle Button Icon Color Styling

- Date: 2026-10-31
- Sequence: `2026103101`
- Plan File: `docs/plans/2026103101-line-wrap-toggle-icon-color.md`

## 1. Goal Description
In the Response Inspector sub-toolbar, the **Toggle Line Wrap** button currently switches to `.primary()` variant when active (`wrap_lines == true`). This renders a prominent, solid-accent pill background (`#3b82f6` in dark mode) that visually dominates the toolbar compared to neighboring ghost buttons (`Save`, `Copy`, `Download`).

The goal is to reduce visual distraction by keeping the button in `.ghost()` mode regardless of state, changing **only the icon color** (`t.primary` accent color when active, default ghost foreground when inactive).

## 2. User Review Required

> [!NOTE]
> The existing `gpui-kit` `Button` component fully supports passing a styled `Icon` via `.icon(Icon::new(IconName::TextWrap).text_color(t.primary))`. Therefore, we do **not** need to create a custom UI component from scratch. Using `Button` preserves tooltips, keyboard accessibility, sizing, and consistent hover behavior.

- **Active State Appearance**:
  - Background: Transparent (ghost), matches neighboring buttons (`Save as Example`, `Copy`, `Download`).
  - Icon: Accent blue (`t.primary`), indicating that line wrap is active without visual overload.
  - Hover: Subtle `t.bg_hover` ghost background.
- **Inactive State Appearance**:
  - Background: Transparent (ghost).
  - Icon: Muted foreground (`IconName::TextWrap` default).
  - Hover: Subtle `t.bg_hover` ghost background.

## 3. Open Questions
None. The requirement is straightforward and directly addresses visual noise reduction.

## 4. Proposed Changes

### Component: Response Inspector

#### [MODIFY] `crates/app/src/components/response_inspector.rs`
Update the wrap button construction in lines ~646–671:
- Eliminate `btn.primary()` when `wrap_lines` is active.
- Construct the icon dynamically:
  ```rust
  let wrap_icon = if wrap_lines {
      Icon::new(IconName::TextWrap).text_color(t.primary)
  } else {
      Icon::new(IconName::TextWrap)
  };
  ```
- Always configure the button as `.ghost()`:
  ```rust
  Button::new("btn-toggle-wrap")
      .ghost()
      .xsmall()
      .icon(wrap_icon)
      .tooltip(wrap_tip)
      .on_click(move |_, window, cx: &mut App| {
          rs_w.update(cx, |state, cx| {
              state.toggle_wrap_lines(cx);
          });
          let new_wrap = rs_w.read(cx).wrap_lines;
          r_ed.update(cx, |ed, cx| {
              ed.set_soft_wrap(new_wrap, window, cx);
          });
      })
  ```

## 5. Verification Plan

### Automated Tests
Execute the project verification pipeline:
```bash
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
```

### Manual Verification
1. Launch Zest: `cargo run`.
2. Send any HTTP request that returns a response (e.g. JSON with long lines).
3. Observe the Response Inspector sub-toolbar:
   - Line Wrap inactive: Button is ghost with default icon color.
   - Click Line Wrap: The editor wraps lines; the button background remains transparent/ghost; the icon turns blue (`t.primary`); tooltip displays `"Line Wrap: Enabled"`.
   - Click again: Line wrap turns off; icon color returns to default; tooltip displays `"Line Wrap: Disabled"`.
