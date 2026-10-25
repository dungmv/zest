# Plan: Inherited Headers & Auth Visibility in Composer

- **Date**: 2026-10-25
- **Status**: Approved / Implemented (Rev 2)
- **Scope**: `zest-core` hierarchy module, `zest` app composer (Headers tab, Auth tab)

## Revision 2 (same day): unified table, no separate Inherited panel

User feedback: the separate collapsible panel hurt the experience. Rev 2 replaces it:

- **Headers tab**: inherited headers render inside the SAME `KvTable`, placed ABOVE the
  local editable rows, as disabled inputs (`render_disabled_input`: same h28/rounded/mono
  shape as env inputs, `bg_app` + `border_subtle`, muted text). The checkbox column shows
  a static (non-clickable) enable state in muted gray (`text_muted` fill) so it never
  reads as a request row; the delete column shows an **info icon**
  (`Popover::new(...).trigger(Button).content(...)`) that reveals the parent, e.g.
  `Inherited from folder "API"` / `Added by inherited auth from workspace "X"`.
  Rows overridden by the request are EXCLUDED (not sent — showing them would mislead).
- **Auth tab** (`Auth::Inherit`): keep the original one-line description banner
  (`get_inherited_auth_description`) naming the parent, followed by a disabled mirror of
  the local auth form for the inherited type (Token/Username/Password/Key/Value/Add-to,
  secrets masked). The Configure button/card from Rev 1 was removed.
- `InheritedHeadersPanel`, `AuthHeaderPreview`, `inherited_auth_header_preview`, and
  `RequestSessionState::inherited_headers_expanded` were deleted.
  `inherited_headers.rs` now only provides `mask_secret` and
  `inherited_readonly_rows(ws, req) -> Vec<KvReadonlyRow>`.
- `inherited_headers_for_request` now emits display labels (`folder "X"` /
  `workspace "Y"`) as sources so popovers read naturally.

## Goal

When a request inherits headers or auth from parent folders / workspace, the composer must
display the actual inherited data (key/value, source, effective auth config) instead of a
count-only banner, eliminating information loss. Reference design: Yaak's collapsible
"Inherited" group (dimmed, read-only rows), extended with source chips.

## Context

- Headers tab banner (`crates/app/src/app/request_composer.rs:388-409`) only printed a raw
  count (`ws.headers.len() + Σ folder.headers.len()`): no dedupe, no enabled filtering, no
  key/value display.
- Auth tab (`crates/app/src/app/auth_tab.rs:191-206`) showed a one-line description for
  `Auth::Inherit` without the actual inherited configuration.
- Headers produced by inherited auth (e.g. `Authorization: Bearer ...`) were invisible.
- Send-time semantics live in `zest_core::http::resolve_hierarchical_headers_and_auth`
  (case-insensitive child-wins merge, order preserved; auth: nearest non-inherit ancestor,
  `Auth::None` blocks). The UI must reflect exactly these semantics.

## Architectural Decisions

1. **Core owns hierarchy projection**: new `crates/core/src/http/hierarchy.rs` module hosts
   the existing merge function (moved out of the 1193-LOC `executor.rs`) plus a new
   `resolve_inherited_headers` that reuses the same merge machinery and attaches source
   attribution and an `overridden_by_request` flag. Single source of truth for merge
   semantics; public path `zest_core::http::resolve_hierarchical_headers_and_auth` is
   preserved via re-export.
2. **Source attribution**: workspace/folder names are passed in by the app; the merged
   winner of each key is attributed to its nearest ancestor.
3. **UI state in `RequestSessionState`**: collapse flag `inherited_headers_expanded`
   (default `true`) follows the multi-entity state architecture (no ad-hoc render state).
4. **Read-only presentation**: new `InheritedHeadersPanel` component mirrors `KvTable`
   column geometry (w20 icon / w220 key / flex value / right chips) with dimmed text,
   source chips, and status chips (`Overridden`, `Disabled`, `Auth`). Body capped at
   ~160px with `overflow_y_scroll().id(...)` (gpui-pre 0.3.9 requires `.id()`).
5. **Secret masking**: non-env secret values render as fixed-length `••••••••`; values
   containing `{{` (environment references) render verbatim. Shared `mask_secret` helper.
6. **Auth tab card**: `Auth::Inherit` renders a read-only card (source, type, mirrored
   fields) plus a "Configure…" button that opens the existing scope-settings modal
   (`ModalState::open_scope_settings`).

## Step-by-Step Implementation Tasks

1. `docs/plans/20261025-inherited-headers-auth-ui.md` (this file).
2. Core `crates/core/src/http/hierarchy.rs`:
   - Move `resolve_hierarchical_headers_and_auth` + its test from `executor.rs`.
   - Add `InheritedHeader { pair, source, overridden_by_request }`,
     `resolve_inherited_headers(workspace_name, workspace_headers, folder_chain: &[(name, headers)], request_headers)`.
   - Unit tests: source attribution, nearest-folder override, request override flag,
     disabled rows kept, empty keys skipped.
   - Update `http/mod.rs` re-exports.
3. App state `workspace_data_state.rs`:
   - `inherited_headers_for_request(&self, req) -> Vec<InheritedHeader>`.
   - `InheritedAuthInfo { source_name, source_folder_id, auth }` +
     `inherited_auth_source(folder_id) -> Option<InheritedAuthInfo>`; refactor
     `get_inherited_auth_description` onto it (output strings unchanged).
   - Extend `workspace_data_tests.rs`.
4. `request_session_state.rs`: `inherited_headers_expanded` + toggle method.
5. New `crates/app/src/components/inherited_headers.rs`: `InheritedHeadersPanel`
   (RenderOnce) + `AuthHeaderPreview` + `pub(crate) mask_secret`.
6. `request_composer.rs` Headers branch: replace banner with the panel; compute auth
   pseudo-row (`Authorization: Bearer ••••••••`, API-key header) when request auth is
   `Inherit`.
7. `auth_tab.rs` `Auth::Inherit` branch: inherited auth card + "Configure…" button.

## Risk Mitigation

- Function move is path-preserving (single re-export; only in-crate test imports move).
- Only GPUI APIs verified to exist in gpui-pre 0.3.9 are used (chevron icons,
  `overflow_y_scroll` + `.id()`, plain div chips; no strikethrough API).
- Merge semantics unchanged at send time — new UI code only reads, never rewrites.

## Verification Steps

1. `cargo fmt --check`
2. `cargo test`
3. `cargo clippy --all-targets -- -D warnings` (0 warnings)
4. Manual: request inside folder with folder/workspace headers & auth → panel lists rows
   with source chips, override/disable chips, masked auth preview; Auth tab card shows
   inherited config; "Configure…" opens the correct scope settings.
