# Implementation Plan: Fix Text Input Vertical Alignment, Padding & Key Width

## 1. Goal Description
The user reported three UI/UX issues in the key-value text inputs (visible in the Params / Headers / Form tables, as well as Environment / Auth inputs):
1. **Vertical Text Alignment**: Text inside input fields feels shifted upward rather than vertically centered.
2. **Excessive Horizontal Padding**: Inner horizontal padding is too large, creating unnecessary empty space before and after text.
3. **Key Column Width**: The Key input column width is too wide (currently 220px), unnecessarily constraining the Value column space.

This plan details the root causes and the surgical fixes to restore clean vertical centering, compact horizontal padding (from ~16px down to 6px), and shrink the Key column width to 160px to give Value significantly more space.

---

## 2. Root Cause Analysis

### 2.1 Vertical Text Misalignment ("Text lệch lên trên")
In `crates/app/src/components/env_input.rs` and `env_modal.rs`:
- An outer container `div().h(px(28.)).px(px(6.)).rounded(px(4.)).bg(t.bg_input).border_1().flex().items_center()` wraps an `Input::new(input_state)` element.
- `Input` in `gpui-component` has a default `Size::Medium`.
- In `gpui-component/src/input/input.rs`, `Input::render` applies:
  - `.input_h(Size::Medium)` which sets the height to `h_8` (**32px**).
  - `.input_py(Size::Medium)` which sets `py(px(8.))`.
- When this 32px element with 8px top padding is placed inside a 28px outer flex container with `items_center()`:
  - Taffy centers the 32px element within the 26px inner content area: `(26 - 32) / 2 = -3px` top offset.
  - The inner `padding_top = 8px` results in text content beginning at `-3px + 8px = 5px` from the top border.
  - Meanwhile, the bottom has a much larger gap (~11-12px for lowercase text).
- **Solution**: Explicitly set `.with_size(Size::Small).h_full().py(px(0.))` on `Input::new(...)`. By overriding the height to 100% of the inner container and zeroing the inner vertical padding, GPUI's flex layout centers the single-line text element with equal spacing top and bottom (`(26 - 12) / 2 = 7px` inner, `8px` outer).

### 2.2 Excessive Horizontal Padding ("Padding 2 bên quá lớn")
- The outer container specifies `.px(px(6.))`.
- The inner `Input::new` applies `input_px(Size::Medium)` = `10px`.
- In addition, when `cleanable(true)` is set, `has_suffix` appends an additional `this.pr(self.size.input_px())` (another 10px).
- This results in a massive **16px** padding on the left before the first character, and 16px+ on the right.
- Furthermore, read-only/disabled rows (`render_disabled_input`) only use `px(px(6.))` with no inner input, causing editable and read-only rows to misalign horizontally by 10px!
- **Solution**: Set `.px(px(0.))` on `Input::new(...)` inside `render_env_input` and use `Size::Small` for suffix padding. The outer `px(px(6.))` container padding will then directly control the inset (6px on the left, 6px on the right). Editable text will align with disabled text at 6px.

### 2.3 Key Column Width ("Chiều dài input key giảm đi")
- In `crates/app/src/components/kv_table.rs`:
  - Header column: `.child(div().w(px(220.)).child("Key"))`
  - Read-only row: `.child(div().w(px(220.)).child(render_disabled_input(&row.key, t)))`
  - Editable row: `.child(div().w(px(220.)).child(render_env_input(...)))`
- 220px is disproportionately wide for API keys/headers (e.g., `input`, `page`, `location`, `Authorization`), leaving less space for long values like URLs, coordinates, and query strings.
- Note: `crates/app/src/components/body_editor/multipart.rs` already uses `160px` (`w(px(160.))`).
- **Solution**: Change `KvTable` default key width from `220px` to `160px`, saving 60px that directly expands the `flex_1()` Value column. Also expose `.key_width(Pixels)` on `KvTable` for customization if needed.

---

## 3. User Review Required

> [!IMPORTANT]
> - **Key Width Reduction**: The Key column will be changed from `220px` to `160px` (a 60px reduction, given entirely to the Value column). 160px comfortably accommodates common keys (up to ~20 characters in 12px monospace) without horizontal scrolling.
> - **Horizontal Inset**: Left padding of editable text fields will change from `16px` to `6px`, matching the disabled/inherited rows.

---

## 4. Proposed Changes

### Component 1: `crates/app/src/components/env_input.rs`
#### [MODIFY] `crates/app/src/components/env_input.rs`
- Import `gpui_component::Sizable as _;` and `gpui_component::sizing::Size`.
- On `Input::new(input_state)`:
  - Add `.with_size(Size::Small)` (or `.small()`).
  - Add `.h_full()`.
  - Add `.py(px(0.))`.
  - Add `.px(px(0.))`.
