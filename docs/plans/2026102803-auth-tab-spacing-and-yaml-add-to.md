# Plan: Auth Tab Vertical Spacing & YAML ApiKey add_to Field

## 1. Goal
1. Adjust vertical spacing in the request composer Auth tab (`auth_tab.rs`) to align and match the vertical rhythm/spacing of the Params tab (`kv_table.rs`).
2. Replace `in_header: bool` in `Auth::ApiKey` with `add_to: ApiKeyLocation` (values: `header` / `query` in YAML) across core models, networking engine, importers, and UI states. Backwards compatibility and legacy migrations are explicitly omitted as requested.

## 2. Context & Root Cause
- **Vertical Spacing**: `KvTable` uses an outer padding of `p(px(8.))` and individual rows with `px(px(6.)).py(px(2.))` (height 32px, 4px visual spacing between 28px inputs). `auth_tab.rs` was using `p(px(16.))` and `gap(px(10.))` between rows (10px visual spacing), making the Auth tab noticeably looser and inconsistent with the Params tab.
- **YAML `add_to` serialization**: `Auth::ApiKey` previously stored `in_header: bool`, which serialized to `in_header: true/false` in YAML. The user wants the field to serialize as `add_to: header` or `add_to: query` to match the UI concept "Add to: Header / Query Params".

## 3. Implementation Tasks
1. **Core Domain Models (`crates/core/src/models.rs`)**:
   - Define `pub enum ApiKeyLocation { Header, Query }` with `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]` and `#[serde(rename_all = "lowercase")]`.
   - Implement helper methods `is_header()`, `is_query()`, and `as_str()`.
   - Update `Auth::ApiKey { key, value, add_to: ApiKeyLocation }`.
   - Update `HttpRequestItem::set_api_key(&mut self, key: String, value: String, add_to: ApiKeyLocation)`.
   - Update unit tests in `models.rs`.
2. **Core Network & Export Engines**:
   - `crates/core/src/http/executor.rs`: Check `add_to.is_query()` for URL parameter appending and `add_to.is_header()` for header injection.
   - `crates/core/src/curl/mod.rs`: Check `add_to.is_header()` when generating cURL headers.
   - `crates/core/src/importers/openapi.rs`: Map `in` ("query" vs "header") to `ApiKeyLocation::Query` / `ApiKeyLocation::Header`.
   - `crates/core/src/importers/postman.rs`: Map `in` ("query" vs "header") to `ApiKeyLocation::Query` / `ApiKeyLocation::Header`.
3. **App UI & Subscriptions**:
   - `crates/app/src/components/inherited_headers.rs`: Update pattern match to `add_to: ApiKeyLocation::Header`.
   - `crates/app/src/app/subscriptions.rs`: Update `apikey_key_input` and `apikey_val_input` change handlers to store and forward `add_to: ApiKeyLocation`.
   - `crates/app/src/components/scope_settings_modal.rs`: Update folder/workspace settings auth handling and location dropdown to use `ApiKeyLocation`.
   - `crates/app/src/app/auth_tab.rs`:
     - Update outer container padding to `p(px(8.))` and remove `gap(px(10.))`.
     - Update each row to use `px(px(6.)).py(px(2.))` to match `KvTable` rows (32px row height, 4px spacing between inputs).
     - Update `Auth::ApiKey` handling to use `ApiKeyLocation` (`add_to`).
4. **Verification**:
   - Run `cargo fmt --check`.
   - Run `cargo test`.
   - Run `cargo clippy --all-targets -- -D warnings`.
