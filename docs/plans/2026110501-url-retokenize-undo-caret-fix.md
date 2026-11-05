# Plan 2026110501 — URL Retokenization: Undo Poisoning, Caret Theft & Hot-Path Cost

## Goal

Fix three defects in the `{{var}}` → inline-token conversion wired into the URL input's
`InputEvent::Change` handler (`app/subscriptions.rs` → `attach_missing_var_tokens`), which was
verified empirically with a GPUI test-context probe before implementation:

1. **Undo poisoning (critical)** — once a `{{var}}` was ever typed/pasted, `Ctrl+Z` in the URL
   input stopped making progress. Each undo replay removes the token, emits `Change`, and the
   handler immediately re-attaches it (pushing a *new* conversion onto the undo stack). The user
   can never undo past the first conversion.
2. **Caret theft** — converting spans parks the caret at the end of the last converted span.
   Pasting `a{{h}}/x/{{b}}/tail` moved the caret from offset 19 to 14 (probe-verified).
3. **Hot-path waste** — on *every* keystroke the handler pre-computed
   `effective_environment()` and `scan_variables` built the env `HashMap` and generated RNG-backed
   fake-variable samples (`{{$uuid}}` etc.) although `attach_missing_var_tokens` only consumes
   span positions and names, never `is_resolved`/`value`.

## Context

- gpui-kit 0.8.1 never auto-tokens typed/pasted text; conversion must run in the `Change`
  subscription (`replace_range_with_token` preserves undo + caret, records an atomic undo step).
- Conversion keeps the text byte-identical, so the re-emitted `Change` carries the same value.
  Undoing a conversion also restores the same value. Both events are indistinguishable from a
  real edit by content alone — a guard is required.
- `InputEvent::Change` carries no edit metadata and gpui-kit 0.8.1 exposes no public
  transaction API to batch conversions into one undo entry (checked `gpui-base-0.8.1`), so
  multi-span conversion still records one undo entry per span; the guard guarantees every
  `Ctrl+Z` press now pops one entry and makes progress.

## Architectural Decisions

- **`RetokenizeGuard` (value-diff guard)** lives in `components/env_autocomplete/var_tokens.rs`
  and is captured by the URL subscription as `Rc<RefCell<_>>`. `should_process(&value)` returns
  `false` for value-identical `Change` events, skipping *both* retokenization and request sync
  (URL sync on an unchanged value is redundant work — curl check, `apply_url`, `mark_dirty`).
- **Known trade-off (documented on the type)**: re-pasting the exact same value over the whole
  input is also skipped, so tags return on the next keystroke instead of immediately. This is
  strictly preferable to a poisoned undo stack.
- **Caret restoration inside `attach_missing_var_tokens`**: capture `cursor()` before the loop,
  `set_selected_range(caret..caret)` after conversion when the caret moved. Attach never changes
  text, so the pre-attach offset is always valid.
- **Lean core scanner `scan_variable_spans(text)`** in `zest-core/src/interpolation.rs`:
  regex-only, no env map, no fake-var generation. `attach_missing_var_tokens` drops its
  `&Environment` parameter entirely, which also removes the per-keystroke
  `effective_environment()` call (and its E0502-avoidance dance) from the subscription.
- `build_tokenized_content` still uses `scan_variables` (discrete user actions only: request
  switch, autocomplete pick, import) — switching it to the lean scanner would orphan its `env`
  parameter across ~10 call sites and is deferred.

## Step-by-Step Implementation Tasks

1. `crates/core/src/interpolation.rs`: add `pub fn scan_variable_spans` + unit test
   (`test_scan_variable_spans_positions_and_names`).
2. `crates/app/src/components/env_autocomplete/var_tokens.rs`:
   - `attach_missing_var_tokens(inp, window, cx)` (env param removed), caret restore, lean scan.
   - Add `RetokenizeGuard`.
   - Update the 3 existing tests; add `test_attach_missing_var_tokens_preserves_caret` and the
     integration test `test_retokenize_guard_lets_undo_progress` (renders a real `Input`,
     wires the guard like `subscriptions.rs`, asserts undo pops one conversion per press and
     redo restores value + tokens).
3. `crates/app/src/components/env_autocomplete/mod.rs`: re-export `RetokenizeGuard`.
4. `crates/app/src/app/subscriptions.rs`: guard wiring + early return; drop
   `effective_environment()` pre-computation; reuse the already-read `val`.

## Risk Mitigation

- The guard mirrors-then-tests approach keeps `subscriptions.rs` wiring 1:1 with the integration
  test, so regressions in either surface immediately.
- Undo of conversions now shows tags turning back into raw `{{var}}` text one per press —
  expected behavior, matching the per-span undo entries gpui-kit records.

## Verification Steps

```bash
cargo fmt --check
cargo test -p zest-core -p zest          # 114 app + 105 core pass, incl. 5 var_tokens tests
cargo clippy --all-targets -- -D warnings  # 0 warnings
```