- This eliminates the double horizontal padding and fixes vertical text centering in all `render_env_input` call sites (`KvTable`, `AuthTab`, `Multipart`, `ScopeSettingsModal`).

```diff
@@ -14,6 +14,7 @@
 use gpui_component::input::{Input, InputState};
 use gpui_component::scroll::ScrollableElement as _;
+use gpui_component::sizing::Size;
+use gpui_component::Sizable as _;

...
                 Input::new(input_state)
                     .id(input_id.clone())
+                    .with_size(Size::Small)
                     .appearance(false)
                     .bordered(false)
                     .focus_bordered(false)
                     .cleanable(true)
+                    .h_full()
+                    .py(px(0.))
+                    .px(px(0.))
                     .text_xs()
                     .font_family(".SystemMonoFont")
```

---

### Component 2: `crates/app/src/components/kv_table.rs`
#### [MODIFY] `crates/app/src/components/kv_table.rs`
- Add a `key_width: Pixels` field to `KvTable`, defaulting to `px(160.)`.
- Provide a `.key_width(mut self, width: Pixels) -> Self` builder method.
- In `render()`:
  - Replace the 3 occurrences of `px(220.)` (header, read-only row, editable row) with `self.key_width`.

```diff
@@ -58,6 +58,7 @@
     pairs: Vec<KeyValuePair>,
     readonly_rows: Vec<InheritedHeaderRow>,
+    key_width: Pixels,
     on_update: Box<dyn Fn(usize, String, String, bool, &mut Window, &mut App)>,
...
@@ -75,6 +76,7 @@
         Self {
             id_prefix: id_prefix.into(),
             pairs,
             readonly_rows: Vec::new(),
+            key_width: px(160.),
             on_update: Box::new(on_update),
...
+    pub fn key_width(mut self, width: Pixels) -> Self {
+        self.key_width = width;
+        self
+    }
...
-                    .child(div().w(px(220.)).child("Key"))
+                    .child(div().w(self.key_width).child("Key"))
...
-                    .child(div().w(px(220.)).child(render_disabled_input(&row.key, t)))
+                    .child(div().w(self.key_width).child(render_disabled_input(&row.key, t)))
...
-                    .child(div().w(px(220.)).child(render_env_input(
+                    .child(div().w(self.key_width).child(render_env_input(
```

---

### Component 3: `crates/app/src/components/env_modal.rs`
#### [MODIFY] `crates/app/src/components/env_modal.rs`
- Apply `.with_size(Size::Small).h_full().py(px(0.)).px(px(0.)).text_xs().font_family(".SystemMonoFont")` to the Key, Initial Value, and Current Value inputs in the environment editor table so all environment modal fields share the same centered text and compact padding.

---

## 5. Verification Plan

### Automated Tests
Run cargo test and clippy to verify zero regressions and zero warnings:
```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

### Manual Verification
1. Launch the app (`cargo run`).
2. Open any request with query params (Params tab) or headers (Headers tab).
3. Observe:
   - Key column width is noticeably more compact (160px instead of 220px), giving Value much more room.
   - Text inside Key and Value fields is vertically centered between top and bottom borders (no upward bias).
   - Horizontal padding on the left is tight and clean (6px instead of 16px), aligning with disabled/read-only rows.
   - Clear button 'x' is cleanly spaced on the right with no clipping.
4. Check Environment Modal (`Ctrl+,` or Env switcher) to verify key/value inputs are vertically centered and have compact padding.

---

## 6. Architecture & UX Decisions (2026-10-26)

- **Horizontal Padding Elimination (`.px(px(0.))` on `Input`)**: Removed `gpui-component`'s default `input_px(10px)` inside `render_env_input`, `env_modal`, and `command_palette`. This eliminates the redundant 10px dead space on the left and right, leaving only the outer container's clean padding (6px) and perfectly aligning editable inputs with read-only/disabled rows.
- **Vertical Padding**: Left completely natural (no vertical hack paddings `pt`/`py`) to prevent regressions with future framework updates.
- **Font Size Upgrade to `sm` (14px)**:
  - Applied to `env_input` and inline environment token badges.
  - Applied to `env_modal` key, initial value, and current value fields.
  - Applied to `request_bar` (URL input box and inline tokens).
  - Applied to `command_palette` (search input, action item titles, request switcher names, and environment switcher names).
- **Key Column Width**: Reduced from `220px` to `160px` in `KvTable` via `pub const KEY_COLUMN_WIDTH: Pixels = px(160.);` without speculative builder methods or `#[allow(dead_code)]`.
