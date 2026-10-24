# UI: Two-Type Autocomplete Popup with Submenu-Style Detail Panel

**Date:** 2026-10-24
**Type:** UI simplification (feature refinement)

## Goal

The env-variable autocomplete popup shows `{{key}} = "value"` plus a per-scope badge (`Active: <env>` / `Global` / `Dynamic`) on every row, which is visually noisy. Simplify it to:

- Rows show only the key and a two-category type badge: **Variable** (merges Active + Global) or **Dynamic**.
- A submenu-style detail panel (right column of the popup, VS Code-style) shows details of the currently selected item: resolved value for Variables, description for Dynamic vars.
- Hovering a row moves the selection to it, so mouse-only users get the same detail preview; keyboard navigation is unchanged.

```
┌───────────────────────┬─────────────────────────────┐
│ {{baseUrl}}  Variable │ {{baseUrl}}                 │
│ {{apiKey}}   Variable │ Global                      │
│ {{$guid}}    Dynamic ←│ ••••••••                    │
│ {{$randomEmail}} Dyn  │                             │
└───────────────────────┴─────────────────────────────┘
      list 240px                detail panel 280px
```

## Context

- Popup rows were rendered as near-identical inline copies in two places: `render_env_input` (`components/env_input.rs`) and the URL bar (`components/request_bar.rs`). Any row change had to be made twice.
- Dynamic-variable descriptions were hardcoded as a `(name, description)` array inside `get_env_completions` and hijacked into the `value` field of `EnvVarCandidate`; `fake_vars.rs` duplicated the same names in `FAKE_VAR_RE`.
- `EnvVarKind` existed but was only derived by re-parsing the scope string at render time.
- `components/env_autocomplete.rs` was 590 LOC; adding shared renderers there would exceed the ~650 LOC modularity limit (AGENTS.md §3.4).

## Architectural Decisions

- **Single source of truth for dynamic vars:** add `FAKE_VAR_CATALOG: &[(&str, &str)]` to `zest_core::scripting::fake_vars` (name + description) and derive `FAKE_VAR_RE`'s alternation from it, so the regex, the generator, and the UI descriptions can no longer drift apart.
- **Folder module split:** convert `components/env_autocomplete.rs` into `env_autocomplete/mod.rs` (data + logic, ~608 LOC) + `env_autocomplete/render.rs` (shared UI, ~193 LOC). All external import paths (`crate::components::env_autocomplete::*`) stay valid via `pub use render::*`.
- **Model carries kind:** `EnvVarCandidate` gains `kind: EnvVarKind` (assigned at build time) and `description: Option<String>`; `value` for dynamic vars becomes a generated sample. A `detail_text()` helper (description if present, else value) feeds the three LSP-style completion providers (`EnvCompletionProvider`, two spots in `script_editor.rs`).
- **Shared renderers:** `render_candidate_row` (key + type badge, hover-select, click → dismiss + on_select callback) and `render_candidate_detail_panel` (title, scope line for variables, wrapped value/description). Both popups consume them, eliminating the duplicated inline row blocks.
- **Two-category badge:** `candidate_type_badge(kind)` maps Active|Global → "Variable" (sky `0x38bdf8`), Dynamic → "Dynamic" (amber `0xf59e0b`); key text keeps the type color. Inline tokens and the token hover tooltip keep their three-color per-scope scheme — only the popup collapses to two types.
- **Popup layout:** container switches `flex_col` → `flex_row` (total 520px = list 240 + detail 280). The tracked scroll div still contains only rows and `.vertical_scrollbar(&scr_h)` stays outside it (now on the list-column wrapper) to preserve the `scroll_to_item` index fix from `20261024-autocomplete-scroll-follow.md`.
- **Detail panel scrolling:** content divs are stateful (`.id(...)`) with `overflow_y_scroll` — gpui keeps per-element scroll state without `track_scroll`, so wheel scrolling works with no extra handles.

## Step-by-Step Implementation Tasks

1. `crates/core/src/scripting/fake_vars.rs`: add `FAKE_VAR_CATALOG`, derive `FAKE_VAR_RE` from it, add consistency test (every catalog name has a generator + non-empty description); export from `scripting/mod.rs`.
2. `git mv env_autocomplete.rs → env_autocomplete/mod.rs`; extend `EnvVarCandidate` (`kind`, `description`, `detail_text()`); update `get_env_completions` (kinds assigned, dynamic entries built from `FAKE_VAR_CATALOG` with sample values); update `EnvCompletionProvider` to use `detail_text()`.
3. New `env_autocomplete/render.rs`: `candidate_type_badge`, `render_candidate_row`, `render_candidate_detail_panel`, width consts.
4. `env_input.rs` and `request_bar.rs`: restructure the popup container to `flex_row` with list column + detail panel, replace inline rows with `render_candidate_row`, add `render_candidate_detail_panel` fed by `candidates.get(selected_index)`.
5. `script_editor.rs`: switch the two completion providers from `c.value` to `c.detail_text()`.
6. Update `test_get_env_completions_and_details` to assert `kind`/`description`/`detail_text()`.

