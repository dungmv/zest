# Plan: Core `zest_core::env` Domain + App `crate::env` UI Binding + Code Editor Env Decorations & LSP

- **Date:** 2026-11-23
- **Spec:** [`docs/specs/07-ui-ux-enhancements.md`](../specs/07-ui-ux-enhancements.md) (§5.3, §5.4)
- **Target Crates:** `crates/core` (`zest-core`), `crates/app` (`zest`)

---

## 1. Goal

1. **Centralize Pure Env Business Logic in `crates/core/src/env/` (`zest_core::env`):**
   - Move variable precedence (`Active Env → Global → Dynamic ($...) → Missing`), first-wins lookup, secret masking (`••••••••`), candidate listing/filtering, and `find_variable_span_at` into `zest-core` so UI, HTTP execution, scripts, and any future CLI/runner share one pure-Rust source of truth.
2. **Centralize GPUI UI Bindings in `crates/app/src/env/` (`crate::env`):**
   - Replace the overgrown 717-LOC `components/completion/env_autocomplete/` with a clean top-level UI binding module `crates/app/src/env/` serving all controls: **Input** (`env_input`, `request_bar`, `kv_table`, `auth_inputs`, `multipart`), **Editor** (JSON, XML, GraphQL, gRPC), and **Script** (`script_editor`).
3. **Code Editor Scope-Aware Pill Decorations (`RangeDecoration` + `TextDecoration`):**
   - Render each `{{variable}}` span in multi-line code editors (`EditorState`) with a tinted pill background (`RangeDecorationStyle::Fill`) and scope-colored semibold text (`TextDecoration`) matching `Active`, `Global`, `Dynamic`, and `Missing`.
   - Keep `{{` and `}}` at full visual weight/color (do **not** dim the braces).
   - Support both quoted (`"{{bookingId}}"`) and unquoted (`{{pageSize}}`) JSON values as well as XML text/attributes.
4. **Code Editor Native LSP Interactions (`HoverProvider` + `DefinitionProvider`):**
   - **Hover:** Hovering any `{{variable}}` opens `gpui-kit`'s built-in `HoverPopover` displaying variable name, scope, and resolved display value (secrets masked as `••••••••`).
   - **Cmd/Ctrl+Click (`GoToDefinition`):** Cmd/Ctrl+hover underlines the variable span; clicking (or invoking `GoToDefinition`) routes via `lsp.show_document` (`zest://env/<var>`) to `open_variable_in_modal`, opening the Environment modal focused on that variable. Dynamic `$...` variables do not trigger Go to Definition.

---

## 2. Context & Current Architecture

