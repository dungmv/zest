# Fix: Autocomplete Popup Scroll-Follow on Keyboard Navigation

**Date:** 2026-10-24
**Type:** Bug fix (UI)

## Goal

When navigating the env-variable autocomplete popup with `ArrowUp`/`ArrowDown`, the highlighted (selected) row must always scroll into the visible viewport. Currently the highlight silently moves outside the popup so the user sees nothing.

## Context

Commit `f330048` ("simplify env autocomplete popup and add keyboard navigation") wired `ScrollHandle::scroll_to_item()` into `handle_autocomplete_keydown` (`crates/app/src/components/env_autocomplete.rs:413, 430`), but the scrolling never worked:

- The popup list is rendered with `.track_scroll(&scr_h).overflow_y_scrollbar()` in both popups (`crates/app/src/components/env_input.rs:270-271`, `crates/app/src/components/request_bar.rs:370-371`).
- `overflow_y_scrollbar()` is provided by gpui-component's `ScrollableElement` trait and wraps the div in a `Scrollable` element that creates its **own** keyed internal scroll handle, attaches `track_scroll + overflow_y_scroll` to an inner area div, and demotes the caller's div to a non-scrolling content child (also overriding its `.flex_1()`).
- Therefore the app's `scr_h` is tracked on an element whose `overflow.y != Overflow::Scroll`, and in gpui-pre the vertical adjustment branch of `scroll_to_item` is skipped — a silent no-op.

## Architectural Decisions

- Mirror the proven pattern from gpui-component's own `popup_menu.rs`: make the app's `scr_h` the **real** handle on the element that actually scrolls, i.e. `.track_scroll(&scr_h).overflow_y_scroll().vertical_scrollbar(&scr_h)` on the same stateful div.
- Keep the overlay scrollbar visual (`vertical_scrollbar`) — matches the "minimal UI" preference; no behavior change for mouse scrolling.
- No changes to `env_autocomplete.rs`: the existing `scroll_to_item` calls start working once the handle sits on the correct element.
- Keep `.flex_1()` on the list div; Taffy resolves `min-size: auto` to 0 for scroll containers, so the list still stretches/shrinks correctly inside the `max_h`-capped popup.

## Step-by-Step Implementation Tasks

1. `crates/app/src/components/env_input.rs` (div `{}-auto-list`): replace `.overflow_y_scrollbar()` with `.overflow_y_scroll()` and add `.vertical_scrollbar(&scr_h)`.
2. `crates/app/src/components/request_bar.rs` (div `url-auto-list`): identical replacement.
3. No import changes needed — `ScrollableElement` is already imported in both files.

## Risk Mitigation

- Layout regression risk (list not filling / overflowing the popup): mitigated by keeping `.flex_1()` + `.overflow_hidden()` on the popup container; verify manually with few and many candidates.
- If a future gpui-component upgrade changes `Scrollable` semantics, prefer this direct pattern (handle + scroll + scrollbar on one element) over the wrapper for popups that need programmatic `scroll_to_item`.

## Follow-up (Round 2): off-by-one item after the first fix

**Symptom:** navigating down leaves the selected row one item outside the viewport; navigating up always shows one extra item above the selection.

**Root cause:** `ScrollableElement::vertical_scrollbar()` (gpui-component 0.7.3, `scroll/scrollable.rs:19-31`) adds a `ScrollbarLayer` child via `self.child(...)`. The first fix placed `.vertical_scrollbar(&scr_h)` **before** `.children(rows)` on the tracked list div, so the div's children became `[ScrollbarLayer, row0, …, rowN-1]`. `ScrollHandle::scroll_to_item(ix)` indexes into the handle's `child_bounds`, which mirrors child order (`gpui-pre` `div.rs:1994-2000`) — every index was shifted by one: `scroll_to_item(i)` scrolled to row `i-1` (down → selection stays outside; up → one surplus row above the selection).

**Fix:** mirror `popup_menu.rs` (`menu/popup_menu.rs:1516-1527`): keep `.track_scroll(&scr_h).overflow_y_scroll()` on the list div (whose children are now rows only, so `child_bounds[i] == row i`) and move `.vertical_scrollbar(&scr_h)` onto the popup container as a sibling appended after the list child. The container is not tracked by the handle, so its child order cannot shift indices, and the `absolute().inset_0()` overlay no longer translates with the scroll offset.

## Verification Steps

- `cargo test` (core + app) passes; `cargo clippy --all-targets -- -D warnings` reports 0 warnings.
- Manual: type `{{` in the URL bar and env inputs with more than ~7 variables; press ArrowDown/ArrowUp repeatedly including wrap-around (last → first): the highlight must always remain inside the viewport; mouse-wheel scrolling and the overlay scrollbar still work.