## Risk Mitigation

- **Popup width 520px** may clip at the window's right edge in narrow contexts (e.g. kv-table cells near the edge) — same class of issue as the previous 340/380px popups; accepted for now.
- **Hover-select mutates `selected_index`:** intentional (detail follows hover like a submenu); `use_keyed_state` entities observe their owning view, so the update re-renders the popup. `scroll_to_item` is only invoked from the keyboard handler, so no scroll jumps on hover.
- **Untracked scroll state** on the detail content divs is keyed by element id (`cand-detail-{val,desc}-{name}`), so scroll position resets per candidate — desirable for a preview panel.

## Verification Steps

- `cargo fmt --check`, `cargo test` (core 73 + app 66 passing), `cargo clippy --all-targets -- -D warnings` → 0 warnings.
- Manual: type `{{` in the URL bar and any env input — rows show key + Variable/Dynamic badge only; ArrowUp/Down moves the highlight and the detail panel follows; hovering a row previews it; selecting a secret variable shows `••••••••` with its scope line; selecting `$randomEmail` shows its description.

## Follow-up (Round 2): compact detail tooltip beside the popup

**Feedback:** the 280px in-popup detail column is too heavy. Wanted: a small one-line tooltip floating next to the popup (Bruno-style) — value/description only, no key repetition, no scope line; positioned beside the popup (exact row alignment not required).

**Complexity assessment:** simple. Exact row alignment was the only costly part — `ScrollHandle::child_bounds` is private in gpui-pre 0.3.9 (only `offset()` is public), so aligning to the selected row would require estimating row heights. Since "beside the popup is enough", the tooltip is top-aligned with the popup instead.

**Changes:**

- `render.rs`: `render_candidate_detail_panel` → `render_candidate_detail_tooltip` — a compact one-line tooltip (`max_w 360`, mono `text_xs`, truncate, border tinted by type color, `occlude()`), mirroring the existing `variable_value_tooltip` pattern. `CANDIDATE_DETAIL_WIDTH` (280) → `CANDIDATE_DETAIL_GAP` (8).
- `env_input.rs` / `request_bar.rs`: popup reverts to a single 240px column (tracked list div + `vertical_scrollbar` on the container, per the scroll-follow fix); the detail renders as a second `deferred` element at `left(popup_left + CANDIDATE_LIST_WIDTH + gap)`, `top` matching the popup, priority 100.
- Selection behavior unchanged: the tooltip follows keyboard selection and hover.

## Verification Steps (Round 2)

- `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings` → all green.
- Manual: type `{{` — the popup is a single narrow list; the selected/hovered item's value (or description for `$` vars) appears in a small tooltip beside the popup and updates as the selection moves.

## Follow-up (Round 3): compact detail card with restored content

**Feedback:** restore the richer detail content from Round 1 (key, source scope, wrapped value/description), but the detail must be **shorter than the popup** — it should not stretch to the list's height.

**Design:** the detail renders as a compact card floating next to the popup (Round 2 placement), sized to its content:
- Fixed width 280px, height hugs the content (typically 3 lines, well below the list height), capped at 160px with an inner scroll area for very long values.
- Content mirrors the Round 1 panel: `{{name}}` title (bold mono, type color), muted scope line for environment variables, wrapped value (mono) or description.
- Card chrome: `bg_card` + neutral border + `shadow_2xl` + `rounded(6)` + `occlude()`, looking like a small companion panel of the popup rather than a one-line tooltip.
- A full-height in-popup column cannot be "shorter than the list" without leaving dead space inside the popup border, hence the card floats beside the popup as its own box.

**Changes:** `render_candidate_detail_tooltip` → `render_candidate_detail_card` in `render.rs` (consts: `CANDIDATE_DETAIL_WIDTH` 280 restored, `CANDIDATE_DETAIL_MAX_HEIGHT` 160 private); call sites in `env_input.rs` / `request_bar.rs` renamed only — popup layout and deferred placement are unchanged.

## Verification Steps (Round 3)

- `cargo fmt --check`, `cargo test` (core 73 + app 66), `cargo clippy --all-targets -- -D warnings` → all green.
- Manual: type `{{` — the detail card beside the popup shows the selected item's key/scope/value, wraps long values, stays visibly shorter than the popup list, and follows keyboard selection and hover.