- Currently, env resolution precedence (`Active → Global → Dynamic → Missing`, first-wins on duplicate keys, secret masking, candidate filtering) is duplicated between `crates/core/src/interpolation.rs` and `crates/app/src/components/completion/env_autocomplete/mod.rs` (717 LOC, exceeding `AGENTS.md` §3.4's 650-LOC limit).
- Single-line inputs use `InputState` with `InlineToken`, whereas multi-line code editors (`json_editor`, `xml_editor`, `grpc.message_editor`, `graphql.query_editor`, `graphql.variables_editor`) use `gpui_kit::component::input::EditorState`.
- In `gpui-kit 0.10.0`, `EditorState` does not expose `InlineToken`, but exposes:
  - `create_range_decorations_collection` (`RangeDecoration`, `RangeDecorationStyle::Fill`)
  - `create_decorations_collection` (`TextDecoration`, `HighlightStyle`)
  - `lsp_mut()` (`hover_provider`, `definition_provider`, `show_document`)
- `JsonHighlighter` and `XmlHighlighter` currently statically color `{{...}}` inside quoted strings with `t.var_resolved`, ignoring actual resolution state, scope, unquoted variables, hover, and click navigation.

---

## 3. Architectural Decisions

### 3.1 Clean Two-Crate Split

#### A. Pure Domain Layer — `crates/core/src/env/` (`zest_core::env`)
No GPUI, no theme, no LSP types; operates on borrowed `&Environment` references (zero unnecessary cloning):

```
crates/core/src/env/
└── mod.rs            # VariableScope, ResolvedVariable, VariableCandidate,
                      # resolve_variable(), list_candidates(), find_variable_span_at(),
                      # unit tests for precedence, masking, filtering, UTF-8 spans
```

- `VariableScope`:
  - `Active(String)` (stores active environment name)
  - `Global`
  - `Dynamic`
  - `Missing`
- `ResolvedVariable`:
  - `name: String`
  - `value: String` (display-safe value: `"••••••••"` when `is_secret`, `"Dynamic: <sample>"` for dynamic vars, `"Unresolved variable"` when missing)
  - `scope: VariableScope`
  - `is_secret: bool`
  - Helper methods: `is_resolved(&self) -> bool`, `scope_label(&self) -> String` (`"Active: <name>"` / `"Global"` / `"Dynamic"` / `"Missing"`).
- `VariableCandidate`:
  - `name: String`, `value: String`, `scope: VariableScope`, `is_secret: bool`, `description: Option<String>`
  - Helper `detail_text(&self) -> &str`.
- `resolve_variable(active_env: Option<&Environment>, globals: &Environment, var_name: &str) -> ResolvedVariable`
- `list_candidates(active_env: Option<&Environment>, globals: &Environment, query: &str) -> Vec<VariableCandidate>`
- `find_variable_span_at(text: &str, offset: usize) -> Option<VariableSpan>` (delegating to `crate::interpolation::scan_variable_spans`).
- Keep `crates/core/src/interpolation.rs` intact and exported as `zest_core::interpolation` (zero breaking changes).

#### B. GPUI UI Binding Layer — `crates/app/src/env/` (`crate::env`)
Registered in `crates/app/src/main.rs` (`mod env;`) with focused <350-LOC submodules:

```
crates/app/src/env/
├── mod.rs            # Facade re-exporting core types + UI helpers
├── style.rs          # Thin wrappers over EnvironmentState -> zest_core::env,
│                     # VariableScope -> AppTheme EnvVarStyle (text + bg_tint),
│                     # variable_value_tooltip(), open_variable_in_modal()
├── input_tokens.rs   # Single-line InputState adapter (from var_tokens.rs):
│                     # RetokenizeGuard, attach_missing_var_tokens(), build_tokenized_content()
├── editor_lsp.rs     # Multi-line EditorState adapter (NEW):
│                     # EnvEditorDecorations, EnvHoverProvider, EnvDefinitionProvider,
│                     # make_env_show_document_handler(), attach_env_lsp()
├── completion.rs     # EnvCompletionProvider (LSP wrapper around zest_core::env::list_candidates)
│                     # + detect_unclosed_env_var*() + unclosed_env_span()
└── popup.rs          # Autocomplete dropdown rendering & keyboard nav (from render.rs)
```

### 3.2 Whole-Span Uniform Styling (No Brace Dimming)
- Both `RangeDecoration` and `TextDecoration` cover the exact `span.start..span.end` byte range of `{{var}}` with uniform `color: Some(style.text)` and `font_weight: Some(FontWeight::SEMIBOLD)` — no separate dim style for `{{` / `}}`.
- Colors map directly from `VariableScope` via `crate::env::variable_style(&resolved.scope)` using existing `AppTheme` tokens (`tag_variable`, `json_boolean`, `var_resolved`, `var_unresolved` with `a: 0.14` fill tint), identical to URL bar pills.

### 3.3 Pure Event-Driven Synchronization (Zero Work in `render()`)
- Strictly obey `AGENTS.md` §2.3 (Dual-Channel Event Architecture): never compute or push decorations inside `render()`.
- Refresh `EnvEditorDecorations` from:
  1. Editor `InputEvent::Change` subscriptions in `crates/app/src/app/subscriptions.rs` (single-editor refresh).
  2. `ComposerInputs::sync_from_request` (since `set_value` suppresses `InputEvent::Change`).
  3. `handle_environment_event` in `crates/app/src/app/workspace_event_handler.rs` (all env-aware editors refresh when active env, env vars, globals, or reloads occur).
  4. Theme change handler (since `variable_style` reads `theme()`).

### 3.4 Scope of Editors Covered
- Primary: `json_editor`, `xml_editor`.
- Secondary (shared reuse via `attach_env_lsp` + `EnvEditorDecorations`): `grpc.message_editor`, `graphql.query_editor`, `graphql.variables_editor`.
- Excluded from `{{var}}` decorations: `pre_script_editor` / `test_script_editor` (JavaScript editors using `z.env.get(...)`; they consume `crate::env::list_candidates` / `VariableCandidate` for script completion).

---

## 4. Step-by-Step Implementation Tasks

### Step 1: Implement `crates/core/src/env/mod.rs` (`zest_core::env`)
- Export `pub mod env;` in `crates/core/src/lib.rs`.
- Implement `VariableScope`, `ResolvedVariable`, `VariableCandidate`, `resolve_variable`, `list_candidates`, and `find_variable_span_at`.
- Add pure Rust unit tests covering:
  - Precedence: Active overrides Global; first-wins on duplicate keys within same environment.
  - Secret masking (`"••••••••"`) in both Active and Global scopes.
  - Dynamic `$...` resolution (`$uuid`, etc.) and unknown `$nonexistent` falling back to `Missing`.
  - Candidate filtering & deduplication (`Active` shadows `Global`, case-insensitive substring match).
  - `find_variable_span_at`: cursor before/inside/at-boundary/outside `{{var}}`, multiple variables on same line, multi-byte UTF-8 prefixes, unclosed `{{var`.

### Step 2: Create `crates/app/src/env/` UI Binding Module & Migrate Callers
- Register `mod env;` in `crates/app/src/main.rs`.
- Create `style.rs`, `input_tokens.rs`, `completion.rs`, `popup.rs`, `mod.rs` delegating all resolution and candidate logic to `zest_core::env`.
- Update the 11 import sites from `crate::components::completion::env_autocomplete` to `crate::env` and delete `crates/app/src/components/completion/env_autocomplete/` (and its export in `components/completion/mod.rs`):
  1. `crates/app/src/app/subscriptions.rs`
  2. `crates/app/src/modals/workspace_setting_modal/auth_inputs.rs`
  3. `crates/app/src/components/env_input.rs`
  4. `crates/app/src/components/request_bar.rs`
  5. `crates/app/src/components/script_editor/completion.rs`
  6. `crates/app/src/components/kv_table.rs`
  7. `crates/app/src/components/completion/proto_json/mod.rs`
  8. `crates/app/src/components/completion/graphql_query.rs`
  9. `crates/app/src/components/completion/mod.rs`
  10. `crates/app/src/components/body_editor/multipart/state.rs`
  11. `crates/app/src/components/request_composer/composer_inputs.rs`

### Step 3: Implement `crates/app/src/env/editor_lsp.rs` (Decorations + Hover + Definition)
- `EnvEditorDecorations`:
  - `new(state: &mut CodeEditorState, cx: &mut Context<CodeEditorState>) -> Self`
  - `sync(&self, text: &str, env: &EnvironmentState, cx: &mut App)`
  - Fast path: if `!text.contains("{{")`, call `.set(vec![], cx)` on both collections immediately.
  - For each span from `zest_core::interpolation::scan_variable_spans(text)`:
    - Resolve via `crate::env::resolve_in_state(env, &span.name)` → `zest_core::env::resolve_variable`.
    - Map scope via `crate::env::variable_style(&resolved.scope)`.
    - Push `RangeDecoration::new(span.start..span.end).with_style(RangeDecorationStyle::Fill).with_color(style.bg_tint)`.
    - Push `TextDecoration::new(span.start..span.end, HighlightStyle { color: Some(style.text), font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() })` (no brace dimming).
- `EnvHoverProvider`:
  - Uses `zest_core::env::find_variable_span_at` + `resolve_in_state`.
  - Converts byte span to `lsp_types::Range` via `RopeExt::offset_to_position`.
  - Renders Markdown (`MarkupKind::Markdown`): `**\`{{name}}\`** · _<scope_label>_` + value (or unresolved hint).
- `EnvDefinitionProvider` & `make_env_show_document_handler`:
  - Skips `VariableScope::Dynamic` (returns `Ok(vec![])`).
  - Returns `LocationLink` with `origin_selection_range` = `span` range and `target_uri` = `zest://env/<percent_encoded_name>`.
  - `show_document` matches `zest://env/<name>`, decodes `<name>`, calls `open_variable_in_modal(&env_state, &modal_state, &var_name, cx)`, returns `true`.
- `attach_env_lsp(state, env_state, modal_state)`.

### Step 4: Wire into `ComposerInputs`
- **File:** `crates/app/src/components/request_composer/composer_inputs.rs`
- Pass `modal_state: Entity<ModalState>` into `ComposerInputs::new(...)` (update call site in `crates/app/src/app/mod.rs`).
- Attach `crate::env::attach_env_lsp(...)` and create `EnvEditorDecorations` for:
  - `json_editor`
  - `xml_editor`
  - `grpc.message_editor`
  - `graphql.query_editor`
  - `graphql.variables_editor`
- Group decoration handles in `pub env_decorations: EnvEditorDecorationSet`.
- Add `ComposerInputs::refresh_all_env_decorations(&self, env: &EnvironmentState, cx: &mut App)` and per-editor refresh helpers.
- Refresh decorations inside `ComposerInputs::sync_from_request` right after `set_value` on each editor (passing `&EnvironmentState`).

### Step 5: Wire Reactive Event Subscriptions
- **Files:**
  - `crates/app/src/app/subscriptions.rs`
  - `crates/app/src/app/workspace_event_handler.rs`
- On `InputEvent::Change` for `json_editor`, `xml_editor`, `grpc_message_editor`, `graphql_query_editor`, `graphql_variables_editor`:
  - Sync that editor's `EnvEditorDecorations` with the new text and `env_state.read(cx)`.
- On `EnvironmentEvent` (`ActiveEnvironmentChanged`, `EnvironmentModified`, `GlobalsModified`, `EnvironmentsReloaded`) in `handle_environment_event`:
  - Call `this.composer_inputs.refresh_all_env_decorations(env_state.read(cx), cx)`.
- On theme change (where `sync_component_theme` is invoked):
  - Call `this.composer_inputs.refresh_all_env_decorations(env_state.read(cx), cx)`.

### Step 6: Clean Up Redundant Static Highlighting in JSON / XML Highlighters
- **Files:**
  - `crates/app/src/components/body_editor/json.rs`
  - `crates/app/src/components/body_editor/xml.rs`
- Verify `TextDecoration` precedence over `InputHighlighter` in `gpui-base`.
- Remove hardcoded `t.var_resolved` branching inside `JsonHighlighter` / `XmlHighlighter` so `EnvEditorDecorations` is the single source of truth for variable colors.

### Step 7: App-Side Unit Tests
- Keep all migrated `crates/app/src/env/` tests passing.
- Add unit tests in `crates/app/src/env/editor_lsp.rs`:
  - Hover Markdown formatting for Active, Global, Secret (`••••••••`), Dynamic `$uuid`, and Missing.
  - Virtual URI `zest://env/<name>` encode/decode round-trip (including `-`, `_`, `.`, spaces).
  - Definition provider skips `VariableScope::Dynamic` and emits valid `LocationLink` for Active/Global/Missing variables.

---

## 5. Risk Mitigation

| Risk | Mitigation |
| :--- | :--- |
| Import churn across 11 app files when migrating `env_autocomplete/` → `crates/app/src/env/` | Compiler checks 100% of imports; zero runtime risk. |
| Core/UI coupling leak | `crates/core/src/env/` accepts only `&Environment` / `&str` and returns plain Rust structs; zero GPUI or `lsp_types` dependencies in `zest-core`. |
| Multi-byte UTF-8 byte offsets panic on slicing or `offset_to_position` | `zest_core::interpolation::scan_variable_spans` yields guaranteed UTF-8 char-boundary offsets; clamp offsets safely to `text.len()`. |
| Secret values leaked in Hover popover | `zest_core::env::resolve_variable` masks `secret == true` as `"••••••••"` at the domain layer before UI ever sees it. |
| Performance regression on large JSON/XML bodies without variables | Fast-path `!text.contains("{{")` skips regex scanning and clears decorations in O(1). Only re-scan the active editor on `InputEvent::Change`. |
| `set_value` on request switch suppresses `InputEvent::Change` | Explicitly refresh decorations inside `ComposerInputs::sync_from_request` right after `set_value`. |
| Theme switch leaves stale light/dark decoration colors | Trigger `refresh_all_env_decorations` whenever theme changes. |

---

## 6. Verification Steps

```bash
# 1. Format check
cargo fmt --check

# 2. Run all unit tests (core + app)
cargo test

# 3. Strict clippy analysis (0 warnings)
cargo clippy --all-targets -- -D warnings
```

**Manual UI Verification:**
1. Open an HTTP request with JSON body containing:
   - Active env var: `"booking_id": "{{bookingId}}"` (blue tint + blue semibold text, `{{` and `}}` not dimmed)
   - Global var: `"api_key": "{{globalKey}}"` (purple tint + purple semibold text)
   - Dynamic var: `"trace_id": "{{$uuid}}"` (green tint + green semibold text)
   - Unquoted var: `"limit": {{pageSize}}` (styled identically without quotes)
   - Missing var: `"missing": "{{notDefined}}"` (red tint + red semibold text)
2. Hover each variable → verify popover displays name, scope, and resolved value (`••••••••` for secret vars).
3. `Cmd+Click` (macOS) / `Ctrl+Click` (Windows/Linux) on an Active/Global/Missing variable → verify Environment modal opens focused on that variable; verify Dynamic `$...` does not trigger Go to Definition.
4. Switch active environment or edit a variable in Environment modal → verify JSON/XML editor decorations update immediately without switching requests.
5. Switch Light/Dark theme → verify pill tint and text colors update immediately.
6. Verify URL bar, KV tables, Auth inputs, Script completion, XML body, GraphQL Query/Variables, and gRPC JSON Message all work seamlessly through `zest_core::env` + `crate::env`.
