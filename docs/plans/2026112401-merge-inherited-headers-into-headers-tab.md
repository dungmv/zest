# Plan: Merge Inherited Headers Into Headers Tab & Encapsulate Auth Header Preview

- **Date**: 2026-11-24
- **Author**: Antigravity
- **Status**: Proposed (Awaiting Approval)
- **Target File**: `docs/plans/2026112401-merge-inherited-headers-into-headers-tab.md`

---

## 1. Goal

Consolidate the orphaned `crates/app/src/components/inherited_headers.rs` into the request composer's `headers_tab.rs` component, unify the secret masking utility, and eliminate logic duplication between UI Auth header preview and runtime transport execution.

---

## 2. Context & Motivation

1. **Fragmentation of Component Responsibilities**:
   - `crates/app/src/components/inherited_headers.rs` (73 LOC) currently sits loosely in `crates/app/src/components/`, while all other request composer tabs are cleanly modularized in `crates/app/src/components/request_composer/`.
   - Its only primary consumer is `headers_tab.rs` via `inherited_readonly_rows(...)`.
   - Its secondary utility `mask_secret` is cross-imported by `auth_tab.rs` (`use crate::components::inherited_headers::mask_secret;`), creating an awkward coupling across tabs.

2. **Domain Logic Duplication (Auth $\rightarrow$ Header)**:
   - When previewing inherited auth in the Headers tab (`inherited_headers.rs`), a manual `match &info.auth` maps Bearer, Basic, and ApiKey to headers (`Authorization: Bearer ...`, `Authorization: Basic ...`, `<key>: <value>`).
   - The same matching exists in `crates/core/src/transport/resolved.rs` for actual request execution.
   - Any new authentication scheme requires synchronized manual updates in both places.

3. **Size Budget**:
   - `headers_tab.rs` is currently 54 LOC.
   - Merging the inherited header logic directly into `headers_tab.rs` yields ~110-120 LOC, which is well below the ~650 LOC modularity threshold specified in `AGENTS.md`.

---

## 3. Architectural Decisions

### Decision 1: Relocate `mask_secret` to Common Component Helpers
- Move `mask_secret` to `crates/app/src/components/common/secret.rs` (and re-export via `crate::components::common::*`).
- **Rationale**: `mask_secret` is a pure display formatting helper for secrets in read-only UI fields. Both `auth_tab.rs` and `headers_tab.rs` can consume it naturally from `components::common` without inter-tab coupling.

### Decision 2: Single Source of Truth for Auth Headers in `zest_core`
- Add a helper method on `zest_core::models::Auth`:
  ```rust
  impl Auth {
      /// Returns the HTTP header (key, value) produced by this auth method for preview or transport.
      /// When `mask_secrets` is true, credential values are replaced with a masked placeholder.
      pub fn to_header_preview(&self) -> Option<(String, String)> { ... }
  }
  ```
- **Rationale**: Centralizes the business rule of "which HTTP headers does an Auth configuration produce" in domain models, keeping core transport and UI preview in sync.

### Decision 3: Merge Inherited Rows Computation into `headers_tab.rs`
- Inline and scope `build_inherited_readonly_rows` private to `crates/app/src/components/request_composer/headers_tab.rs`.
- `render_headers_tab` calls it directly to construct `readonly_rows` for `KvTable`.
- Delete `crates/app/src/components/inherited_headers.rs` and remove its export from `crates/app/src/components/mod.rs`.

---

## 4. Step-by-Step Implementation Tasks

### Task 1: Create `crates/app/src/components/common/secret.rs`
- Implement `pub fn mask_secret(value: &str) -> String`.
  - Preserve `{{...}}` variable syntax verbatim.
  - Return `••••••••` for non-empty secret values.
- Add unit tests for `mask_secret`.
- Expose via `crates/app/src/components/common/mod.rs`.

### Task 2: Update `auth_tab.rs` Import
- In `crates/app/src/components/request_composer/auth_tab.rs`:
  - Change `use crate::components::inherited_headers::mask_secret;` to `use crate::components::common::mask_secret;` (or use via `use crate::components::*;`).

### Task 3: Unify Auth Header Generation in `zest-core`
- In `crates/core/src/models/request.rs`:
  - Add helper `Auth::header_preview(&self, mask: bool) -> Option<(String, String)>` or a dedicated preview formatter.
  - Cover with unit tests in `crates/core/src/models/tests.rs`.

### Task 4: Integrate Inherited Headers into `headers_tab.rs`
- In `crates/app/src/components/request_composer/headers_tab.rs`:
  - Add private helper `build_inherited_readonly_rows(ws: &WorkspaceDataState, req: &RequestItem) -> Vec<KvReadonlyRow>`.
  - Call `ws.inherited_headers_for_request(req)`.
  - Use the core `Auth::header_preview` with masking for inherited auth preview.
  - Directly pass these rows to `KvTable::new(...).readonly_rows(...)`.

### Task 5: Clean Up Old Module
- Remove `pub mod inherited_headers;` and `pub use inherited_headers::*;` from `crates/app/src/components/mod.rs`.
- Delete `crates/app/src/components/inherited_headers.rs`.
- Update references in `.agents/skills/gpui-architecture/SKILL.md` and `.agents/skills/http-pipeline-engine/SKILL.md` to point to `headers_tab.rs`.

### Task 6: Verification & Quality Assurance
- Run `cargo fmt --check`.
- Run `cargo test` (both core and app crates).
- Run `cargo clippy --all-targets -- -D warnings` (ensure 0 warnings).

---

## 5. Risk Assessment & Mitigation

| Risk | Severity | Mitigation |
| :--- | :--- | :--- |
| Breaking UI tests or imports | Low | Full-codebase grep confirmed only `auth_tab.rs` and `headers_tab.rs` access `inherited_headers.rs`. |
| Exceeding file LOC limit | Low | Merged `headers_tab.rs` will be ~120 LOC, well under the 650 LOC threshold. |
| Masking inconsistency | Low | Centralizing `mask_secret` in `common::secret` and adding unit tests guarantees consistent behavior across Auth Tab and Headers Tab. |

---

## 6. Verification Checklist

- [ ] `cargo check --workspace` passes without errors.
- [ ] Unit tests for `mask_secret` and `Auth::header_preview` pass.
- [ ] UI displays inherited headers and auth headers identically to previous implementation.
- [ ] No warnings in `cargo clippy --all-targets -- -D warnings`.
