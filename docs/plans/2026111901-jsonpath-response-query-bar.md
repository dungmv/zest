# Plan 2026111901 — JSONPath Response Query Bar

> **Note:** This plan document was committed alongside its implementation per the
> AGENTS.md § 4 plan-storage protocol.

## Goal

Add a **JSONPath query bar** to the response Body tab toolbar: typing
`$.data.items[*].id` filters the JSON response view down to the matched nodes
(Postman-style replace-view UX), with an inline match-count / error hint.

## Context

- The response Body tab (`crates/app/src/components/response_inspector/body_tab.rs`)
  only ever shows the full JSON body — no way to quickly extract data from large
  responses.
- `ResponseState::build_cached_format` already parses the body with
  `serde_json` for pretty-printing, but **discards the parsed `Value`** — the
  query feature retains it so per-keystroke evaluation is cheap.
- Dead pre-existing fields `search_query`/`is_searching` (+ setters) on
  `ResponseState` are replaced by the new query state.

## Architectural Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Query language | JSONPath only (RFC 9535) via `serde_json_path` | User example is JSONPath; XML/XPath deferred. Pure crate, evaluates on `&serde_json::Value`. |
| Result UX | Replace body view with filtered pretty JSON + "N matches" hint | Simple, correct, Postman-parity. In-place highlight would require span tracking serde_json does not keep. |
| Evaluation timing | Live per keystroke, on the cached parsed `Value` | Sidebar search precedent; parse (expensive) is cached, eval is O(tree) ms-level. |
| Result text format | 1 match → pretty node; N>1 → pretty JSON array; 0 → empty | Predictable, copyable. |
| Layering | Evaluator lives in `zest-core` (pure, unit-tested) | Core owns pure domain logic; app only orchestrates (AGENTS.md § 1.1). |
| Theme | Reuse existing tokens (monochrome, no new colors) | Monochrome UI direction; warnings use `t.var_unresolved` like the large-payload banner. |
| Query lifecycle | Cleared on new response load (switch request/example) | Stale queries silently filtering a different response is confusing. |

## Step-by-Step Implementation Tasks

1. **Plan artifact** — this file, committed with the implementation.
2. **zest-core evaluator** — `crates/core/src/json_path.rs` + `serde_json_path` dep:
   - `pub struct JsonQueryResult { result_text, lines, match_count }`
   - `pub fn run_json_path(root: &Value, query: &str) -> Result<JsonQueryResult, String>`
   - Unit tests: `$`, `$.data.items[*].id`, `$..id`, `$[?(@.price<10)]`, invalid syntax,
     scalar/array roots.
3. **ResponseState** (`crates/app/src/state/response_state.rs`):
   - `CachedResponseFormat.parsed_json: Option<Arc<serde_json::Value>>` (retain parsed value).
   - Replace dead `search_query`/`is_searching` with `jsonpath_query: String`;
     `FormattedResponseBody` gains `match_count: Option<usize>` + `jsonpath_error: Option<String>`.
   - `refresh_formatted_body()` applies the query when non-empty (result always
     pretty JSON regardless of Pretty/Raw view mode; syntax error keeps base display
     + inline error).
   - `set_jsonpath_query(query, cx)` → refresh → `ResponseEvent::ContentChanged
     { sync_sent_preview: false }` → `cx.notify()` (existing event flow re-syncs the editor).
   - Clear `jsonpath_query` in all response-load paths.
4. **body_tab.rs UI** — `render_body_tab` gains `window: &mut Window`; sidebar-style
   `use_keyed_state` InputState + `InputEvent::Change` subscription + two-way sync
   guard; toolbar row (Pretty/Raw | query input | hint | icon buttons) only rendered
   when the body parsed as JSON.
5. **Cleanup** — dead search fields/setters removed.

## Risk Mitigation

- Per-keystroke eval on very large JSON is O(tree) on the cached `Value`
  (~ms for 10 MB); the expensive parse step is cached per response. Debounce is a
  follow-up if needed.
- `parsed_json` is retained only for the **active** response (`formatted_body`),
  so memory stays bounded.
- Render purity preserved: all mutations happen in input event handlers.

## Verification Steps

1. `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`.
2. Manual: JSON response query filters + match count; invalid query shows inline
   error; clear restores full body; switching requests resets the bar; GraphQL
   responses work; non-JSON hides the bar; copy copies the filtered result;
   save-to-file still writes raw bytes.
