# Plan 2026112001 — OpenAPI 3.0 Collection Export (Phase 5, Item 4 — Reduced Scope)

## Goal

Implement roadmap Phase 5 item 4 ("Export đa định dạng") with a **reduced scope agreed with the user (2026-11-20)**:
export a Zest workspace to **OpenAPI 3.0 (YAML / JSON)** only. Postman v2.1 and Bruno export are removed from
the plan. Entry point: workspace switcher dropdown menu → "Export OpenAPI..." → format-choice modal → save dialog.

## Context

- Phase 5 items 1–3 (cURL paste, Postman import, OpenAPI import) are complete. The only existing exporter is
  `zest_core::curl::request_to_curl` (single request → text). No `exporters` module exists in `zest-core`.
- The user chose the workspace dropdown menu as the entry point and a small format-choice modal (mirroring
  `ImportModal`) instead of extension-based detection.

## Architectural Decisions

1. **New core module `zest_core::exporters`** mirroring the importer architecture (pure functions, DTO reuse):
   - Reuse `importers::models::ImportedRequest` as the neutral export input DTO.
   - `requests_to_openapi(title, description, &[ImportedRequest]) -> OpenApiExport`
     where `OpenApiExport { document: serde_json::Value, warnings: Vec<String> }`.
   - `OpenApiFileFormat { Yaml, Json }` + `serialize_openapi(&OpenApiExport, format) -> Result<String>`
     (serde_yaml 0.9 / serde_json::to_string_pretty). Document built as `serde_json::Value` via `json!` —
     same raw-Value style as the importers, zero new dependencies.
2. **Inheritance resolved app-side**: `WorkspaceDataState::build_openapi_export(&self)` filters
   `self.requests` by `protocol.uses_http_transport()`, builds `folder_path` from `get_folder_hierarchy`,
   and resolves merged headers+auth via `resolve_request_hierarchy` (workspace + folder chain). WS/gRPC
   requests are skipped with warnings.
3. **UI mirrors ImportModal**: `components/export_modal/mod.rs` + `ExportModalUi { is_open }` in `ModalState`
   + registration in `app/modals.rs::attach_modal_overlays` + `dismiss_top_modal`/`has_any_modal_open`.
   File write via rfd `save_file` + `background_executor` (pattern from `response_inspector/body_tab.rs:422`).
4. Output target: `openapi: 3.0.3`, `info.version: "1.0.0"`.

## OpenAPI Mapping Rules

- paths: URL parsed with `url::Url`; path portion is the path key, deduped `scheme://host` entries go to
  `servers`; unparseable URLs (e.g. `{{base_url}}/users`) are emitted as raw path keys without servers.
- Query string in URL merged with `params` (enabled only, dedup by name+value). `{{var}}` → `{var}`; each
  `{var}` emits a required `in: path` parameter.
- Operation: `tags` = folder chain names, `summary` = request name, unique slugified `operationId`,
  lowercase method key.
- Headers (enabled, skip `Content-Type`) → `in: header` parameters; params → `in: query`.
- requestBody: Json → `application/json` + parsed example ({{var}} placeholder substitution fallback);
  FormUrlEncoded → urlencoded object properties; Multipart → `multipart/form-data` (`format: binary` for
  file items); Raw → its `content_type`; Binary → `application/octet-stream`; None → omitted.
- Responses: baseline `"200"` placeholder; `RequestExampleFile` entries → `responses.{status}` with content
  example (mime from content-type header, else mime_guess / JSON sniff).
- Auth (resolved): Bearer → http/bearer scheme; Basic → http/basic; ApiKey → apiKey header/query; deduped
  in `components.securitySchemes`, per-operation `security`.
- GraphQL → POST JSON body `{query, variables, operationName}`; SSE → auto `Accept: text/event-stream`;
  WebSocket/gRPC → skipped with warning.

## Step-by-Step Implementation Tasks

1. `crates/core/src/exporters/mod.rs` + `exporters/openapi/mod.rs` + `exporters/openapi/tests.rs`;
   declare `pub mod exporters;` in `crates/core/src/lib.rs`. Keep files < 650 LOC.
2. Core tests: fixtures for path templating, auth schemes, body variants, examples → responses,
   non-HTTP warnings, YAML/JSON serialization; roundtrip export → `importers::parse_openapi`.
3. App: `WorkspaceDataState::build_openapi_export`; `ModalState` `export_modal` flag + open/close +
   dismiss/has_any updates.
4. UI: `components/export_modal/mod.rs` (info row with counts, YAML/JSON choice, Export button →
   save dialog → background write → toast → close); register in `app/modals.rs`.
5. Menu entry in `titlebar/menus.rs` next to "Import..." (IconName::Upload).
6. Docs: `docs/roadmap.md` item 4 (mark OpenAPI export done, note Postman/Bruno removal, user decision
   2026-11-20); `.agents/skills/interop-spec-converters/SKILL.md` format matrix + exporter principles.

## Risk Mitigation

- Invalid JSON bodies containing `{{var}}` tokens: quoted-then-bare token substitution to placeholder
  strings before parse retry; final fallback = raw text example.
- URL parse failures: raw URL as path key keeps data lossless; path params declared for `{var}` tokens.
- Files kept small: body/auth mapping helpers may split into `exporters/openapi/body.rs` if `mod.rs`
  approaches the 650 LOC limit.

## Verification Steps

- `cargo fmt --check`
- `cargo test` (workspace; new `exporters::` tests incl. roundtrip)
- `cargo clippy --all-targets -- -D warnings` (0 warnings)
- Manual: workspace dropdown → Export OpenAPI → YAML and JSON outputs verified; exported YAML re-imported
  via Import modal (roundtrip sanity); WS/gRPC requests surface as warnings in the toast.
