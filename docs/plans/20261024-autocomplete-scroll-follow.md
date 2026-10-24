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

## Verification Steps

- `cargo test` (core + app) passes; `cargo clippy --all-targets -- -D warnings` reports 0 warnings.
- Manual: type `{{` in the URL bar and env inputs with more than ~7 variables; press ArrowDown/ArrowUp repeatedly including wrap-around (last → first): the highlight must always remain inside the viewport; mouse-wheel scrolling and the overlay scrollbar still work.
