# Implementation Plan: OpenAPI & Swagger Multipart/Form-Data & URL-Encoded Body Import

Date: 2026-10-31
Sequence: 03
Plan Name: `openapi-multipart-form-urlencoded-body-import`

---

## 1. Goal

Fix missing request bodies when importing OpenAPI 3.0 / 3.1 and Swagger 2.0 specifications that define:
- `multipart/form-data` requests (including file uploads and mixed fields).
- `application/x-www-form-urlencoded` requests.
Ensure form fields, types (`is_file`), sample values, descriptions, and Content-Type headers are accurately extracted and imported into Zest's canonical domain models (`RequestBody::Multipart` and `RequestBody::FormUrlEncoded`).

---

## 2. Context & Root Cause Analysis

When importing OpenAPI specifications, JSON request bodies work smoothly, but `multipart/form-data` and `application/x-www-form-urlencoded` request bodies are missing or incomplete due to several gaps in `crates/core/src/importers/openapi.rs`:

1. **Swagger 2.0 `in: "formData"` parameters**:
   - In Swagger 2.0, form and multipart payloads are defined in `parameters` with `in: "formData"`.
   - `openapi.rs` only mapped `in: "query"` and `in: "header"`, and only checked `in: "body"` in `extract_request_body`.
   - As a result, all Swagger 2.0 form requests were assigned `RequestBody::None` with no Content-Type header.

2. **OpenAPI 3.x strict media-type string matching**:
   - `content.get("application/x-www-form-urlencoded")` and `content.get("multipart/form-data")` used exact string lookup.
   - Specs with media-type parameters (e.g., `multipart/form-data; charset=utf-8` or `boundary=...`) failed to match and fell through to `RequestBody::Raw`.

3. **Incomplete Schema Traversal (`allOf`, `anyOf`, `oneOf`, `$ref`)**:
   - `res_schema.get("properties")` failed to discover properties inside `allOf` compositions (very common with models extending base classes or DTOs).

4. **Missing Fallbacks to Examples**:
   - If a spec provided `example` or `examples` at the media-type or schema level without explicit `properties`, the parameter list was left empty.

5. **Multipart File Detection (`is_file`)**:
   - Properties with `format: "binary"`, `format: "byte"`, or Swagger 2 `type: "file"` were imported as plain text fields with `"string"` values instead of files (`is_file: true`).

---

## 3. Architectural Decisions

1. **Robust Media-Type Detection**:
   - Match content keys using normalized MIME types: split by `;`, trim whitespace, and lowercase.
   - Detect Form URL-Encoded if MIME type is `application/x-www-form-urlencoded` or `application/x-url-encoded`.
   - Detect Multipart if MIME type is `multipart/form-data` or starts with `multipart/`.

2. **Recursive Property Collector with Cycle Safety**:
   - Implement `collect_schema_properties(root, schema)` bounded by depth and visited `$ref` set.
   - Gather properties from `properties`, `allOf` entries, and first variant of `oneOf` / `anyOf`.
   - Capture `description`, `required`, and sample values.

3. **Fallback to Media/Schema Examples**:
   - When no properties are discovered from schema, parse `example` / `examples` (objects or query-string formats like `k1=v1&k2=v2`) into key-value pairs.

4. **File Upload Recognition**:
   - Identify file fields when `type == "file"` or `format == "binary" | "byte"`.
   - For `MultipartItem`, set `is_file: true` and keep `file_path: None` (or sample file name).

5. **Swagger 2.0 `in: "formData"` Support**:
   - Collect all operation-level and path-level parameters with `in: "formData"`.
   - Determine whether to create `RequestBody::Multipart` or `RequestBody::FormUrlEncoded` based on parameter types (`type: "file"`) and `consumes` declarations.

---

## 4. Implementation Tasks

- [x] **Task 1: Helper functions in `crates/core/src/importers/openapi/body.rs`**
  - Implement `normalize_mime_type(s: &str) -> &str`.
  - Implement recursive property extractor `collect_schema_form_fields` with cycle safety.
  - Implement example/sample fallback extractor for form fields.
  - Implement file type check `is_file_schema(schema: &Value) -> bool`.
- [x] **Task 2: Modularize OpenAPI importer and update `extract_request_body`**
  - Split `openapi.rs` into `openapi/mod.rs`, `openapi/body.rs`, and `openapi/tests.rs` (< 650 LOC each).
  - Extract operation/path-level `in: "formData"` parameters in Swagger 2.0 and convert them to `Multipart` or `FormUrlEncoded`.
  - Support normalized media type matching for OpenAPI 3.x `requestBody.content`.
  - Accurately populate `RequestBody::FormUrlEncoded` and `RequestBody::Multipart` with correct fields, `is_file`, descriptions, and sample values.
- [x] **Task 3: Unit Tests**
  - Test OpenAPI 3.0 `multipart/form-data` with binary file upload and text field.
  - Test OpenAPI 3.0 `application/x-www-form-urlencoded` with `allOf` and schema references.
  - Test OpenAPI 3.0 with media-type parameters (e.g., `multipart/form-data; charset=utf-8`).
  - Test Swagger 2.0 with `in: "formData"` (both file and text fields).
  - Test Swagger 2.0 with `application/x-www-form-urlencoded`.
  - Test OpenAPI 3.0 fallback to example/examples when schema has no explicit properties.

---

## 5. Verification

1. `cargo test --package zest-core --lib importers::`
2. `cargo clippy --package zest-core --all-targets -- -D warnings`
3. Verify all existing tests pass with 0 regressions.
